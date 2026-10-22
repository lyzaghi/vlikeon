#ifndef OMNIPLAY_LINUX_NAMESPACES_H
#define OMNIPLAY_LINUX_NAMESPACES_H
#include <stdbool.h>
#include <stdint.h>
#include <sys/types.h>
#include "root.h"

struct omni_linux_namespace_plan {
  pid_t expected_parent;
  int cgroup_fd;unsigned cpu_cores;
  uint64_t memory_bytes,maximum_file_bytes,root_bytes,scratch_bytes,save_bytes;
  char session[37];
  /* Owned admitted data, never a renderer-provided pointer or constructor. The
   * fixed root implementation consumes its exact images/input/root FD plan. */
  const struct omni_linux_root_plan* root_plan;
};

/* Trusted, unprivileged, single-threaded bootstrap only. FDs 0/1 are actor
 * control read/write pipes, 2 is write-only /dev/null, 3/4 are presentation
 * read/write pipes. All other FDs must belong to the closed root plan and are
 * scrubbed before the actor. Starts and reaps an init/actor chain; never returns.
 * A broker still needs an exact executable/FD/error channel, pidfd/process
 * ownership, delegated cgroup lifecycle, journal and presentation helper before
 * Linux may be enabled. The admitted leaf receives and contains this chain.
 */
_Noreturn void omni_linux_namespace_supervise(const struct omni_linux_namespace_plan*);
#endif
