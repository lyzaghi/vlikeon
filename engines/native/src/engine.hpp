#pragma once
#include "protocol.hpp"
#include "runtime_identity.hpp"
#include "saved_files.hpp"
#include "memory_files.hpp"
#include "md_portable.hpp"
#include "gl_video.hpp"
#include "measurement.hpp"
#include "input_profile.hpp"
#include "audio_ring.hpp"
#include "owned_actor_exit.h"
#include "client.hpp"
#include <SDL3/SDL.h>
#include <libretro.h>
#include <array>
#include <atomic>
#include <chrono>
#include <map>
#include <mutex>
#include <optional>
#include <stdexcept>
#include <string>
#include <type_traits>
#include <utility>

namespace omni {
struct Failure:std::runtime_error {const char* code;explicit Failure(const char* value):std::runtime_error(value),code(value){}};
class Engine {
  static Engine* active_;
  std::unique_ptr<presentation::Client> presentation_;
  std::uint64_t presentation_epoch_=0,presentation_count_=0;bool presentation_escape_=false,presentation_input_seen_=false;
  std::atomic<unsigned> presentation_rate_{48000};
  SDL_SharedObject* object_=nullptr;
  SDL_Window* window_=nullptr;SDL_Renderer* renderer_=nullptr;SDL_Texture* texture_=nullptr;SDL_AudioStream* audio_=nullptr;
  std::array<SDL_Gamepad*,4> pads_{};InputProfile input_profile_;
  std::array<std::array<std::array<std::int16_t,2>,2>,4> stick_input_{};
  struct Rumble{std::mutex mutex;std::uint16_t strong=0,weak=0;bool available=false;std::uint16_t sent_strong=0,sent_weak=0;std::uint64_t sent_at=0;};std::array<Rumble,4> rumble_{};
  std::map<std::string,std::string> variables_;
  std::vector<std::uint8_t> rom_;std::string filename_,title_,core_path_;
  std::vector<std::uint32_t> pixels_;unsigned texture_width_=0,texture_height_=0,pixel_format_=RETRO_PIXEL_FORMAT_0RGB1555;
  bool initialized_=false,loaded_=false,close_requested_=false,quit_=false,battery_=false,core_paused_=true;
  bool guest_pause_active_=false,guest_pause_failed_=false,core_call_active_=false,core_call_failed_=false;
  std::atomic<bool> running_{false},started_{false},fault_{false},shutdown_requested_{false};
  enum class CorePhase{Created,Loading,Ready,Stopping};std::atomic<CorePhase> core_phase_{CorePhase::Created};
  std::atomic<bool> audio_running_{false};
  AudioRing audio_ring_;std::atomic<std::uint32_t> underruns_{0};
  std::atomic<std::uint64_t> total_presented_{0},total_underruns_{0};std::uint64_t total_dropped_=0;Measurement measurement_;
  unsigned latency_frames_=2;double nominal_hz_=60.0,sample_rate_=48000.0;float volume_=0.8f;
  double active_time_ms_=0;
  bool reset_after_ram_load_=false;
  GlVideo gl_video_;bool hardware_video_=false,analog_controls_=false;std::atomic<const char*> fault_code_{"runtime_crashed"};
  std::mutex av_mutex_;std::optional<retro_system_av_info> pending_av_;std::optional<retro_game_geometry> pending_geometry_;
  SavedFiles saved_files_;std::string system_directory_=".",core_save_directory_="save";bool filesystem_save_memory_=false;
  MemoryFiles memory_files_;OmniWritableFilesV1 memory_files_interface_{};bool memory_files_installed_=false;
  bool md_portable_=false,md_layout_failed_=false;std::optional<MdSaveContract> md_contract_;
  std::chrono::steady_clock::time_point deadline_,previous_frame_,metrics_start_,presentation_due_;
  std::vector<double> intervals_;std::uint32_t presented_=0,dropped_=0;Json latest_metrics_;
  struct Api {
    decltype(&retro_api_version) api_version;
    decltype(&retro_init) init;decltype(&retro_deinit) deinit;
    decltype(&retro_set_environment) set_environment;decltype(&retro_set_video_refresh) set_video;
    decltype(&retro_set_audio_sample) set_audio;decltype(&retro_set_audio_sample_batch) set_audio_batch;
    decltype(&retro_set_input_poll) set_input_poll;decltype(&retro_set_input_state) set_input_state;
    decltype(&retro_get_system_info) system_info;decltype(&retro_get_system_av_info) av_info;
    decltype(&retro_set_controller_port_device) controller;decltype(&retro_load_game) load_game;
    decltype(&retro_unload_game) unload_game;decltype(&retro_run) run;decltype(&retro_reset) reset;
    decltype(&retro_serialize_size) serialize_size;decltype(&retro_serialize) serialize;decltype(&retro_unserialize) unserialize;
    decltype(&retro_get_memory_data) memory_data;decltype(&retro_get_memory_size) memory_size;
    void (*flush_io)()=nullptr;
    OmniInstallWritableFilesV1 install_files=nullptr;
    int (*flush_io_checked)()=nullptr;
    void (*describe_md_save)(std::uint32_t*)=nullptr;
    bool (*finish_prepare)()=nullptr;int (*pause_guest)()=nullptr;
  } api_{};
  retro_system_info info_{};
  retro_disk_control_callback discs_{};double aspect_ratio_=0;
  SDL_FingerID finger_=0;bool finger_down_=false,pointer_down_=false,pointer_inside_=false;
  std::int16_t pointer_x_=0,pointer_y_=0;float finger_x_=0,finger_y_=0;
  template<class T>T symbol(const char* name);
  template<class Function,class... Args>auto call_core(Function callback,Args&&... args){
    if(core_call_active_||core_call_failed_||guest_pause_active_||guest_pause_failed_)throw Failure("runtime_crashed");
    core_call_active_=true;
    try{
      if constexpr(std::is_void_v<std::invoke_result_t<Function,Args...>>){
        callback(std::forward<Args>(args)...);core_call_active_=false;
      }else{
        auto result=callback(std::forward<Args>(args)...);core_call_active_=false;return result;
      }
    }catch(...){core_call_active_=false;core_call_failed_=true;fault_=true;throw Failure("runtime_crashed");}
  }
  void prepare(const Json& params);void pause();void resume();void reset_audio();void read_av();
  bool restart_content();
  void pause_core();void retire_guest();void stop_content();void flush_files();
  std::vector<std::uint8_t> save_memory(bool allow_empty);void validate_memory(std::span<const std::uint8_t> data);void load_memory(std::span<const std::uint8_t> data);
  void complete_preparation();bool consume_av_changes();bool valid_av(const retro_system_av_info& av)const;
  Json disc_status();bool switch_disc(unsigned index,bool ejected);void apply_av(const retro_system_av_info& av);
  std::vector<std::uint8_t> save(const std::string& kind);void load(const std::string& kind,const std::vector<std::uint8_t>& payload);
  static bool environment(unsigned command,void* data);
  static bool environment_impl(unsigned command,void* data);
  static void video(const void* data,unsigned width,unsigned height,std::size_t pitch);
  static void video_impl(const void* data,unsigned width,unsigned height,std::size_t pitch);
  static std::uintptr_t hardware_framebuffer();
  static void audio_sample(std::int16_t left,std::int16_t right);
  static std::size_t audio_batch(const std::int16_t* data,std::size_t frames);
  static void input_poll();static std::int16_t input_state(unsigned port,unsigned device,unsigned index,unsigned id);
  void update_pointer();
  static void SDLCALL supply_audio(void* context,SDL_AudioStream* stream,int additional,int total);
  static void quiet_log(enum retro_log_level,const char*,...);
  void pad_added(SDL_JoystickID id);void pad_removed(SDL_JoystickID id);void update_metrics();
  static bool rumble(unsigned port,retro_rumble_effect effect,std::uint16_t strength);void flush_rumble();void stop_rumble();
public:
  explicit Engine(std::unique_ptr<presentation::Client> presentation={});~Engine();
  bool process(const Packet& packet);
  void poll();void tick();void shutdown();
  bool quitting()const{return quit_;}
};
}
