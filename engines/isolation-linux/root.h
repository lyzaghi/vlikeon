#ifndef OMNIPLAY_LINUX_ROOT_H
#define OMNIPLAY_LINUX_ROOT_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define OMNI_LINUX_ROOT_FILES 256
enum omni_linux_root_role{OMNI_LINUX_IMAGE_FILE=1,OMNI_LINUX_GAME_INPUT=2};
struct omni_linux_root_file{
  int fd;enum omni_linux_root_role role;uint64_t bytes;bool executable;
  char path[512];
};
struct omni_linux_root_plan{
  int empty_directory_fd;
  size_t file_count;
  struct omni_linux_root_file files[OMNI_LINUX_ROOT_FILES];
};

/* Only reviewed single-threaded namespace-init code may call this. Image FDs
 * must be sealed immutable regular files whose exact bytes were authenticated
 * against the signed package by the broker. Game FDs are the broker's exclusive
 * immutable private staging copies, never arbitrary renderer-selected paths.
 * Creates only private namespace mounts over the caller's empty 0700 directory;
 * any failed admission or kernel operation exits 125. There is no host path API.
 */
void omni_linux_construct_root(const struct omni_linux_root_plan*,uint64_t root_bytes,uint64_t scratch_bytes,uint64_t save_bytes);
#endif
