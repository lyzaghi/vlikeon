#import <Foundation/Foundation.h>
#include "process.hpp"
#include <atomic>
#include <cerrno>
#include <csignal>
#include <fcntl.h>
#include <libproc.h>
#include <map>
#include <memory>
#include <mutex>
#include <spawn.h>
#include <string>
#include <sys/event.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <thread>
#include <unistd.h>
#include <utility>
#include <vector>

namespace omniplay::macos {
namespace {
struct Descriptor {
  int value = -1;
  Descriptor() = default;
  explicit Descriptor(int descriptor) : value(descriptor) {}
  Descriptor(const Descriptor&) = delete;
  Descriptor& operator=(const Descriptor&) = delete;
  Descriptor(Descriptor&& other) noexcept : value(other.value) { other.value = -1; }
  Descriptor& operator=(Descriptor&& other) noexcept {
    if (this != &other) {
      if (value >= 0) close(value);
      value = other.value;
      other.value = -1;
    }
    return *this;
  }
  ~Descriptor() { if (value >= 0) close(value); }
  int release() { int result = value; value = -1; return result; }
};

struct Pipe { Descriptor read; Descriptor write; };
struct Identity { pid_t pid = -1; uint64_t seconds = 0; uint64_t microseconds = 0; };
struct Active {
  std::string session;
  uint64_t owner = 0;
  Identity actor;
  Identity presentation;
  Descriptor events;
  std::atomic_bool stopping = false;
};

std::mutex g_processes;
std::map<std::string, std::shared_ptr<Active>> g_active;

bool uuid(const std::string& value) {
  if (value.size() != 36) return false;
  for (size_t index = 0; index < value.size(); ++index) {
    if (index == 8 || index == 13 || index == 18 || index == 23) {
      if (value[index] != '-') return false;
    } else if (!((value[index] >= '0' && value[index] <= '9') ||
                 (value[index] >= 'a' && value[index] <= 'f'))) return false;
  }
  return true;
}

bool ordinary_private_directory(const std::filesystem::path& path) {
  struct stat shape{};
  return lstat(path.c_str(), &shape) == 0 && S_ISDIR(shape.st_mode) &&
      shape.st_uid == geteuid() && (shape.st_mode & 07777) == 0700;
}

bool same(const RuntimeSpec& left, const RuntimeSpec& right) {
  return left.id == right.id && left.team_identifier == right.team_identifier &&
      left.actor_path == right.actor_path && left.actor_identifier == right.actor_identifier &&
      left.presentation_path == right.presentation_path &&
      left.presentation_identifier == right.presentation_identifier &&
      left.core_path == right.core_path && left.core_identifier == right.core_identifier;
}

std::optional<Pipe> make_pipe() {
  int raw[2] = {-1, -1};
  if (pipe(raw) != 0) return std::nullopt;
  Descriptor original_read(raw[0]), original_write(raw[1]);
  int elevated_read = fcntl(original_read.value, F_DUPFD_CLOEXEC, 10);
  if (elevated_read < 10) {
    if (elevated_read >= 0) close(elevated_read);
    return std::nullopt;
  }
  Descriptor read(elevated_read);
  int elevated_write = fcntl(original_write.value, F_DUPFD_CLOEXEC, 10);
  if (elevated_write < 10) {
    if (elevated_write >= 0) close(elevated_write);
    return std::nullopt;
  }
  return Pipe{std::move(read), Descriptor(elevated_write)};
}

std::optional<Identity> identity(pid_t pid) {
  proc_bsdinfo info{};
  if (pid <= 1 || proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != sizeof(info) ||
      info.pbi_pid != static_cast<uint32_t>(pid)) return std::nullopt;
  return Identity{pid, info.pbi_start_tvsec, info.pbi_start_tvusec};
}

bool alive(const Identity& expected) {
  auto current = identity(expected.pid);
  return current && current->seconds == expected.seconds &&
      current->microseconds == expected.microseconds;
}

void terminate(const Identity& process) {
  if (alive(process)) kill(process.pid, SIGKILL);
}

void reap(pid_t pid) {
  if (pid <= 1) return;
  int status = 0;
  while (waitpid(pid, &status, 0) < 0 && errno == EINTR) {}
}

bool register_exit(int queue, pid_t pid) {
  struct kevent change{};
  EV_SET(&change, static_cast<uintptr_t>(pid), EVFILT_PROC,
      EV_ADD | EV_ENABLE | EV_ONESHOT, NOTE_EXIT, 0, nullptr);
  return kevent(queue, &change, 1, nullptr, 0, nullptr) == 0;
}

std::vector<char*> pointers(std::vector<std::string>& storage) {
  std::vector<char*> result;
  result.reserve(storage.size() + 1);
  for (auto& value : storage) result.push_back(value.data());
  result.push_back(nullptr);
  return result;
}

std::vector<char*> environment(std::vector<std::string>& storage) {
  NSString* home = NSHomeDirectory();
  NSString* temporary = NSTemporaryDirectory();
  if (![home isKindOfClass:NSString.class] || home.length == 0 ||
      ![temporary isKindOfClass:NSString.class] || temporary.length == 0) return {};
  storage = {
      "HOME=" + std::string(home.fileSystemRepresentation),
      "TMPDIR=" + std::string(temporary.fileSystemRepresentation),
      "PATH=/usr/bin:/bin", "LANG=en_US.UTF-8", "LC_ALL=en_US.UTF-8"};
  return pointers(storage);
}

std::optional<pid_t> spawn_child(const std::string& executable,
    const std::filesystem::path& working_directory, std::vector<std::string> argument_storage,
    const std::vector<std::pair<int, int>>& descriptors) {
  Descriptor null(open("/dev/null", O_RDWR | O_CLOEXEC | O_NOFOLLOW));
  if (null.value < 0) return std::nullopt;
  posix_spawn_file_actions_t actions{};
  posix_spawnattr_t attributes{};
  if (posix_spawn_file_actions_init(&actions) != 0) return std::nullopt;
  bool ready = posix_spawn_file_actions_addchdir_np(&actions, working_directory.c_str()) == 0;
  for (const auto& [source, target] : descriptors) {
    if (ready && posix_spawn_file_actions_adddup2(&actions, source, target) != 0) ready = false;
  }
  if (ready && posix_spawn_file_actions_adddup2(&actions, null.value, STDERR_FILENO) != 0)
    ready = false;
  if (posix_spawnattr_init(&attributes) != 0) {
    posix_spawn_file_actions_destroy(&actions);
    return std::nullopt;
  }
  short flags = POSIX_SPAWN_CLOEXEC_DEFAULT | POSIX_SPAWN_SETPGROUP;
  if (posix_spawnattr_setflags(&attributes, flags) != 0 ||
      posix_spawnattr_setpgroup(&attributes, 0) != 0) ready = false;
  std::vector<std::string> environment_storage;
  auto env = environment(environment_storage);
  auto argv = pointers(argument_storage);
  pid_t child = -1;
  int result = ready && !env.empty() ? posix_spawn(&child, executable.c_str(), &actions,
      &attributes, argv.data(), env.data()) : EINVAL;
  posix_spawnattr_destroy(&attributes);
  posix_spawn_file_actions_destroy(&actions);
  if (result != 0 || child <= 1) return std::nullopt;
  return child;
}

void monitor(const std::shared_ptr<Active>& active) {
  struct kevent event{};
  for (;;) {
    int count = kevent(active->events.value, nullptr, 0, &event, 1, nullptr);
    if (count > 0 || (count < 0 && errno != EINTR)) break;
  }
  active->stopping.store(true);
  terminate(active->actor);
  terminate(active->presentation);
  reap(active->actor.pid);
  reap(active->presentation.pid);
  std::lock_guard lock(g_processes);
  auto found = g_active.find(active->session);
  if (found != g_active.end() && found->second.get() == active.get()) g_active.erase(found);
}

void stop(const std::shared_ptr<Active>& active) {
  active->stopping.store(true);
  terminate(active->actor);
  terminate(active->presentation);
}
}

std::optional<LaunchEndpoints> launch_pair(const RuntimeSpec& runtime,
    const std::filesystem::path& session_directory, const LaunchParameters& parameters,
    uint64_t connection_owner) {
  if (!connection_owner || !uuid(parameters.session_id) || parameters.maximum_width == 0 ||
      parameters.maximum_width > 4096 || parameters.maximum_height == 0 ||
      parameters.maximum_height > 4096 || parameters.volume > 100 ||
      !ordinary_private_directory(session_directory)) return std::nullopt;
  auto current = admit_runtime(runtime.id.c_str());
  if (!current || !same(runtime, *current)) return std::nullopt;
  auto broker_to_actor = make_pipe();
  auto actor_to_broker = make_pipe();
  auto presentation_to_actor = make_pipe();
  auto actor_to_presentation = make_pipe();
  if (!broker_to_actor || !actor_to_broker || !presentation_to_actor ||
      !actor_to_presentation) return std::nullopt;
  Descriptor events(kqueue());
  if (events.value < 0) return std::nullopt;
  std::vector<std::string> presentation_arguments{
      runtime.presentation_path, "--stdio-presentation-v2", parameters.session_id,
      std::to_string(parameters.maximum_width), std::to_string(parameters.maximum_height),
      std::to_string(parameters.volume), parameters.haptics ? "haptics" : "no-haptics"};
  auto fresh_presentation = admit_runtime(runtime.id.c_str());
  if (!fresh_presentation || !same(runtime, *fresh_presentation)) return std::nullopt;
  auto presentation_pid = spawn_child(runtime.presentation_path, session_directory,
      std::move(presentation_arguments), {{actor_to_presentation->read.value, STDIN_FILENO},
                                         {presentation_to_actor->write.value, STDOUT_FILENO}});
  if (!presentation_pid) return std::nullopt;
  auto presentation_identity = identity(*presentation_pid);
  if (!presentation_identity || !register_exit(events.value, *presentation_pid)) {
    if (presentation_identity) terminate(*presentation_identity); else kill(*presentation_pid, SIGKILL);
    reap(*presentation_pid);
    return std::nullopt;
  }
  std::vector<std::string> actor_arguments{
      runtime.actor_path, "--stdio-runtime-v1", "--presentation-client-v2",
      parameters.session_id, "3", "4"};
  auto fresh_actor = admit_runtime(runtime.id.c_str());
  if (!fresh_actor || !same(runtime, *fresh_actor)) {
    terminate(*presentation_identity);
    reap(*presentation_pid);
    return std::nullopt;
  }
  auto actor_pid = spawn_child(runtime.actor_path, session_directory, std::move(actor_arguments),
      {{broker_to_actor->read.value, STDIN_FILENO}, {actor_to_broker->write.value, STDOUT_FILENO},
       {presentation_to_actor->read.value, 3}, {actor_to_presentation->write.value, 4}});
  if (!actor_pid) {
    terminate(*presentation_identity);
    reap(*presentation_pid);
    return std::nullopt;
  }
  auto actor_identity = identity(*actor_pid);
  if (!actor_identity || !register_exit(events.value, *actor_pid)) {
    if (actor_identity) terminate(*actor_identity); else kill(*actor_pid, SIGKILL);
    terminate(*presentation_identity);
    reap(*actor_pid);
    reap(*presentation_pid);
    return std::nullopt;
  }
  auto active = std::make_shared<Active>();
  active->session = parameters.session_id;
  active->owner = connection_owner;
  active->actor = *actor_identity;
  active->presentation = *presentation_identity;
  active->events = std::move(events);
  {
    std::lock_guard lock(g_processes);
    if (g_active.size() >= 4 || g_active.contains(active->session)) {
      stop(active);
      reap(active->actor.pid);
      reap(active->presentation.pid);
      return std::nullopt;
    }
    g_active.emplace(active->session, active);
  }
  try { std::thread([active] { monitor(active); }).detach(); }
  catch (...) {
    { std::lock_guard lock(g_processes); g_active.erase(active->session); }
    stop(active);
    reap(active->actor.pid);
    reap(active->presentation.pid);
    return std::nullopt;
  }
  return LaunchEndpoints{broker_to_actor->write.release(), actor_to_broker->read.release()};
}

bool session_active(const std::string& session_id) {
  std::lock_guard lock(g_processes);
  return g_active.contains(session_id);
}

void stop_session(const std::string& session_id) {
  std::shared_ptr<Active> active;
  {
    std::lock_guard lock(g_processes);
    auto found = g_active.find(session_id);
    if (found != g_active.end()) active = found->second;
  }
  if (active) stop(active);
}

void stop_connection(uint64_t connection_owner) {
  std::vector<std::shared_ptr<Active>> matches;
  {
    std::lock_guard lock(g_processes);
    for (const auto& [_, active] : g_active) {
      if (active->owner == connection_owner) matches.push_back(active);
    }
  }
  for (const auto& active : matches) stop(active);
}
}
