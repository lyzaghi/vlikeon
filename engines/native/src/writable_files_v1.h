#ifndef OMNIPLAY_WRITABLE_FILES_V1_H
#define OMNIPLAY_WRITABLE_FILES_V1_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif

enum OmniFilesResult {
  OMNI_FILES_OK=0,OMNI_FILES_NOT_FOUND=1,OMNI_FILES_DENIED=2,
  OMNI_FILES_INVALID=3,OMNI_FILES_NO_SPACE=4,OMNI_FILES_NO_MEMORY=5,
  OMNI_FILES_BUSY=6,OMNI_FILES_EXISTS=7,OMNI_FILES_STALE=8,
  OMNI_FILES_NOT_EMPTY=9,OMNI_FILES_IS_DIRECTORY=10,OMNI_FILES_INTERNAL=11
};
enum OmniFilesMode {
  OMNI_FILES_READ=1,OMNI_FILES_WRITE=2,OMNI_FILES_CREATE=4,
  OMNI_FILES_TRUNCATE=8,OMNI_FILES_EXCLUSIVE=16
};
enum OmniFilesKind {OMNI_FILES_REGULAR=1,OMNI_FILES_DIRECTORY=2};
#define OMNI_FILES_MAX_TRANSFER (8u*1024u*1024u)
struct OmniFilesStatV1 {uint32_t kind;uint32_t reserved;uint64_t bytes;uint64_t allocated_bytes;};
struct OmniFilesEntryV1 {struct OmniFilesStatV1 stat;uint32_t name_bytes;char name[256];};
struct OmniFilesUsageV1 {
  uint64_t save_allocated_bytes;uint64_t scratch_allocated_bytes;uint64_t revision;
  uint32_t files;uint32_t directories;uint32_t open_handles;uint32_t reserved;
};
/* This table covers only save/ and scratch/. It never opens host filesystem
 * paths. Handles are monotonic opaque integers, not process/file pointers.
 * All writes are complete or unchanged; each transfer is at most 8 MiB.
 * Paths are bounded UTF-8 relative names (1024 bytes, 16 components), under
 * save/ or scratch/; files have a 256-MiB logical bound. Page-rounded payload
 * limits are 272 MiB save and 64 MiB scratch. Node/handle metadata is bounded
 * separately: 4096 files, 8224 nodes (including roots/ancestors) and 256
 * simultaneously open handles. Save envelopes retain their 8192-entry bound.
 * flush acknowledges memory visibility, not durable broker save publication.
 * list returns at most 64 immediate children. expected_revision=0 starts a
 * listing; subsequent batches must retain the returned revision or get STALE.
 * The core must retire all callers before uninstalling/replacing the table. */
struct OmniWritableFilesV1 {
  uint32_t version;uint32_t struct_bytes;void* context;
  int (*stat)(void*,const char*,struct OmniFilesStatV1*);
  int (*mkdir)(void*,const char*,uint32_t recursive);
  int (*open)(void*,const char*,uint32_t mode,uint64_t* handle);
  int (*close)(void*,uint64_t handle);
  int (*read)(void*,uint64_t handle,uint64_t offset,void*,uint64_t bytes,uint64_t* actual);
  int (*write)(void*,uint64_t handle,uint64_t offset,const void*,uint64_t bytes,uint64_t* actual);
  int (*resize)(void*,uint64_t handle,uint64_t bytes);
  int (*flush)(void*,uint64_t handle);
  int (*remove)(void*,const char*);
  int (*rename)(void*,const char*,const char*,uint32_t replace);
  int (*list)(void*,const char*,uint64_t expected_revision,uint32_t offset,
              struct OmniFilesEntryV1*,uint32_t capacity,uint32_t* actual,uint64_t* revision);
  int (*usage)(void*,struct OmniFilesUsageV1*);
};
/* A qualified core exports this function. Success is exactly 1; a null table
 * uninstalls only after every guest producer/handle has retired. No fallback
 * from this contract to ordinary OS writes is allowed in the confined mode. */
typedef int (*OmniInstallWritableFilesV1)(const struct OmniWritableFilesV1*);
#ifdef __cplusplus
}
#endif
#endif
