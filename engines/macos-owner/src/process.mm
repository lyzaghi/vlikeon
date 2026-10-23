#import <Foundation/Foundation.h>
#include "process.hpp"
#include <cerrno>
#include <algorithm>
#include <array>
#include <chrono>
#include <csignal>
#include <fcntl.h>
#include <libproc.h>
#include <map>
#include <memory>
#include <mutex>
#include <spawn.h>
#include <stdexcept>
#include <sys/event.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <thread>
#include <unistd.h>
#include <utility>
#include <vector>

namespace omniplay::macos {
namespace {
using Clock = std::chrono::steady_clock;
struct Descriptor {
  int value = -1;
  Descriptor() = default;
  explicit Descriptor(int descriptor) : value(descriptor) {}
  Descriptor(const Descriptor&) = delete;
  Descriptor& operator=(const Descriptor&) = delete;
  Descriptor(Descriptor&& other) noexcept : value(std::exchange(other.value, -1)) {}
  Descriptor& operator=(Descriptor&& other) noexcept {
    if (this != &other) { if (value >= 0) close(value); value = std::exchange(other.value, -1); }
    return *this;
  }
  ~Descriptor() { if (value >= 0) close(value); }
  int release() { return std::exchange(value, -1); }
};
struct Pipe { Descriptor read; Descriptor write; };
struct Identity { pid_t pid; uint64_t seconds; uint64_t microseconds; };
struct Child {
  // Only an immediate posix_spawn result enters here. Reaping and signalling
  // share Active::lifecycle; an unreaped direct child PID cannot be recycled.
  // Never construct this owner from an XPC request or persisted PID.
  pid_t pid = -1;
  std::optional<Identity> identity;
  bool reaped = false;
  bool uncertain = false;
};
struct Active {
  std::string session;
  uint64_t owner = 0;
  std::mutex lifecycle;
  std::shared_ptr<VirtualMachine> guest;
  uint64_t reserved_mb = 0;
  Child presentation;
  Descriptor events;
  Descriptor lifetime;
  Descriptor activation;
  uint64_t activation_sequence=0;
  std::optional<SessionLease> storage;
  bool stopping = false;
  bool monitor_started = false;
};
std::mutex g_processes;
std::map<std::string, std::shared_ptr<Active>> g_active;

bool uuid(const std::string& value) {
  if (value.size() != 36) return false;
  bool nonzero = false;
  for (size_t index = 0; index < value.size(); ++index) {
    if (index == 8 || index == 13 || index == 18 || index == 23) {
      if (value[index] != '-') return false;
    } else {
      if (!((value[index] >= '0' && value[index] <= '9') ||
            (value[index] >= 'a' && value[index] <= 'f'))) return false;
      nonzero |= value[index] != '0';
    }
  }
  return nonzero;
}
bool ordinary_private_directory(const std::filesystem::path& path) {
  struct stat shape{};
  return path.is_absolute() && lstat(path.c_str(), &shape) == 0 && S_ISDIR(shape.st_mode) &&
      shape.st_uid == geteuid() && (shape.st_mode & 07777) == 0700;
}
bool same(const RuntimeSpec& a, const RuntimeSpec& b) {
  return a.id == b.id && a.team_identifier == b.team_identifier &&
      a.presentation_path == b.presentation_path && a.presentation_identifier == b.presentation_identifier &&
      a.guest.kernel == b.guest.kernel && a.guest.initrd == b.guest.initrd &&
      a.kernel_sha256 == b.kernel_sha256 && a.initrd_sha256 == b.initrd_sha256 &&
      a.guest.memory_mb == b.guest.memory_mb && a.guest.cpu_count == b.guest.cpu_count &&
      a.guest.scratch_mb == b.guest.scratch_mb && a.presentation_memory_mb == b.presentation_memory_mb;
}
std::optional<Pipe> make_pipe() {
  int raw[2] = {-1, -1};
  if (pipe(raw) != 0) return std::nullopt;
  Descriptor original_read(raw[0]), original_write(raw[1]);
  Descriptor read(fcntl(original_read.value, F_DUPFD_CLOEXEC, 10));
  if (read.value < 10) return std::nullopt;
  Descriptor write(fcntl(original_write.value, F_DUPFD_CLOEXEC, 10));
  if (write.value < 10) return std::nullopt;
  return Pipe{std::move(read), std::move(write)};
}
std::optional<Identity> identity(pid_t pid) {
  proc_bsdinfo info{};
  if (pid <= 1 || proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &info, sizeof(info)) != sizeof(info) ||
      info.pbi_pid != static_cast<uint32_t>(pid) || info.pbi_ppid != static_cast<uint32_t>(getpid()) ||
      (!info.pbi_start_tvsec && !info.pbi_start_tvusec)) return std::nullopt;
  return Identity{pid, info.pbi_start_tvsec, info.pbi_start_tvusec};
}
bool same_identity(const Identity& expected) {
  auto current = identity(expected.pid);
  return current && current->seconds == expected.seconds && current->microseconds == expected.microseconds;
}
// All helpers require Active::lifecycle. ESRCH, ECHILD and a failed process
// observation never constitute a successful wait receipt.
void signal_owned(Child& child, int signal) {
  if (child.pid > 1 && !child.reaped && !child.uncertain) {
    if (kill(child.pid, signal) != 0 && errno != ESRCH) child.uncertain = true;
  }
}
bool observe(Child& child) {
  if (child.pid <= 1 || child.reaped) return true;
  if (child.uncertain) return false;
  int status = 0;
  const pid_t result = waitpid(child.pid, &status, WNOHANG);
  if (result == child.pid) { child.reaped = true; return true; }
  if (result < 0 && errno != EINTR) child.uncertain = true;
  return false;
}
void stop_locked(Active& active) {
  active.stopping = true;
  active.lifetime=Descriptor();active.activation=Descriptor();
  if (active.guest) active.guest->stop();
  signal_owned(active.presentation, SIGKILL);
}
bool ended_locked(Active& active) {
  const bool actor = !active.guest || active.guest->ended();
  const bool presentation = observe(active.presentation);
  return actor && presentation && (!active.storage || active.storage->finish_launch());
}
void forget_confirmed(const std::shared_ptr<Active>& active) {
  std::lock_guard lock(g_processes);
  auto found = g_active.find(active->session);
  if (found != g_active.end() && found->second.get() == active.get()) g_active.erase(found);
}
bool register_exit(int queue, pid_t pid) {
  struct kevent change{};
  EV_SET(&change, static_cast<uintptr_t>(pid), EVFILT_PROC,
      EV_ADD | EV_ENABLE | EV_ONESHOT, NOTE_EXIT, 0, nullptr);
  return kevent(queue, &change, 1, nullptr, 0, nullptr) == 0;
}
std::vector<char*> pointers(std::vector<std::string>& storage) {
  std::vector<char*> result; result.reserve(storage.size() + 1);
  for (auto& value : storage) result.push_back(value.data());
  result.push_back(nullptr); return result;
}
std::vector<char*> environment(std::vector<std::string>& storage,uint32_t memory_mb) {
  NSString* home = NSHomeDirectory(); NSString* temporary = NSTemporaryDirectory();
  if (![home isKindOfClass:NSString.class] || !home.length ||
      ![temporary isKindOfClass:NSString.class] || !temporary.length) return {};
  storage = {"HOME=" + std::string(home.fileSystemRepresentation),
      "TMPDIR=" + std::string(temporary.fileSystemRepresentation),
      "PATH=/usr/bin:/bin", "LANG=en_US.UTF-8", "LC_ALL=en_US.UTF-8",
      "OMNIPLAY_CHILD_MEMORY_MB="+std::to_string(memory_mb)};
  return pointers(storage);
}
std::optional<pid_t> spawn_child(const std::string& executable,
    const std::filesystem::path& directory, std::vector<std::string> arguments,
    const std::vector<std::pair<int, int>>& descriptors,uint32_t memory_mb) {
  // Allocate before initializing C spawn resources; no C++ throwing operation
  // may leak those owners or a newly created child.
  std::vector<std::string> environment_storage;
  auto env = environment(environment_storage,memory_mb); auto argv = pointers(arguments);
  if (env.empty()) return std::nullopt;
  Descriptor null(open("/dev/null", O_RDWR | O_CLOEXEC | O_NOFOLLOW));
  if (null.value < 0) return std::nullopt;
  posix_spawn_file_actions_t actions{}; posix_spawnattr_t attributes{};
  if (posix_spawn_file_actions_init(&actions) != 0) return std::nullopt;
  bool ready = posix_spawn_file_actions_addchdir_np(&actions, directory.c_str()) == 0;
  for (const auto& [source, target] : descriptors)
    if (ready && posix_spawn_file_actions_adddup2(&actions, source, target) != 0) ready = false;
  if (ready && posix_spawn_file_actions_adddup2(&actions, null.value, STDERR_FILENO) != 0) ready = false;
  if (posix_spawnattr_init(&attributes) != 0) {
    posix_spawn_file_actions_destroy(&actions); return std::nullopt;
  }
  sigset_t empty{}, defaults{};
  short flags = POSIX_SPAWN_CLOEXEC_DEFAULT | POSIX_SPAWN_SETPGROUP |
      POSIX_SPAWN_START_SUSPENDED | POSIX_SPAWN_SETSIGDEF | POSIX_SPAWN_SETSIGMASK;
  if (sigemptyset(&empty) != 0 || sigfillset(&defaults) != 0 ||
      posix_spawnattr_setsigmask(&attributes, &empty) != 0 ||
      posix_spawnattr_setsigdefault(&attributes, &defaults) != 0 ||
      posix_spawnattr_setflags(&attributes, flags) != 0 ||
      posix_spawnattr_setpgroup(&attributes, 0) != 0) ready = false;
  pid_t child = -1;
  const int result = ready ? posix_spawn(&child, executable.c_str(), &actions,
      &attributes, argv.data(), env.data()) : EINVAL;
  posix_spawnattr_destroy(&attributes); posix_spawn_file_actions_destroy(&actions);
  if (result != 0 || child <= 1) return std::nullopt;
  return child;
}
void monitor(const std::shared_ptr<Active>& active) {
  std::optional<Clock::time_point> deadline;
  for (;;) {
    struct kevent event{}; const timespec interval{0, 100 * 1000 * 1000};
    const int count = kevent(active->events.value, nullptr, 0, &event, 1, &interval);
    const int event_error = count < 0 ? errno : 0;
    bool ended = false;
    {
      std::lock_guard lock(active->lifecycle);
      if (count > 0 || (count < 0 && event_error != EINTR)) stop_locked(*active);
      ended = ended_locked(*active);
      if ((active->guest && (active->guest->ended() || active->guest->failed())) || active->presentation.reaped) stop_locked(*active);
      if (active->stopping && !deadline) deadline = Clock::now() + std::chrono::seconds(5);
    }
    if (ended) { forget_confirmed(active); return; }
    // An uncertain pair retains its session slot and child owners. Future
    // owner queries may retry WNOHANG; a cleanup thread never waits forever.
    if (deadline && Clock::now() >= *deadline) return;
    if (count < 0 && event_error != EINTR) std::this_thread::sleep_for(std::chrono::milliseconds(100));
  }
}
void start_monitor_locked(const std::shared_ptr<Active>& active) {
  if (active->monitor_started) return;
  try { std::thread([active] { monitor(active); }).detach(); active->monitor_started = true; }
  catch (...) { stop_locked(*active); }
}
void failed_launch(const std::shared_ptr<Active>& active) {
  bool ended = false;
  { std::lock_guard lock(active->lifecycle); stop_locked(*active);
    ended = ended_locked(*active); if (!ended) start_monitor_locked(active); }
  if (ended) forget_confirmed(active);
}
}

