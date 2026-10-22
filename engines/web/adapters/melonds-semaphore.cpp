// SPDX-License-Identifier: GPL-3.0-or-later
// The software renderer is fixed to synchronous execution. No producer can
// arrive while this Worker waits; an unexpected blocking wait is a fatal error.
#include <Platform.h>
#include <cstdint>
#include <cstdlib>
#include <limits>
namespace melonDS::Platform {
struct Semaphore { uint32_t count = 0; };
Semaphore* Semaphore_Create() { return new Semaphore; }
void Semaphore_Free(Semaphore* sema) { delete sema; }
void Semaphore_Reset(Semaphore* sema) { if(!sema) std::abort(); sema->count = 0; }
bool Semaphore_TryWait(Semaphore* sema, int timeout_ms) {
    if(!sema || timeout_ms < 0) std::abort();
    if(!sema->count) return false;
    --sema->count; return true;
}
void Semaphore_Post(Semaphore* sema, int count) {
    if(!sema || count < 0 || static_cast<uint32_t>(count) >
       std::numeric_limits<uint32_t>::max() - sema->count) std::abort();
    sema->count += static_cast<uint32_t>(count);
}
void Semaphore_Wait(Semaphore* sema) {
    if(!Semaphore_TryWait(sema, 0)) std::abort();
}
}
