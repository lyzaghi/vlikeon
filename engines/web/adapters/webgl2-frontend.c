/* SPDX-License-Identifier: MIT
 * One Worker owns one WebGL2 context and a bounded private libretro target.
 * Guest reset/destroy callbacks are called only through the void completion
 * export; a failed callback is never retried after the Worker is retired.
 */
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <emscripten.h>
#include <emscripten/html5.h>
#include <emscripten/html5_webgl.h>
#include <GLES3/gl3.h>
#include "libretro.h"

#if !defined(__EMSCRIPTEN__) || defined(__EMSCRIPTEN_PTHREADS__)
#error This frontend requires a single synchronous Emscripten Worker
#endif

static EMSCRIPTEN_WEBGL_CONTEXT_HANDLE om_context;
static struct retro_hw_render_callback om_callbacks;
static GLuint om_framebuffer, om_color, om_depth;
static unsigned om_width, om_height;
static bool om_registered, om_reset_armed, om_destroyed, om_cleanup_failed;
static int om_power;
static const char om_target[] = "!omniplay";

/* Context creation's library dependencies include specialHTMLTargets. This
 * explicit mapping avoids querying a DOM that does not exist in a Worker. */
EM_JS(int, om_hw_map_canvas, (), {
    const canvas = Module['canvas'];
    if (typeof OffscreenCanvas !== 'function' || !(canvas instanceof OffscreenCanvas)) return 0;
    specialHTMLTargets['!omniplay'] = canvas;
    return 1;
});
EM_JS(void, om_hw_unmap_canvas, (), {
    delete specialHTMLTargets['!omniplay'];
});

