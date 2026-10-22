/* OmniPlay libco backend for the pinned bsnes source (ISC; see LICENSE.libco).
 * These are cooperative Asyncify fibers on ONE core worker, never OS threads.
 * No host stack pointer, native machine-code trampoline or JIT is serialized.
 */
#include "libco.h"
#include <emscripten/fiber.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

#if !defined(__EMSCRIPTEN__) || defined(__EMSCRIPTEN_PTHREADS__)
#error This adapter requires the single-threaded Emscripten Asyncify profile
#endif

enum {
  OM_CO_LIMIT = 64,
  OM_CO_C_MIN = 256 * 1024,
  OM_CO_C_MAX = 2 * 1024 * 1024,
  OM_CO_ASYNC = 512 * 1024,
  OM_CO_ROOT_ASYNC = 1024 * 1024
};

typedef struct om_co {
  emscripten_fiber_t fiber;
  void *c_stack;
  void *async_stack;
  size_t c_bytes;
  void (*entry)(void);
  struct om_co *next;
} om_co;

static om_co om_root;
static om_co *om_active;
static om_co *om_owned;
static unsigned om_count;

static om_co *om_known(cothread_t handle) {
  om_co *item;
  if (handle == &om_root && om_active) return &om_root;
  for (item = om_owned; item; item = item->next)
    if (handle == item) return item;
  return NULL;
}

static void om_entry(void *opaque) {
  om_co *item = (om_co *)opaque;
  if (om_active != item || !item->entry) abort();
  item->entry();
  /* Returning would end the Emscripten program, rather than return to libco. */
  abort();
}

cothread_t co_active(void) {
  if (!om_active) {
    om_root.async_stack = malloc(OM_CO_ROOT_ASYNC);
    if (!om_root.async_stack) abort();
    emscripten_fiber_init_from_current_context(&om_root.fiber,
        om_root.async_stack, OM_CO_ROOT_ASYNC);
    om_active = &om_root;
  }
  return om_active;
}

static void om_initialize(om_co *item, void (*entry)(void)) {
  memset(&item->fiber, 0, sizeof(item->fiber));
  memset(item->c_stack, 0, item->c_bytes);
  memset(item->async_stack, 0, OM_CO_ASYNC);
  item->entry = entry;
  emscripten_fiber_init(&item->fiber, om_entry, item,
      item->c_stack, item->c_bytes, item->async_stack, OM_CO_ASYNC);
}

cothread_t co_create(unsigned int heapsize, void (*entry)(void)) {
  om_co *item;
  size_t size;
  co_active();
  if (!entry || !heapsize || heapsize > OM_CO_C_MAX || om_count >= OM_CO_LIMIT)
    abort();
  size = heapsize < OM_CO_C_MIN ? OM_CO_C_MIN : heapsize;
  size = (size + 15u) & ~(size_t)15u;
  item = (om_co *)calloc(1, sizeof(*item));
  if (!item) abort();
  item->c_stack = aligned_alloc(16, size);
  item->async_stack = malloc(OM_CO_ASYNC);
  if (!item->c_stack || !item->async_stack) {
    free(item->c_stack); free(item->async_stack); free(item); abort();
  }
  item->c_bytes = size;
  item->next = om_owned;
  om_owned = item;
  ++om_count;
  om_initialize(item, entry);
  return item;
}

cothread_t co_derive(void *memory, unsigned int heapsize, void (*entry)(void)) {
  /* bsnes re-powers/reloads by deriving an existing opaque handle. It is not
   * supplying an arbitrary external stack allocation in this adapter. */
  om_co *item = om_known(memory);
  if (!item || item == &om_root || item == om_active || !entry ||
      !heapsize || heapsize > item->c_bytes) abort();
  om_initialize(item, entry);
  return item;
}

void co_delete(cothread_t handle) {
  om_co **cursor = &om_owned;
  while (*cursor && *cursor != handle) cursor = &(*cursor)->next;
  if (!*cursor || *cursor == om_active) abort();
  {
    om_co *item = *cursor;
    *cursor = item->next;
    free(item->c_stack); free(item->async_stack); free(item);
  }
  --om_count;
}

void co_switch(cothread_t handle) {
  om_co *from;
  om_co *to;
  co_active();
  from = om_active;
  to = om_known(handle);
  if (!to) abort();
  if (from == to) return;
  om_active = to;
  emscripten_fiber_swap(&from->fiber, &to->fiber);
  om_active = from;
}

int co_serializable(void) {
  /* Force upstream runToSave and synchronized chip state serialization. The
   * opaque pointers/Asyncify continuation bytes are not a stable save ABI. */
  return 0;
}
