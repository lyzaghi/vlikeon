/* ISC, copyright (c) 2026 OmniPlay contributors. See LICENSE.libco.
 * Single-owner native libco backend. Opaque handle identity survives derive;
 * Windows owns all guest stacks and no executable trampoline is generated.
 */
#ifndef _WIN32
#error This backend requires Windows fibers
#endif
#ifndef _WIN32_WINNT
#define _WIN32_WINNT 0x0602
#endif
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <stdlib.h>
#include "libco.h"

enum { OM_CO_LIMIT = 64, OM_CO_STACK_MIN = 256 * 1024,
       OM_CO_STACK_MAX = 4 * 1024 * 1024, OM_CO_COMMIT = 64 * 1024 };
typedef struct om_native_co {
  void *fiber;
  void (*entry)(void);
  struct om_native_co *next;
} om_native_co;
static om_native_co om_root;
static om_native_co *om_active;
static om_native_co *om_owned;
static DWORD om_owner;
static unsigned om_count;

static void om_thread(void) {
  if (om_owner && om_owner != GetCurrentThreadId()) abort();
}
static om_native_co *om_known(cothread_t handle) {
  om_native_co *item;
  if (handle == &om_root && om_active) return &om_root;
  for (item = om_owned; item; item = item->next)
    if (item == handle) return item;
  return NULL;
}
static VOID WINAPI om_entry(LPVOID opaque) {
  om_native_co *item = (om_native_co *)opaque;
  om_thread();
  if (item != om_active || !item->entry) abort();
  item->entry();
  abort();
}
cothread_t co_active(void) {
  om_thread();
  if (!om_active) {
    om_owner = GetCurrentThreadId();
    om_root.fiber = IsThreadAFiber() ? GetCurrentFiber() :
        ConvertThreadToFiberEx(NULL, FIBER_FLAG_FLOAT_SWITCH);
    if (!om_root.fiber) abort();
    om_active = &om_root;
  }
  if (GetCurrentFiber() != om_active->fiber) abort();
  return om_active;
}
static void *om_create_fiber(om_native_co *item, unsigned int heapsize) {
  SIZE_T size;
  void *fiber;
  if (!heapsize || heapsize > OM_CO_STACK_MAX) abort();
  size = heapsize < OM_CO_STACK_MIN ? OM_CO_STACK_MIN : heapsize;
  fiber = CreateFiberEx(OM_CO_COMMIT, size, FIBER_FLAG_FLOAT_SWITCH, om_entry, item);
  if (!fiber) abort();
  return fiber;
}
cothread_t co_create(unsigned int heapsize, void (*entry)(void)) {
  om_native_co *item;
  co_active();
  if (!entry || om_count >= OM_CO_LIMIT) abort();
  item = (om_native_co *)calloc(1, sizeof(*item));
  if (!item) abort();
  item->entry = entry;
  item->fiber = om_create_fiber(item, heapsize);
  item->next = om_owned; om_owned = item; ++om_count;
  return item;
}
cothread_t co_derive(void *handle, unsigned int heapsize, void (*entry)(void)) {
  om_native_co *item;
  void *previous;
  co_active();
  item = om_known(handle);
  if (!item || item == &om_root || item == om_active || !entry) abort();
  previous = item->fiber;
  item->fiber = om_create_fiber(item, heapsize);
  item->entry = entry;
  DeleteFiber(previous);
  return item;
}
void co_delete(cothread_t handle) {
  om_native_co **cursor = &om_owned;
  om_native_co *item;
  co_active();
  while (*cursor && *cursor != handle) cursor = &(*cursor)->next;
  if (!*cursor || *cursor == om_active) abort();
  item = *cursor; *cursor = item->next;
  DeleteFiber(item->fiber); free(item); --om_count;
}
void co_switch(cothread_t handle) {
  om_native_co *from;
  om_native_co *to;
  co_active();
  from = om_active; to = om_known(handle);
  if (!to) abort();
  if (from == to) return;
  om_active = to;
  SwitchToFiber(to->fiber);
  om_active = from;
}
int co_serializable(void) { return 0; }