unsigned om_web_hw_callback_bytes(void) { return sizeof(struct retro_hw_render_callback); }
int om_web_hw_alive(void) {
    return om_context && !emscripten_is_webgl_context_lost(om_context) &&
        emscripten_webgl_make_context_current(om_context) == EMSCRIPTEN_RESULT_SUCCESS;
}
static uintptr_t om_get_framebuffer(void) { return om_framebuffer; }
static retro_proc_address_t om_get_proc(const char* name) {
    if (!name || !om_web_hw_alive()) return NULL;
    /* This is a WASM function-table entry, never a native driver pointer. */
    return (retro_proc_address_t)emscripten_webgl_get_proc_address(name);
}
static int om_close_context(void) {
    if (om_web_hw_alive()) {
        if (om_depth) glDeleteRenderbuffers(1, &om_depth);
        if (om_color) glDeleteTextures(1, &om_color);
        if (om_framebuffer) glDeleteFramebuffers(1, &om_framebuffer);
    }
    om_framebuffer = om_color = om_depth = 0;
    if (om_context && emscripten_webgl_destroy_context(om_context) != EMSCRIPTEN_RESULT_SUCCESS)
        om_cleanup_failed = true;
    om_context = 0;
    return !om_cleanup_failed;
}
int om_web_hw_prepare(unsigned width, unsigned height, int power) {
    if (om_context || om_registered || !width || !height || width > 1920 || height > 1080 ||
        (uint64_t)width * height > 1920u * 1080u || power < 0 || power > 1 || !om_hw_map_canvas()) return 0;
    om_width = width; om_height = height; om_power = power;
    om_destroyed = false; om_reset_armed = false; om_cleanup_failed = false;
    return 1;
}
int om_web_hw_register(struct retro_hw_render_callback* request) {
    if (!request || !om_width || !om_height || om_registered || om_context ||
        !request->context_reset || !request->bottom_left_origin || request->debug_context ||
        (request->stencil && !request->depth)) return 0;
    if (request->context_type != RETRO_HW_CONTEXT_OPENGLES3 &&
        !(request->context_type == RETRO_HW_CONTEXT_OPENGLES_VERSION &&
          request->version_major == 3 && request->version_minor == 0)) return 0;
    if (request->context_type == RETRO_HW_CONTEXT_OPENGLES3 &&
        (request->version_minor != 0 || (request->version_major != 0 && request->version_major != 3))) return 0;
    EmscriptenWebGLContextAttributes attributes;
    emscripten_webgl_init_context_attributes(&attributes);
    attributes.majorVersion = 2; attributes.minorVersion = 0;
    attributes.alpha = false; attributes.depth = false; attributes.stencil = false;
    attributes.antialias = false; attributes.premultipliedAlpha = false;
    attributes.preserveDrawingBuffer = false; attributes.explicitSwapControl = true;
    attributes.renderViaOffscreenBackBuffer = true;
    attributes.proxyContextToMainThread = EMSCRIPTEN_WEBGL_CONTEXT_PROXY_DISALLOW;
    attributes.powerPreference = om_power ? EM_WEBGL_POWER_PREFERENCE_LOW_POWER : EM_WEBGL_POWER_PREFERENCE_DEFAULT;
    if (emscripten_set_canvas_element_size(om_target, (int)om_width, (int)om_height) != EMSCRIPTEN_RESULT_SUCCESS) return 0;
    om_context = emscripten_webgl_create_context(om_target, &attributes);
    if (!om_web_hw_alive()) { om_close_context(); return 0; }
    GLint maximum_texture = 0, maximum_renderbuffer = 0;
    glGetIntegerv(GL_MAX_TEXTURE_SIZE, &maximum_texture);
    glGetIntegerv(GL_MAX_RENDERBUFFER_SIZE, &maximum_renderbuffer);
    if (maximum_texture < (GLint)om_width || maximum_texture < (GLint)om_height ||
        maximum_renderbuffer < (GLint)om_width || maximum_renderbuffer < (GLint)om_height) {
        om_close_context(); return 0;
    }
    glGenFramebuffers(1, &om_framebuffer);
    glBindFramebuffer(GL_FRAMEBUFFER, om_framebuffer);
    glGenTextures(1, &om_color);
    glBindTexture(GL_TEXTURE_2D, om_color);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MIN_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_MAG_FILTER, GL_NEAREST);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_S, GL_CLAMP_TO_EDGE);
    glTexParameteri(GL_TEXTURE_2D, GL_TEXTURE_WRAP_T, GL_CLAMP_TO_EDGE);
    glTexImage2D(GL_TEXTURE_2D, 0, GL_RGBA8, (GLsizei)om_width, (GLsizei)om_height, 0, GL_RGBA, GL_UNSIGNED_BYTE, NULL);
    glFramebufferTexture2D(GL_FRAMEBUFFER, GL_COLOR_ATTACHMENT0, GL_TEXTURE_2D, om_color, 0);
    if (request->depth) {
        glGenRenderbuffers(1, &om_depth);
        glBindRenderbuffer(GL_RENDERBUFFER, om_depth);
        glRenderbufferStorage(GL_RENDERBUFFER, request->stencil ? GL_DEPTH24_STENCIL8 : GL_DEPTH_COMPONENT24,
            (GLsizei)om_width, (GLsizei)om_height);
        glFramebufferRenderbuffer(GL_FRAMEBUFFER, request->stencil ? GL_DEPTH_STENCIL_ATTACHMENT : GL_DEPTH_ATTACHMENT,
            GL_RENDERBUFFER, om_depth);
    }
    if (!om_framebuffer || !om_color || (request->depth && !om_depth) ||
        glCheckFramebufferStatus(GL_FRAMEBUFFER) != GL_FRAMEBUFFER_COMPLETE || glGetError() != GL_NO_ERROR) {
        om_close_context(); return 0;
    }
    glDisable(GL_SCISSOR_TEST); glColorMask(GL_TRUE, GL_TRUE, GL_TRUE, GL_TRUE);
    glDepthMask(GL_TRUE); glStencilMask(0xffffffffu);
    glClearColor(0, 0, 0, 1); glClearDepthf(1); glClearStencil(0);
    glClear(GL_COLOR_BUFFER_BIT | (request->depth ? GL_DEPTH_BUFFER_BIT : 0) | (request->stencil ? GL_STENCIL_BUFFER_BIT : 0));
    glBindFramebuffer(GL_FRAMEBUFFER, 0); glBindTexture(GL_TEXTURE_2D, 0); glBindRenderbuffer(GL_RENDERBUFFER, 0);
    request->get_current_framebuffer = om_get_framebuffer;
    request->get_proc_address = om_get_proc;
    om_callbacks = *request;
    om_registered = true;
    return 1;
}
static int om_present(unsigned width, unsigned height) {
    if (!om_reset_armed || !om_registered || om_destroyed || !width || !height ||
        width > om_width || height > om_height || !om_web_hw_alive()) return 0;
    GLint saved_read = 0, saved_draw = 0;
    const GLboolean scissor = glIsEnabled(GL_SCISSOR_TEST);
    glGetIntegerv(GL_READ_FRAMEBUFFER_BINDING, &saved_read);
    glGetIntegerv(GL_DRAW_FRAMEBUFFER_BINDING, &saved_draw);
    int canvas_width = 0, canvas_height = 0;
    if (emscripten_get_canvas_element_size(om_target, &canvas_width, &canvas_height) != EMSCRIPTEN_RESULT_SUCCESS) return 0;
    if ((unsigned)canvas_width != width || (unsigned)canvas_height != height) {
        if (emscripten_set_canvas_element_size(om_target, (int)width, (int)height) != EMSCRIPTEN_RESULT_SUCCESS) return 0;
    }
    glDisable(GL_SCISSOR_TEST);
    glBindFramebuffer(GL_READ_FRAMEBUFFER, om_framebuffer);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, 0);
    glBlitFramebuffer(0, 0, (GLint)width, (GLint)height, 0, 0, (GLint)width, (GLint)height, GL_COLOR_BUFFER_BIT, GL_NEAREST);
    const int accepted = glGetError() == GL_NO_ERROR && emscripten_webgl_commit_frame() == EMSCRIPTEN_RESULT_SUCCESS;
    glBindFramebuffer(GL_READ_FRAMEBUFFER, (GLuint)saved_read);
    glBindFramebuffer(GL_DRAW_FRAMEBUFFER, (GLuint)saved_draw);
    if (scissor) glEnable(GL_SCISSOR_TEST);
    return accepted && om_web_hw_alive();
}
void om_web_hw_call(unsigned operation, unsigned first, unsigned second, uint32_t* result) {
    if (!result) return;
    unsigned accepted = 0;
    switch (operation) {
    case 1: /* Activate exactly once after the core has accepted content. */
        if (!om_registered || om_reset_armed || om_destroyed || !om_web_hw_alive()) break;
        om_reset_armed = true;
        om_callbacks.context_reset();
        accepted = om_web_hw_alive();
        break;
    case 2: accepted = om_present(first, second); break;
    case 3: /* Disarm before the fallible callback; keep the context for unload. */
        if (om_destroyed) break;
        om_destroyed = true;
        if (om_registered && om_reset_armed) {
            if (!om_web_hw_alive()) break;
            if (om_callbacks.context_destroy) om_callbacks.context_destroy();
        }
        om_reset_armed = false;
        memset(&om_callbacks, 0, sizeof(om_callbacks));
        accepted = 1;
        break;
    case 4: /* Called only after unload/deinit have returned normally. */
        if (!om_destroyed) break;
        accepted = om_close_context();
        om_hw_unmap_canvas();
        memset(&om_callbacks, 0, sizeof(om_callbacks));
        om_registered = om_reset_armed = om_destroyed = false;
        om_width = om_height = 0;
        break;
    default: return;
    }
    result[0] = accepted;
    result[1] = 1;
}