std::optional<LaunchEndpoints> launch_pair(const RuntimeSpec& runtime,
    SessionLease session, const LaunchParameters& parameters, uint64_t owner) {
  const auto directory = session.directory();
  if (!owner || !uuid(parameters.session_id) || !parameters.maximum_width ||
      parameters.maximum_width > 4096 || !parameters.maximum_height ||
      parameters.maximum_height > 4096 || parameters.volume > 100 ||
      !ordinary_private_directory(directory) || directory.filename() != parameters.session_id ||
      session.id() != parameters.session_id || session.runtime() != runtime.id)
    return std::nullopt;
  auto current = admit_runtime(runtime.id.c_str());
  if (!current || !same(runtime, *current)) return std::nullopt;
  auto active = std::make_shared<Active>(); active->session = parameters.session_id; active->owner = owner;
  active->storage.emplace(std::move(session));
  {
    // Reserve before descriptors/spawn. Launching, running and unconfirmed
    // retirement pairs all consume the same four process-owner slots.
    std::lock_guard lock(g_processes);
    if (g_active.size() >= 4 || g_active.contains(active->session)) return std::nullopt;
    uint64_t charged = 0;
    for (const auto& [_, entry] : g_active) charged += entry->reserved_mb;
    // Fixed guest RAM plus the declared helper address-space ceiling. Budget
    // is reserved while holding the global lock, including uncertain retirees.
    const uint64_t physical_mb = NSProcessInfo.processInfo.physicalMemory / (1024 * 1024);
    const uint64_t limit = std::min<uint64_t>(12288, physical_mb / 2);
    active->reserved_mb = static_cast<uint64_t>(runtime.guest.memory_mb) + runtime.presentation_memory_mb;
    if (active->reserved_mb > limit || charged > limit - active->reserved_mb) return std::nullopt;
    g_active.emplace(active->session, active);
  }
  try {
    auto broker_to_actor = make_pipe(); auto actor_to_broker = make_pipe();
    auto presentation_to_actor = make_pipe(); auto actor_to_presentation = make_pipe();
    auto lifetime=make_pipe();auto activation=make_pipe();
    if (!broker_to_actor || !actor_to_broker || !presentation_to_actor || !actor_to_presentation || !lifetime || !activation) {
      failed_launch(active); return std::nullopt;
    }
    std::unique_lock lock(active->lifecycle);
    if (active->stopping) throw std::runtime_error("macos_owner_unavailable");
    active->lifetime=std::move(lifetime->write);active->activation=std::move(activation->write);
    if(fcntl(active->activation.value,F_SETFL,O_NONBLOCK)!=0)throw std::runtime_error("macos_owner_unavailable");
    if (!active->storage->begin_launch()) throw std::runtime_error("macos_owner_unavailable");
    active->events = Descriptor(kqueue());
    if (active->events.value < 0 || fcntl(active->events.value, F_SETFD, FD_CLOEXEC) != 0)
      throw std::runtime_error("macos_owner_unavailable");
    auto fresh = admit_runtime(runtime.id.c_str());
    if (!fresh || !same(runtime, *fresh)) throw std::runtime_error("macos_owner_unavailable");
    auto presentation = spawn_child(runtime.presentation_path, directory,
        {runtime.presentation_path, "--stdio-presentation-v2-gated", parameters.session_id,
         std::to_string(parameters.maximum_width), std::to_string(parameters.maximum_height),
         std::to_string(parameters.volume), parameters.haptics ? "haptics" : "no-haptics",std::to_string(getpid())},
        {{actor_to_presentation->read.value, STDIN_FILENO}, {presentation_to_actor->write.value, STDOUT_FILENO},
         {activation->read.value,5},{lifetime->read.value,6}},runtime.presentation_memory_mb);
    if (!presentation) throw std::runtime_error("macos_owner_unavailable");
    active->presentation.pid = *presentation;
    active->presentation.identity = identity(*presentation);
    if (!active->presentation.identity || !register_exit(active->events.value, *presentation) ||
        !admit_owned_child(runtime, *presentation) || !same_identity(*active->presentation.identity))
      throw std::runtime_error("macos_owner_unavailable");
    fresh = admit_runtime(runtime.id.c_str());
    if (!fresh || !same(runtime, *fresh)) throw std::runtime_error("macos_owner_unavailable");
    active->guest = VirtualMachine::launch(runtime.guest, directory, parameters.session_id,
        broker_to_actor->read.value, actor_to_broker->write.value,
        presentation_to_actor->read.value, actor_to_presentation->write.value);
    if (!active->guest) throw std::runtime_error("macos_guest_unavailable");
    start_monitor_locked(active);
    if (!active->monitor_started || active->stopping)
      throw std::runtime_error("macos_owner_unavailable");
    if (kill(active->presentation.pid, SIGCONT) != 0)
      throw std::runtime_error("macos_owner_unavailable");
    return LaunchEndpoints{broker_to_actor->write.release(), actor_to_broker->read.release()};
  } catch (...) { failed_launch(active); return std::nullopt; }
}
bool session_active(const std::string& session) {
  std::shared_ptr<Active> active;
  { std::lock_guard lock(g_processes); auto found = g_active.find(session);
    if (found == g_active.end()) return false; active = found->second; }
  bool ended = false;
  { std::lock_guard lock(active->lifecycle);
    if (!active->stopping) return true;
    ended = ended_locked(*active); }
  if (ended) forget_confirmed(active);
  return !ended;
}
bool stop_session(const std::string& session, uint64_t owner) {
  if (!owner) return false;
  std::shared_ptr<Active> active;
  { std::lock_guard lock(g_processes); auto found = g_active.find(session);
    if (found != g_active.end() && found->second->owner == owner) active = found->second; }
  if (active) { std::lock_guard lock(active->lifecycle); stop_locked(*active); }
  return static_cast<bool>(active);
}
bool activate_session(const std::string& session,uint64_t owner){
  if(!owner)return false;std::shared_ptr<Active> active;
  {std::lock_guard lock(g_processes);auto found=g_active.find(session);if(found!=g_active.end()&&found->second->owner==owner)active=found->second;}
  if(!active)return false;std::lock_guard lock(active->lifecycle);
  if(active->stopping||active->activation.value<0||active->activation_sequence>=0x7fffffffffffffffULL)return false;
  std::array<unsigned char,40> packet{'O','P','O','W','N','E','R','1'};
  auto digit=[](char value)->unsigned char{return value<='9'?value-'0':value-'a'+10;};
  size_t byte=8;for(size_t i=0;i<session.size();){if(session[i]=='-'){++i;continue;}packet[byte++]=static_cast<unsigned char>((digit(session[i])<<4)|digit(session[i+1]));i+=2;}
  const uint64_t sequence=++active->activation_sequence;
  for(size_t i=0;i<8;++i)packet[24+i]=static_cast<unsigned char>(sequence>>(8*i));
  packet[32]=1;ssize_t count;
  do{count=write(active->activation.value,packet.data(),packet.size());}while(count<0&&errno==EINTR);
  return count==static_cast<ssize_t>(packet.size());
}
void stop_connection(uint64_t owner) {
  std::array<std::shared_ptr<Active>, 4> matches{}; size_t count = 0;
  { std::lock_guard lock(g_processes);
    for (const auto& [_, active] : g_active) if (active->owner == owner) matches[count++] = active; }
  for (size_t index = 0; index < count; ++index) {
    std::lock_guard lock(matches[index]->lifecycle); stop_locked(*matches[index]);
  }
}
LaunchEndpoints::LaunchEndpoints(int in, int out) noexcept : input(in), output(out) {}
LaunchEndpoints::~LaunchEndpoints() { if (input >= 0) close(input); if (output >= 0) close(output); }
LaunchEndpoints::LaunchEndpoints(LaunchEndpoints&& other) noexcept
    : input(std::exchange(other.input, -1)), output(std::exchange(other.output, -1)) {}
LaunchEndpoints& LaunchEndpoints::operator=(LaunchEndpoints&& other) noexcept {
  if (this != &other) {
    if (input >= 0) close(input); if (output >= 0) close(output);
    input = std::exchange(other.input, -1); output = std::exchange(other.output, -1);
  }
  return *this;
}
int LaunchEndpoints::take_input() noexcept { return std::exchange(input, -1); }
int LaunchEndpoints::take_output() noexcept { return std::exchange(output, -1); }
}
