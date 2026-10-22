#include "engine.hpp"
#include "gl_video.hpp"
#include <algorithm>
#include <cmath>
#include <cstring>
#include <cstdlib>

namespace omni {
namespace {
template<class T>T load(const char* name){const auto value=SDL_GL_GetProcAddress(name);if(!value)throw Failure("runtime_unavailable");return reinterpret_cast<T>(value);}
void attribute(SDL_GLAttr key,int value){if(!SDL_GL_SetAttribute(key,value))throw Failure("runtime_unavailable");}
}
void GlVideo::window_attributes(){
  SDL_GL_ResetAttributes();attribute(SDL_GL_CONTEXT_PROFILE_MASK,SDL_GL_CONTEXT_PROFILE_CORE);
  attribute(SDL_GL_CONTEXT_MAJOR_VERSION,3);attribute(SDL_GL_CONTEXT_MINOR_VERSION,3);
  attribute(SDL_GL_RED_SIZE,8);attribute(SDL_GL_GREEN_SIZE,8);attribute(SDL_GL_BLUE_SIZE,8);attribute(SDL_GL_ALPHA_SIZE,8);
  attribute(SDL_GL_DEPTH_SIZE,24);attribute(SDL_GL_STENCIL_SIZE,8);attribute(SDL_GL_DOUBLEBUFFER,1);
  attribute(SDL_GL_FRAMEBUFFER_SRGB_CAPABLE,0);
  attribute(SDL_GL_CONTEXT_FLAGS,SDL_GL_CONTEXT_ROBUST_ACCESS_FLAG);
  attribute(SDL_GL_CONTEXT_RESET_NOTIFICATION,SDL_GL_CONTEXT_RESET_LOSE_CONTEXT);
}
void GlVideo::configure(SDL_Window* window,unsigned width,unsigned height){
  if(window_||!window||width<16||height<16||width>4096||height>4096||std::uint64_t(width)*height>16u*1024u*1024u)throw Failure("runtime_crashed");
  window_=window;width_=width;height_=height;
}
void GlVideo::load_functions(){
#define OMNI_GL(member,name) gl_.member=load<decltype(gl_.member)>(name)
  OMNI_GL(get_integer,"glGetIntegerv");OMNI_GL(get_boolean,"glGetBooleanv");OMNI_GL(get_float,"glGetFloatv");OMNI_GL(is_enabled,"glIsEnabled");OMNI_GL(enable,"glEnable");OMNI_GL(disable,"glDisable");
  OMNI_GL(clear_color,"glClearColor");OMNI_GL(clear,"glClear");OMNI_GL(color_mask,"glColorMask");OMNI_GL(viewport,"glViewport");
  OMNI_GL(gen_textures,"glGenTextures");OMNI_GL(bind_texture,"glBindTexture");OMNI_GL(texture_parameter,"glTexParameteri");OMNI_GL(texture_image,"glTexImage2D");OMNI_GL(delete_textures,"glDeleteTextures");
  OMNI_GL(read_buffer,"glReadBuffer");OMNI_GL(draw_buffer,"glDrawBuffer");OMNI_GL(gen_framebuffers,"glGenFramebuffers");OMNI_GL(bind_framebuffer,"glBindFramebuffer");OMNI_GL(delete_framebuffers,"glDeleteFramebuffers");OMNI_GL(framebuffer_texture,"glFramebufferTexture2D");OMNI_GL(framebuffer_status,"glCheckFramebufferStatus");
  OMNI_GL(gen_renderbuffers,"glGenRenderbuffers");OMNI_GL(bind_renderbuffer,"glBindRenderbuffer");OMNI_GL(renderbuffer_storage,"glRenderbufferStorage");OMNI_GL(framebuffer_renderbuffer,"glFramebufferRenderbuffer");OMNI_GL(delete_renderbuffers,"glDeleteRenderbuffers");OMNI_GL(blit,"glBlitFramebuffer");
#undef OMNI_GL
  const bool khr=SDL_GL_ExtensionSupported("GL_KHR_robustness"),arb=SDL_GL_ExtensionSupported("GL_ARB_robustness");if(!khr&&!arb)throw Failure("runtime_unavailable");
  const char* reset=khr?"glGetGraphicsResetStatus":"glGetGraphicsResetStatusARB";
  gl_.reset_status=load<decltype(gl_.reset_status)>(reset);
  GLint flags=0;gl_.get_integer(GL_CONTEXT_FLAGS,&flags);
  if(!(flags&GL_CONTEXT_FLAG_ROBUST_ACCESS_BIT))throw Failure("runtime_unavailable");
}
void GlVideo::allocate_target(bool depth,bool stencil){
  GLint max_texture=0,max_renderbuffer=0;gl_.get_integer(GL_MAX_TEXTURE_SIZE,&max_texture);gl_.get_integer(GL_MAX_RENDERBUFFER_SIZE,&max_renderbuffer);
  if(width_>static_cast<unsigned>(std::max(0,max_texture))||height_>static_cast<unsigned>(std::max(0,max_texture))||width_>static_cast<unsigned>(std::max(0,max_renderbuffer))||height_>static_cast<unsigned>(std::max(0,max_renderbuffer)))throw Failure("runtime_unavailable");
  gl_.gen_textures(1,&color_);gl_.bind_texture(GL_TEXTURE_2D,color_);gl_.texture_parameter(GL_TEXTURE_2D,GL_TEXTURE_MIN_FILTER,GL_LINEAR);gl_.texture_parameter(GL_TEXTURE_2D,GL_TEXTURE_MAG_FILTER,GL_LINEAR);gl_.texture_parameter(GL_TEXTURE_2D,GL_TEXTURE_WRAP_S,GL_CLAMP_TO_EDGE);gl_.texture_parameter(GL_TEXTURE_2D,GL_TEXTURE_WRAP_T,GL_CLAMP_TO_EDGE);
  gl_.texture_image(GL_TEXTURE_2D,0,GL_RGBA8,static_cast<GLsizei>(width_),static_cast<GLsizei>(height_),0,GL_RGBA,GL_UNSIGNED_BYTE,nullptr);
  gl_.gen_framebuffers(1,&framebuffer_);gl_.bind_framebuffer(GL_FRAMEBUFFER,framebuffer_);gl_.framebuffer_texture(GL_FRAMEBUFFER,GL_COLOR_ATTACHMENT0,GL_TEXTURE_2D,color_,0);
  if(depth||stencil){
    gl_.gen_renderbuffers(1,&depth_);gl_.bind_renderbuffer(GL_RENDERBUFFER,depth_);
    const auto format=depth&&stencil?GL_DEPTH24_STENCIL8:depth?GL_DEPTH_COMPONENT24:GL_STENCIL_INDEX8;
    const auto attachment=depth&&stencil?GL_DEPTH_STENCIL_ATTACHMENT:depth?GL_DEPTH_ATTACHMENT:GL_STENCIL_ATTACHMENT;
    gl_.renderbuffer_storage(GL_RENDERBUFFER,format,static_cast<GLsizei>(width_),static_cast<GLsizei>(height_));gl_.framebuffer_renderbuffer(GL_FRAMEBUFFER,attachment,GL_RENDERBUFFER,depth_);
  }
  if(!framebuffer_||!color_||((depth||stencil)&&!depth_)||gl_.framebuffer_status(GL_FRAMEBUFFER)!=GL_FRAMEBUFFER_COMPLETE)throw Failure("runtime_unavailable");
  // The target must not expose uninitialized driver memory on an early frame.
  gl_.disable(GL_SCISSOR_TEST);gl_.color_mask(GL_TRUE,GL_TRUE,GL_TRUE,GL_TRUE);gl_.clear_color(0,0,0,1);gl_.clear(GL_COLOR_BUFFER_BIT|(depth?GL_DEPTH_BUFFER_BIT:0)|(stencil?GL_STENCIL_BUFFER_BIT:0));
  gl_.bind_texture(GL_TEXTURE_2D,0);gl_.bind_renderbuffer(GL_RENDERBUFFER,0);gl_.clear_color(0,0,0,0);gl_.viewport(0,0,static_cast<GLsizei>(width_),static_cast<GLsizei>(height_));
}
bool GlVideo::negotiate(retro_hw_render_callback& callback){
  if(!window_||negotiated_||core_callback_active()||core_callback_failed()||callback.context_type!=RETRO_HW_CONTEXT_OPENGL_CORE||callback.version_major!=3||callback.version_minor>3||!callback.bottom_left_origin||!callback.context_reset)return false;
  // The declared adapter is OpenGL core 3.3, with robust access. A core cannot
  // negotiate an ES/Vulkan/D3D context or silently request another trust policy.
  if(context_){const auto window=window_;const auto width=width_,height=height_;destroy();configure(window,width,height);}
  if(!context_){context_=SDL_GL_CreateContext(window_);if(!context_)throw Failure("runtime_unavailable");make_current();load_functions();allocate_target(callback.depth,callback.stencil);}
  make_current();callback_=callback;negotiated_=true;
  // CPU pacing follows the core clock; a second blocking swap clock is avoided.
  if(!SDL_GL_SetSwapInterval(0))throw Failure("runtime_unavailable");return true;
}
void GlVideo::make_current(){if(!window_||!context_)throw Failure("runtime_unavailable");if((SDL_GL_GetCurrentContext()!=context_||SDL_GL_GetCurrentWindow()!=window_)&&!SDL_GL_MakeCurrent(window_,context_))throw Failure("runtime_unavailable");}
void GlVideo::reset_core(){
  if(!negotiated_||reset_called_||core_callback_active()||core_callback_failed())throw Failure("runtime_crashed");
  make_current();gl_.bind_framebuffer(GL_FRAMEBUFFER,framebuffer_);
  reset_in_progress_=true;
  try{callback_.context_reset();}
  catch(...){reset_in_progress_=false;reset_failed_=true;throw;}
  reset_in_progress_=false;reset_called_=true;
}
void GlVideo::release_core(){
  if(core_callback_active()||core_callback_failed())throw Failure("runtime_crashed");
  const auto destroy=reset_called_?callback_.context_destroy:nullptr;
  // A failed context acquisition has not invoked the callback: retain its
  // ownership for a later attempt while the core DLL is still live.
  if(destroy)make_current();
  release_in_progress_=true;
  try{if(destroy)destroy();}
  catch(...){release_in_progress_=false;release_failed_=true;throw;}
  release_in_progress_=false;reset_called_=negotiated_=false;callback_={};
}
void GlVideo::destroy()noexcept{
  // Unknown core callback lifetime cannot be hidden by destroying the context
  // or later unloading the DLL. This is the contained actor process.
  try{release_core();}catch(...){std::_Exit(1);}
  if(context_){if(window_&&SDL_GL_MakeCurrent(window_,context_)){if(depth_&&gl_.delete_renderbuffers)gl_.delete_renderbuffers(1,&depth_);if(framebuffer_&&gl_.delete_framebuffers)gl_.delete_framebuffers(1,&framebuffer_);if(color_&&gl_.delete_textures)gl_.delete_textures(1,&color_);}SDL_GL_DestroyContext(context_);}
  context_=nullptr;window_=nullptr;framebuffer_=color_=depth_=0;gl_={};
}
retro_proc_address_t GlVideo::procedure(const char* name){
  if(!name)return nullptr;unsigned size=0;
  while(size<128&&name[size]){const unsigned char c=name[size++];if(!((c>='a'&&c<='z')||(c>='A'&&c<='Z')||(c>='0'&&c<='9')||c=='_'))return nullptr;}
  if(!size||size==128||!(std::strncmp(name,"gl",2)==0||std::strncmp(name,"wgl",3)==0))return nullptr;
  return reinterpret_cast<retro_proc_address_t>(SDL_GL_GetProcAddress(name));
}
bool GlVideo::present(unsigned width,unsigned height,double aspect){
  if(!negotiated_||!reset_called_||core_callback_active()||core_callback_failed()||!width||!height||width>width_||height>height_)throw Failure("runtime_crashed");make_current();
  if(gl_.reset_status()!=GL_NO_ERROR)throw Failure("runtime_crashed");
  int output_w=0,output_h=0;if(!SDL_GetWindowSizeInPixels(window_,&output_w,&output_h))throw Failure("runtime_crashed");
  if(output_w<=0||output_h<=0||output_w>16384||output_h>16384||std::uint64_t(output_w)*output_h>64u*1024u*1024u)return false;
  const auto ratio=std::isfinite(aspect)&&aspect>.1&&aspect<10?aspect:double(width)/height;
  int draw_w=output_w,draw_h=static_cast<int>(std::lround(draw_w/ratio));if(draw_h>output_h){draw_h=output_h;draw_w=static_cast<int>(std::lround(draw_h*ratio));}
  const int left=(output_w-draw_w)/2,bottom=(output_h-draw_h)/2;
  GLint read=0,draw=0;GLfloat clear[4]{};GLboolean mask[4]{};
  gl_.get_integer(GL_READ_FRAMEBUFFER_BINDING,&read);gl_.get_integer(GL_DRAW_FRAMEBUFFER_BINDING,&draw);gl_.get_float(GL_COLOR_CLEAR_VALUE,clear);gl_.get_boolean(GL_COLOR_WRITEMASK,mask);
  const bool scissor=gl_.is_enabled(GL_SCISSOR_TEST)!=0,srgb=gl_.is_enabled(GL_FRAMEBUFFER_SRGB)!=0;
  gl_.disable(GL_SCISSOR_TEST);gl_.disable(GL_FRAMEBUFFER_SRGB);gl_.color_mask(GL_TRUE,GL_TRUE,GL_TRUE,GL_TRUE);
  gl_.bind_framebuffer(GL_DRAW_FRAMEBUFFER,0);gl_.draw_buffer(GL_BACK);gl_.clear_color(0,0,0,1);gl_.clear(GL_COLOR_BUFFER_BIT);
  gl_.bind_framebuffer(GL_READ_FRAMEBUFFER,framebuffer_);GLint source_read=0;gl_.get_integer(GL_READ_BUFFER,&source_read);gl_.read_buffer(GL_COLOR_ATTACHMENT0);
  gl_.blit(0,0,static_cast<GLint>(width),static_cast<GLint>(height),left,bottom,left+draw_w,bottom+draw_h,GL_COLOR_BUFFER_BIT,GL_LINEAR);
  gl_.read_buffer(static_cast<GLenum>(source_read));gl_.bind_framebuffer(GL_READ_FRAMEBUFFER,static_cast<GLuint>(read));gl_.bind_framebuffer(GL_DRAW_FRAMEBUFFER,static_cast<GLuint>(draw));
  gl_.clear_color(clear[0],clear[1],clear[2],clear[3]);gl_.color_mask(mask[0],mask[1],mask[2],mask[3]);if(scissor)gl_.enable(GL_SCISSOR_TEST);if(srgb)gl_.enable(GL_FRAMEBUFFER_SRGB);
  if(!SDL_GL_SwapWindow(window_))throw Failure("runtime_crashed");return true;
}
}
