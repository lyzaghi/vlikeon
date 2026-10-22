#ifndef OMNIPLAY_LINUX_CGROUP_H
#define OMNIPLAY_LINUX_CGROUP_H
#include <stdint.h>

/* An empty fresh leaf, owned 0700 by the current unprivileged user, under the
 * broker's existing legitimate cgroup-v2 delegation. Only its own controls are
 * changed. No controller is enabled on a host/ancestor tree and no permission
 * is changed. Missing delegation/controllers/readback exits 125. Consumes fd.
 */
void omni_linux_cgroup_enter(int fd,const char* session,uint64_t memory_bytes,uint64_t root_bytes,uint64_t scratch_bytes,uint64_t save_bytes,unsigned cpu_cores);
#endif
