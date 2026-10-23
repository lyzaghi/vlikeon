// PID 1 of the fixed-memory, diskless ARM64 guest. The host never executes a
// core process. Guest root has no writable host filesystem or network device.
#include <ctype.h>
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/reboot.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static void finish(void) {
  // No sync of host-writable storage is necessary: none exists.
  reboot(RB_POWER_OFF);
  for (;;) pause();
}
static bool read_text(const char* path, char* value, size_t capacity) {
  int fd = open(path, O_RDONLY | O_CLOEXEC | O_NOFOLLOW);
  if (fd < 0) return false;
  ssize_t count = read(fd, value, capacity);
  close(fd);
  if (count < 0 || (size_t)count >= capacity) return false;
  value[count] = '\0';
  return true;
}
static bool uuid(const char* value) {
  if (strlen(value) != 36) return false;
  for (size_t i = 0; i < 36; ++i) {
    if (i == 8 || i == 13 || i == 18 || i == 23) { if (value[i] != '-') return false; }
    else if (!((value[i] >= '0' && value[i] <= '9') || (value[i] >= 'a' && value[i] <= 'f'))) return false;
  }
  return true;
}
static int find_port(const char* wanted) {
  DIR* directory = opendir("/sys/class/virtio-ports");
  if (!directory) return -1;
  int result = -1;
  unsigned count = 0;
  struct dirent* entry;
  while ((entry = readdir(directory))) {
    if (entry->d_name[0] == '.') continue;
    if (++count > 8) { if (result >= 0) close(result); result = -1; break; }
    bool valid = true;
    for (const char* c = entry->d_name; *c; ++c)
      if (!isalnum((unsigned char)*c) && *c != '.') valid = false;
    if (!valid) continue;
    char path[512], name[128];
    snprintf(path, sizeof(path), "/sys/class/virtio-ports/%s/name", entry->d_name);
    if (!read_text(path, name, sizeof(name))) continue;
    name[strcspn(name, "\r\n")] = '\0';
    if (strcmp(name, wanted)) continue;
    if (result >= 0) { close(result); result = -1; break; }
    snprintf(path, sizeof(path), "/dev/%s", entry->d_name);
    result = open(path, O_RDWR | O_CLOEXEC | O_NOFOLLOW);
    struct stat shape;
    if (result >= 0 && (fstat(result, &shape) || !S_ISCHR(shape.st_mode))) { close(result); result = -1; }
  }
  closedir(directory);
  return result;
}
int main(void) {
  if (getpid() != 1) return 125;
  umask(0077);
  if (mount("proc", "/proc", "proc", MS_NOSUID | MS_NODEV | MS_NOEXEC, NULL) ||
      mount("sysfs", "/sys", "sysfs", MS_NOSUID | MS_NODEV | MS_NOEXEC, NULL) ||
      mount("devtmpfs", "/dev", "devtmpfs", MS_NOSUID | MS_NOEXEC, NULL)) finish();
  char command[4096], session[37] = {0}, scratch[16] = {0};
  if (!read_text("/proc/cmdline", command, sizeof(command))) finish();
  char* context = NULL;
  for (char* word = strtok_r(command, " \n", &context); word; word = strtok_r(NULL, " \n", &context)) {
    if (!strncmp(word, "vlike.session=", 14)) {
      if (session[0] || strlen(word + 14) != 36) finish();
      memcpy(session, word + 14, 37);
    }
    if (!strncmp(word, "vlike.scratch_mb=", 17)) {
      if (scratch[0] || strlen(word + 17) >= sizeof(scratch)) finish();
      strcpy(scratch, word + 17);
    }
  }
  if (!uuid(session) || !scratch[0]) finish();
  for (const char* c = scratch; *c; ++c) if (*c < '0' || *c > '9') finish();
  unsigned long mb = strtoul(scratch, NULL, 10);
  if (mb < 16 || mb > 512) finish();
  char options[64];
  snprintf(options, sizeof(options), "size=%luM,mode=0700", mb);
  if (mount("vlike-input", "/inputs", "virtiofs", MS_RDONLY | MS_NODEV | MS_NOSUID | MS_NOEXEC, NULL) ||
      mount("tmpfs", "/scratch", "tmpfs", MS_NODEV | MS_NOSUID | MS_NOEXEC, options) ||
      mkdir("/scratch/upper", 0700) || mkdir("/scratch/work", 0700) ||
      mount("overlay", "/work", "overlay", MS_NODEV | MS_NOSUID | MS_NOEXEC,
          "lowerdir=/inputs,upperdir=/scratch/upper,workdir=/scratch/work")) finish();
  int control = -1, presentation = -1;
  // Device enumeration may follow initial PID-1 startup. Bound discovery to
  // 15 seconds of monotonic sleep; startup failure powers off the entire guest.
  for (unsigned attempt = 0; attempt < 150 && (control < 0 || presentation < 0); ++attempt) {
    if (control < 0) control = find_port("vlike-control");
    if (presentation < 0) presentation = find_port("vlike-presentation");
    struct timespec interval = {0, 100000000};
    if (control < 0 || presentation < 0) nanosleep(&interval, NULL);
  }
  if (control < 0 || presentation < 0) finish();
  // Move originals above the protocol's fixed slots before duplicating.
  int c = fcntl(control, F_DUPFD_CLOEXEC, 10), p = fcntl(presentation, F_DUPFD_CLOEXEC, 10);
  close(control); close(presentation);
  if (c < 10 || p < 10) finish();
  pid_t worker = fork();
  if (worker < 0) finish();
  if (!worker) {
    if (dup2(c, 0) < 0 || dup2(c, 1) < 0 || dup2(p, 3) < 0 || dup2(p, 4) < 0) _exit(125);
    close(c); close(p);
    int null = open("/dev/null", O_WRONLY | O_CLOEXEC);
    // dup2(fd, fd) preserves CLOEXEC. PID 1 may start with stderr closed,
    // allowing open() to return 2; keep that /dev/null across exec explicitly.
    if (null < 0 || dup2(null, 2) < 0 || fcntl(2, F_SETFD, 0) < 0) _exit(125);
    if (null > 4) close(null);
    if (chdir("/work")) _exit(125);
    char* argv[] = {"/opt/vlike/runtime/omniplay-native-worker", "--stdio-runtime-v1",
        "--presentation-client-v2", session, "3", "4", NULL};
    char* env[] = {"PATH=/bin", "HOME=/work", "TMPDIR=/scratch", "LANG=C", "LC_ALL=C", NULL};
    execve(argv[0], argv, env);
    _exit(125);
  }
  close(c); close(p);
  for (;;) {
    int status;
    pid_t exited = waitpid(-1, &status, 0);
    if (exited == worker || (exited < 0 && errno != EINTR)) finish();
  }
}
