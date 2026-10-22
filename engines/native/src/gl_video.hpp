#pragma once
#include <SDL3/SDL.h>
#include <SDL3/SDL_opengl.h>
#include <libretro.h>
#include <cstdint>

namespace omni {
// One contained worker owns one bounded offscreen target. Cores never receive
// the browser's graphics context or a handle to another process's surface.
class GlVideo {
  SDL_Window* window_=nullptr;SDL_GLContext context_=nullptr;
  retro_hw_render_callback callback_{};bool negotiated_=false,reset_called_=false;
  bool release_in_progress_=false,release_failed_=false;
  bool reset_in_progress_=false,reset_failed_=false;
  unsigned width_=0,height_=0;GLuint framebuffer_=0,color_=0,depth_=0;
  struct Functions {
    decltype(&glGetIntegerv) get_integer=nullptr;decltype(&glGetBooleanv) get_boolean=nullptr;decltype(&glGetFloatv) get_float=nullptr;
    decltype(&glIsEnabled) is_enabled=nullptr;decltype(&glEnable) enable=nullptr;decltype(&glDisable) disable=nullptr;
    decltype(&glClearColor) clear_color=nullptr;decltype(&glClear) clear=nullptr;decltype(&glColorMask) color_mask=nullptr;decltype(&glViewport) viewport=nullptr;
    decltype(&glGenTextures) gen_textures=nullptr;decltype(&glBindTexture) bind_texture=nullptr;decltype(&glTexParameteri) texture_parameter=nullptr;decltype(&glTexImage2D) texture_image=nullptr;decltype(&glDeleteTextures) delete_textures=nullptr;
    decltype(&glReadBuffer) read_buffer=nullptr;decltype(&glDrawBuffer) draw_buffer=nullptr;
    PFNGLGENFRAMEBUFFERSPROC gen_framebuffers=nullptr;PFNGLBINDFRAMEBUFFERPROC bind_framebuffer=nullptr;PFNGLDELETEFRAMEBUFFERSPROC delete_framebuffers=nullptr;
    PFNGLFRAMEBUFFERTEXTURE2DPROC framebuffer_texture=nullptr;PFNGLCHECKFRAMEBUFFERSTATUSPROC framebuffer_status=nullptr;
    PFNGLGENRENDERBUFFERSPROC gen_renderbuffers=nullptr;PFNGLBINDRENDERBUFFERPROC bind_renderbuffer=nullptr;PFNGLRENDERBUFFERSTORAGEPROC renderbuffer_storage=nullptr;
    PFNGLFRAMEBUFFERRENDERBUFFERPROC framebuffer_renderbuffer=nullptr;PFNGLDELETERENDERBUFFERSPROC delete_renderbuffers=nullptr;PFNGLBLITFRAMEBUFFERPROC blit=nullptr;
    PFNGLGETGRAPHICSRESETSTATUSPROC reset_status=nullptr;
  } gl_{};
  void load_functions();void allocate_target(bool depth,bool stencil);
public:
  static void window_attributes();
  void configure(SDL_Window* window,unsigned width,unsigned height);
  bool negotiate(retro_hw_render_callback& callback);
  bool negotiated()const{return negotiated_;}
  bool core_callback_active()const{return release_in_progress_||reset_in_progress_;}
  bool core_callback_failed()const{return release_failed_||reset_failed_;}
  std::uintptr_t framebuffer()const{return framebuffer_;}
  unsigned width()const{return width_;}unsigned height()const{return height_;}
  void make_current();void reset_core();void release_core();void destroy()noexcept;
  bool present(unsigned width,unsigned height,double aspect_ratio);
  static retro_proc_address_t procedure(const char* name);
};
}
