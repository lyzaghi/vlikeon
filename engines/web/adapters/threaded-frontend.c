/* SPDX-License-Identifier: MIT
 * Typed libretro callbacks for the OmniPlay pthread frontend protocol 1.
 * Each address is compiled into the shared Wasm function table; JS addFunction
 * closures from the parent instance are never called through a child instance.
 */
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <emscripten.h>
#include <emscripten/threading.h>
#ifndef __EMSCRIPTEN_PTHREADS__
#error "The threaded callback frontend requires an Emscripten pthread build"
#endif

static bool om_thread_environment(unsigned command, void *data) {
    return MAIN_THREAD_EM_ASM_INT({
        const callbacks = Module['omniplayCallbacksV1'];
        return callbacks ? callbacks.environment($0 >>> 0, $1 >>> 0) : 0;
    }, command, data) != 0;
}
static void om_thread_video(const void *data, unsigned width, unsigned height, size_t pitch) {
    MAIN_THREAD_EM_ASM({
        const callbacks = Module['omniplayCallbacksV1'];
        if (callbacks) callbacks.video_refresh($0 >>> 0, $1 >>> 0, $2 >>> 0, $3 >>> 0);
    }, data, width, height, pitch);
}
static void om_thread_audio_sample(int16_t left, int16_t right) {
    MAIN_THREAD_EM_ASM({
        const callbacks = Module['omniplayCallbacksV1'];
        if (callbacks) callbacks.audio_sample($0 | 0, $1 | 0);
    }, left, right);
}
static size_t om_thread_audio_batch(const int16_t *samples, size_t frames) {
    return (size_t)MAIN_THREAD_EM_ASM_INT({
        const callbacks = Module['omniplayCallbacksV1'];
        return callbacks ? callbacks.audio_sample_batch($0 >>> 0, $1 >>> 0) : 0;
    }, samples, frames);
}
static void om_thread_input_poll(void) {
    MAIN_THREAD_EM_ASM({
        const callbacks = Module['omniplayCallbacksV1'];
        if (callbacks) callbacks.input_poll();
    });
}
static int16_t om_thread_input_state(unsigned port, unsigned device, unsigned index, unsigned id) {
    return (int16_t)MAIN_THREAD_EM_ASM_INT({
        const callbacks = Module['omniplayCallbacksV1'];
        return callbacks ? callbacks.input_state($0 >>> 0, $1 >>> 0, $2 >>> 0, $3 >>> 0) : 0;
    }, port, device, index, id);
}
static bool om_thread_rumble(unsigned port, unsigned effect, uint16_t strength) {
    return MAIN_THREAD_EM_ASM_INT({
        const callbacks = Module['omniplayCallbacksV1'];
        return callbacks ? callbacks.rumble($0 >>> 0, $1 >>> 0, $2 >>> 0) : 0;
    }, port, effect, strength) != 0;
}
EMSCRIPTEN_KEEPALIVE unsigned om_web_thread_abi(void) {
    return emscripten_is_main_runtime_thread() ? 1u : 0u;
}
EMSCRIPTEN_KEEPALIVE uintptr_t om_web_thread_callback_pointer(unsigned operation) {
    if (!emscripten_is_main_runtime_thread()) return 0;
    switch (operation) {
    case 0: return (uintptr_t)om_thread_environment;
    case 1: return (uintptr_t)om_thread_video;
    case 2: return (uintptr_t)om_thread_audio_sample;
    case 3: return (uintptr_t)om_thread_audio_batch;
    case 4: return (uintptr_t)om_thread_input_poll;
    case 5: return (uintptr_t)om_thread_input_state;
    case 6: return (uintptr_t)om_thread_rumble;
    default: return 0;
    }
}
