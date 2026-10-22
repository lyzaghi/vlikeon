#ifndef OMNIPLAY_LINUX_POLICY_H
#define OMNIPLAY_LINUX_POLICY_H
#include <stddef.h>
#include <stdint.h>

#define OMNI_LINUX_PATHS 24
enum omni_linux_access { OMNI_LINUX_READ_FILE=1,OMNI_LINUX_READ_DIRECTORY=2,OMNI_LINUX_WRITE_DIRECTORY=3,OMNI_LINUX_EXECUTABLE=4,OMNI_LINUX_NULL_DEVICE=5 };
struct omni_linux_path { int fd;enum omni_linux_access access; };
struct omni_linux_policy {
  uint64_t memory_bytes;
  uint64_t maximum_file_bytes;
  int executable_fd;
  size_t path_count;
  struct omni_linux_path paths[OMNI_LINUX_PATHS];
};

/* Trusted single-threaded bootstrap only, before loading any core. The caller
 * must already own a private namespace/root, remove capabilities, admit exact
 * inherited pipe/program FDs, close every other authority-bearing FD, and build
 * the fixed actor argv/environment. Path FDs must be O_PATH|O_CLOEXEC in that
 * approved root. Only executable_fd remains until execveat. This never returns;
 * any missing policy/kernel operation exits 125. It is not a standalone sandbox.
 */
_Noreturn void omni_linux_policy_exec(const struct omni_linux_policy*,char*const argv[],char*const environment[]);
#endif
