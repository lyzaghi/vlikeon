#include "engine.hpp"
#include "presentation_input.hpp"
#include <algorithm>
#include <cmath>
#include <cstring>
#include <cstdlib>
#include <fstream>
#include <filesystem>
#include <limits>
#include <new>
#include <utility>

namespace omni {
using Clock=std::chrono::steady_clock;
Engine* Engine::active_=nullptr;
template<class T>T Engine::symbol(const char* name){const auto found=core_module_symbol(object_,name);if(!found)throw Failure("runtime_crashed");return reinterpret_cast<T>(found);}

Engine::Engine(std::unique_ptr<presentation::Client> presentation):presentation_(std::move(presentation)){
  if(active_)throw Failure("runtime_crashed");active_=this;
  if(presentation_&&std::strcmp(OMNIPLAY_VIDEO_BACKEND,"software"))throw Failure("unsupported_capability");
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
  if(!presentation_)throw Failure("unsupported_capability");
#else
  if(!SDL_Init(presentation_?0:SDL_INIT_VIDEO|SDL_INIT_AUDIO))throw Failure("runtime_crashed");
  // An unavailable optional controller subsystem does not remove keyboard input.
  if(!presentation_)SDL_InitSubSystem(SDL_INIT_GAMEPAD);
#endif
  core_path_=executable_base_directory()+OMNIPLAY_CORE_FILENAME;
  object_=load_core_module(core_path_);if(!object_)throw Failure("runtime_crashed");
  api_.api_version=symbol<decltype(api_.api_version)>("retro_api_version");
  api_.init=symbol<decltype(api_.init)>("retro_init");api_.deinit=symbol<decltype(api_.deinit)>("retro_deinit");
  api_.set_environment=symbol<decltype(api_.set_environment)>("retro_set_environment");api_.set_video=symbol<decltype(api_.set_video)>("retro_set_video_refresh");
  api_.set_audio=symbol<decltype(api_.set_audio)>("retro_set_audio_sample");api_.set_audio_batch=symbol<decltype(api_.set_audio_batch)>("retro_set_audio_sample_batch");
  api_.set_input_poll=symbol<decltype(api_.set_input_poll)>("retro_set_input_poll");api_.set_input_state=symbol<decltype(api_.set_input_state)>("retro_set_input_state");
  api_.system_info=symbol<decltype(api_.system_info)>("retro_get_system_info");api_.av_info=symbol<decltype(api_.av_info)>("retro_get_system_av_info");
  api_.controller=symbol<decltype(api_.controller)>("retro_set_controller_port_device");api_.load_game=symbol<decltype(api_.load_game)>("retro_load_game");api_.unload_game=symbol<decltype(api_.unload_game)>("retro_unload_game");api_.run=symbol<decltype(api_.run)>("retro_run");
  api_.reset=symbol<decltype(api_.reset)>("retro_reset");
  api_.serialize_size=symbol<decltype(api_.serialize_size)>("retro_serialize_size");api_.serialize=symbol<decltype(api_.serialize)>("retro_serialize");api_.unserialize=symbol<decltype(api_.unserialize)>("retro_unserialize");api_.memory_data=symbol<decltype(api_.memory_data)>("retro_get_memory_data");api_.memory_size=symbol<decltype(api_.memory_size)>("retro_get_memory_size");
  if(call_core(api_.api_version)!=1)throw Failure("runtime_crashed");call_core(api_.system_info,&info_);
  if(!info_.library_name||!info_.library_version||std::strlen(info_.library_name)>128||std::strlen(info_.library_version)>128)throw Failure("runtime_crashed");
  latest_metrics_={{"window_duration_ms",1000},{"frames_presented",0},{"dropped_frames",0},{"audio_underruns",0},{"frame_time_p50_ms",0.0},{"frame_time_p95_ms",0.0},{"frame_time_p99_ms",0.0}};
}
Engine::~Engine(){shutdown();}

bool Engine::process(const Packet& packet){
  const auto id=packet.header.at("id").get<std::uint64_t>();const auto method=packet.header.at("method").get<std::string>();const auto& params=packet.header.at("params");
  if(method!="load"&&!packet.payload.empty())throw Failure("invalid_state");
  if(method=="hello")reply(id,{{"runtime_id",OMNIPLAY_RUNTIME_ID},{"library_name",info_.library_name},{"library_version",info_.library_version},{"video_backend",OMNIPLAY_VIDEO_BACKEND},{"presentation_protocol",presentation::kProtocol},{"presentation_mode",presentation_?"brokered_rgba":"direct"},{"measurement_protocol",presentation_?0:1},{"input_profile_protocol",3},{"haptics_protocol",1},{"persistent_files_protocol",2},{"checked_file_io_protocol",1},{"combined_in_game_protocol",1},{"portable_file_protocol",1},{"writable_files_protocol",1},{"pause_barrier_protocol",1}});
  else if(method=="prepare"){
    prepare(params);reply(id,{{"nominal_refresh_hz",nominal_hz_},{"sample_rate",sample_rate_},{"save_ram_bytes",call_core(api_.memory_size,RETRO_MEMORY_SAVE_RAM)},{"save_rtc_bytes",call_core(api_.memory_size,RETRO_MEMORY_RTC)},{"save_files_supported",saved_files_.enabled()},{"save_memory_files",filesystem_save_memory_},{"writable_files_protocol",memory_files_installed_?1:0},{"save_state_bytes",call_core(api_.serialize_size)},{"portable_files_profile",md_portable_?"ares_md_v1":""},{"portable_progress_available",md_contract_&&(md_contract_->words[2]||md_contract_->words[3])},{"disc",disc_status()}});
  }else if(method=="start"||method=="resume"){if(!loaded_||core_phase_.load()!=CorePhase::Ready)throw Failure("invalid_state");resume();reply(id,Json::object());}
  else if(method=="pause"){if(!loaded_||core_phase_.load()!=CorePhase::Ready)throw Failure("invalid_state");pause();reply(id,Json::object());}
  else if(method=="cancel_close"){close_requested_=false;pause();reply(id,Json::object());}
  else if(method=="status")reply(id,{{"state",!loaded_?"created":!started_?"ready":running_?"running":"paused"},{"close_requested",close_requested_},{"active_playtime_ms",static_cast<std::uint64_t>(active_time_ms_)}});
  else if(method=="metrics"){update_metrics();reply(id,latest_metrics_);}
  else if(method=="measurement_start"){if(presentation_)throw Failure("unsupported_capability");if(!loaded_||!started_||close_requested_||measurement_.active())throw Failure("invalid_state");
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(window_)SDL_RaiseWindow(window_);
#endif
if(!running_)resume();measurement_.start(params.at("id").get<std::string>(),nominal_hz_);reply(id,measurement_.status());}
  else if(method=="measurement_status")reply(id,measurement_.status());
  else if(method=="measurement_cancel"){measurement_.cancel("user_cancelled");reply(id,measurement_.status());}
  else if(method=="measurement_read")reply(id,measurement_.result(),measurement_.payload());
  else if(method=="disc_status"){if(!loaded_||core_phase_.load()!=CorePhase::Ready)throw Failure("invalid_state");reply(id,disc_status());}
  else if(method=="disc_select"){
    if(!loaded_||core_phase_.load()!=CorePhase::Ready)throw Failure("invalid_state");const auto index=params.at("index").get<unsigned>();const auto before=disc_status();
    if(!before.at("supported").get<bool>()||index>=before.at("count").get<unsigned>())throw Failure("unsupported_capability");
    const bool playing=running_;pause();if(!switch_disc(index,false)){if(!switch_disc(before.at("index").get<unsigned>(),before.at("ejected").get<bool>()))throw Failure("runtime_crashed");if(playing)resume();throw Failure("invalid_state");}if(playing)resume();reply(id,disc_status());
  }
  else if(method=="save"){
    if(!loaded_||core_phase_.load()!=CorePhase::Ready)throw Failure("invalid_state");const bool playing=running_;pause();
    try{auto payload=save(params.at("kind").get<std::string>());if(playing)resume();reply(id,Json::object(),payload);}
    catch(const std::bad_alloc&){if(playing&&!fault_)resume();throw Failure("resource_limit");}
    catch(const std::filesystem::filesystem_error&){if(playing&&!fault_)resume();throw Failure("storage_failure");}
    catch(...){if(playing&&!fault_)resume();throw;}
  }
  else if(method=="load"){
    if(!loaded_||core_phase_.load()!=CorePhase::Ready)throw Failure("invalid_state");const bool playing=running_;
    try{load(params.at("kind").get<std::string>(),packet.payload);reply(id,Json::object());}
    catch(const Failure& error){if(playing&&!fault_&&loaded_&&(std::strcmp(error.code,"incompatible_save")==0||std::strcmp(error.code,"unsupported_capability")==0||std::strcmp(error.code,"resource_limit")==0||std::strcmp(error.code,"storage_failure")==0))resume();throw;}
  }
  else if(method=="stop"){pause();shutdown();if(fault_)throw Failure(fault_code_.load());reply(id,Json::object());quit_=true;}
  else throw Failure("unsupported_capability");
  return !quit_;
}

void Engine::prepare(const Json& params){
  if(initialized_||loaded_)throw Failure("invalid_state");
  filename_=params.at("filename").get<std::string>();title_=params.at("title").get<std::string>();const auto expected=params.at("bytes").get<std::uint64_t>();
  if(filename_.size()>32||filename_.rfind("game.",0)!=0||filename_.find_first_not_of("abcdefghijklmnopqrstuvwxyz0123456789.")!=std::string::npos||title_.empty()||title_.size()>512||expected==0||expected>std::uint64_t(4)*1024*1024*1024)throw Failure("runtime_crashed");
  const auto path=std::filesystem::path(filename_);if(!std::filesystem::is_regular_file(path)||std::filesystem::is_symlink(path)||std::filesystem::file_size(path)!=expected)throw Failure("runtime_crashed");
  const auto& config=params.at("config");volume_=config.at("audio_volume").get<float>();latency_frames_=config.at("latency_frames").get<unsigned>();const auto preset=config.at("preset").get<std::string>();
  if(!std::isfinite(volume_)||volume_<0||volume_>1||latency_frames_>8||(preset!="accurate"&&preset!="smooth"&&preset!="battery"))throw Failure("runtime_crashed");
  battery_=preset=="battery";if(preset=="smooth")latency_frames_=std::max(3u,latency_frames_);
  const auto writable=params.value("writable_files_protocol",0u);
  if(writable>1||(writable&&!params.value("pause_barrier",false)))throw Failure("unsupported_capability");
  if(writable){
    api_.install_files=symbol<OmniInstallWritableFilesV1>("omniplay_install_writable_files_v1");
    memory_files_interface_=memory_files_.interface();
    if(call_core(api_.install_files,&memory_files_interface_)!=1)throw Failure("unsupported_capability");
    memory_files_installed_=true;
  }
  reset_after_ram_load_=params.value("reset_after_ram_load",false);
  const auto save_directory=params.find("filesystem_save_directory");
  if(save_directory!=params.end()&&!save_directory->is_null())saved_files_.prepare(save_directory->get<std::string>(),memory_files_installed_?&memory_files_:nullptr);
  const auto file_io=params.value("file_io_protocol",0u);filesystem_save_memory_=params.value("filesystem_save_memory",false);
  if(file_io>1||(file_io&&(!saved_files_.enabled()||!params.value("pause_barrier",false)))||(filesystem_save_memory_&&(!saved_files_.enabled()||file_io!=1||!params.value("pause_barrier",false))))throw Failure("runtime_crashed");
  const auto core_save=params.find("core_save_directory");if(core_save!=params.end()&&!core_save->is_null()){if(!saved_files_.enabled()||*core_save!=*save_directory)throw Failure("runtime_crashed");core_save_directory_=core_save->get<std::string>();}
  if(filesystem_save_memory_&&(core_save==params.end()||core_save->is_null()))throw Failure("runtime_crashed");
  if(saved_files_.enabled()){if(file_io==1)api_.flush_io_checked=symbol<decltype(api_.flush_io_checked)>("omniplay_flush_io_checked");else api_.flush_io=symbol<decltype(api_.flush_io)>("omniplay_flush_io");}
  if(params.value("prepare_completion",false))api_.finish_prepare=symbol<decltype(api_.finish_prepare)>("omniplay_finish_prepare");
  const bool pause_barrier=params.value("pause_barrier",false);
  if(params.contains("pause_barrier_protocol")&&!params.at("pause_barrier_protocol").is_number_unsigned())throw Failure("unsupported_capability");
  const auto pause_protocol=params.value("pause_barrier_protocol",0u);
  if(pause_protocol!=(pause_barrier?1u:0u))throw Failure("unsupported_capability");
  if(pause_barrier)api_.pause_guest=symbol<decltype(api_.pause_guest)>("omniplay_pause_guest_checked_v1");
  const auto portable=params.value("portable_files_profile",std::string{});
  if(!portable.empty()){
    if(portable!="ares_md_v1"||std::strcmp(OMNIPLAY_RUNTIME_ID,"ares-md-native")||std::strcmp(info_.library_name,"OmniPlay ares MD")||std::strcmp(info_.library_version,"0.3.0+ares.9408cb43d494")||file_io!=1||!api_.pause_guest||filesystem_save_memory_||core_save_directory_!="save/MD"||!saved_files_.enabled())throw Failure("runtime_crashed");
    api_.describe_md_save=symbol<decltype(api_.describe_md_save)>("om_md_describe_save");md_portable_=true;
  }
  if(params.value("use_scratch_system_directory",false))system_directory_="scratch";
  if(params.value("has_system_assets",false))system_directory_=executable_base_directory()+"assets";
  const auto& options=params.at("core_options");if(!options.is_object()||options.size()>128)throw Failure("runtime_crashed");
  for(auto it=options.begin();it!=options.end();++it){const auto value=it.value().get<std::string>();if(it.key().empty()||it.key().size()>128||value.size()>256)throw Failure("runtime_crashed");variables_.emplace(it.key(),value);}
  const auto backend=params.value("video_backend",std::string("software"));if(backend!=OMNIPLAY_VIDEO_BACKEND)throw Failure("runtime_crashed");hardware_video_=backend=="opengl_core";
  analog_controls_=params.value("analog_controls",false);
  if(!input_profile_.load(params.value("input_profile",Json{})))throw Failure("invalid_request");
  if(presentation_){if(hardware_video_||!presentation_->snapshot().ready)throw Failure("runtime_unavailable");if(!presentation_->playback(false,static_cast<unsigned>(std::lround(volume_*100)),config.value("fullscreen",false)?2:1))throw Failure("runtime_unavailable");}
  else{
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
    throw Failure("unsupported_capability");
#else
    if(hardware_video_)GlVideo::window_attributes();
    window_=SDL_CreateWindow(("OmniPlay — "+title_).c_str(),768,672,SDL_WINDOW_RESIZABLE|(hardware_video_?SDL_WINDOW_OPENGL:0));if(!window_)throw Failure("runtime_unavailable");
    if(hardware_video_)gl_video_.configure(window_,params.at("gpu_max_width").get<unsigned>(),params.at("gpu_max_height").get<unsigned>());
    else{renderer_=SDL_CreateRenderer(window_,"software");if(!renderer_)throw Failure("runtime_unavailable");}
    if(config.value("fullscreen",false))SDL_SetWindowFullscreen(window_,true);
#endif
  }
  core_phase_=CorePhase::Loading;call_core(api_.set_environment,environment);call_core(api_.set_video,video);call_core(api_.set_audio,audio_sample);call_core(api_.set_audio_batch,audio_batch);call_core(api_.set_input_poll,input_poll);call_core(api_.set_input_state,input_state);call_core(api_.init);initialized_=true;if(fault_)throw Failure(fault_code_.load());
  retro_game_info game{};game.path=filename_.c_str();
  if(!info_.need_fullpath){if(expected>512u*1024u*1024u)throw Failure("unsupported_capability");rom_.resize(static_cast<std::size_t>(expected));std::ifstream source(path,std::ios::binary);if(!source.read(reinterpret_cast<char*>(rom_.data()),static_cast<std::streamsize>(rom_.size())))throw Failure("runtime_crashed");game.data=rom_.data();game.size=rom_.size();}
  loaded_=call_core(api_.load_game,&game);if(loaded_)core_paused_=false;
  if(!loaded_||fault_)throw Failure(fault_?fault_code_.load():"runtime_crashed");
  complete_preparation();
  if(md_portable_)md_contract_=call_core(&MdSaveContract::read,api_.describe_md_save);
  if(saved_files_.enabled()&&!filesystem_save_memory_&&(call_core(api_.memory_size,RETRO_MEMORY_SAVE_RAM)||call_core(api_.memory_size,RETRO_MEMORY_RTC)))throw Failure("unsupported_capability");
  if(api_.flush_io_checked){flush_files();if(!call_core(api_.serialize_size)||call_core(api_.serialize_size)>kMaxSave)throw Failure("unsupported_capability");}
  const auto minimum_ram=params.value("minimum_system_ram_bytes",std::uint64_t(0));
  if(minimum_ram>64u*1024u*1024u||call_core(api_.memory_size,RETRO_MEMORY_SYSTEM_RAM)<minimum_ram)throw Failure("content_mismatch");
  for(unsigned port=0;port<4;++port)call_core(api_.controller,port,RETRO_DEVICE_JOYPAD);read_av();
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(!presentation_){SDL_AudioSpec spec{};spec.format=SDL_AUDIO_S16;spec.channels=2;spec.freq=static_cast<int>(std::lround(sample_rate_));
    audio_=SDL_OpenAudioDeviceStream(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,&spec,supply_audio,this);if(!audio_)throw Failure("runtime_crashed");
    int count=0;SDL_JoystickID* ids=SDL_GetGamepads(&count);for(int i=0;ids&&i<count;++i)pad_added(ids[i]);SDL_free(ids);
  }
#endif
  metrics_start_=Clock::now();presentation_due_=Clock::time_point{};previous_frame_=Clock::time_point{};
}

void Engine::read_av(){
  retro_system_av_info av{};call_core(api_.av_info,&av);apply_av(av);
  std::lock_guard lock(av_mutex_);pending_av_.reset();pending_geometry_.reset();
}
void Engine::complete_preparation(){
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(hardware_video_){if(!gl_video_.negotiated())throw Failure("runtime_unavailable");gl_video_.reset_core();if(fault_)throw Failure("runtime_unavailable");}
#endif
  if(fault_)throw Failure(fault_code_.load());
  if(api_.finish_prepare&&!call_core(api_.finish_prepare))throw Failure(fault_?fault_code_.load():"content_mismatch");
  if(fault_)throw Failure(fault_code_.load());pause_core();
  if(fault_)throw Failure(fault_code_.load());core_phase_=CorePhase::Ready;
}
bool Engine::valid_av(const retro_system_av_info& av)const{
  if(!std::isfinite(av.timing.fps)||av.timing.fps<1||av.timing.fps>240||!std::isfinite(av.timing.sample_rate)||av.timing.sample_rate<8000||av.timing.sample_rate>192000)return false;
  return !hardware_video_||(av.geometry.base_width&&av.geometry.base_height&&av.geometry.max_width>=av.geometry.base_width&&av.geometry.max_height>=av.geometry.base_height&&av.geometry.max_width<=gl_video_.width()&&av.geometry.max_height<=gl_video_.height());
}
bool Engine::consume_av_changes(){
  std::optional<retro_system_av_info> av;std::optional<retro_game_geometry> geometry;
  {std::lock_guard lock(av_mutex_);av=std::exchange(pending_av_,std::nullopt);geometry=std::exchange(pending_geometry_,std::nullopt);}
  if(!av&&!geometry)return false;const bool playing=running_;
  if(av){
    if(playing)pause();else pause_core();
    // Draining a threaded renderer can publish one last geometry/clock update.
    // Merge it only after the guest has reached its pause barrier.
    std::lock_guard lock(av_mutex_);if(pending_av_){av=std::exchange(pending_av_,std::nullopt);geometry.reset();}if(pending_geometry_)geometry=std::exchange(pending_geometry_,std::nullopt);
  }
  if(av)apply_av(*av);if(geometry)aspect_ratio_=geometry->aspect_ratio;
  if(av&&playing){resume();return true;}return false;
}
void Engine::apply_av(const retro_system_av_info& av){
  if(!valid_av(av))throw Failure("unsupported_capability");
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(audio_){
    if(!SDL_LockAudioStream(audio_)){fault_=true;throw Failure("runtime_crashed");}
    SDL_AudioSpec input{};input.format=SDL_AUDIO_S16;input.channels=2;input.freq=static_cast<int>(std::lround(av.timing.sample_rate));
    if(!SDL_SetAudioStreamFormat(audio_,&input,nullptr)||!SDL_ClearAudioStream(audio_)){
      fault_=true;SDL_UnlockAudioStream(audio_);throw Failure("runtime_crashed");
    }
    audio_ring_.reset_quiescent();
  }
#endif
  nominal_hz_=av.timing.fps;sample_rate_=av.timing.sample_rate;
  presentation_rate_.store(static_cast<unsigned>(std::lround(sample_rate_)));
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(audio_&&!SDL_UnlockAudioStream(audio_)){fault_=true;throw Failure("runtime_crashed");}
#endif
  aspect_ratio_=std::isfinite(av.geometry.aspect_ratio)&&av.geometry.aspect_ratio>.1f&&av.geometry.aspect_ratio<10.f?av.geometry.aspect_ratio:0;
}
Json Engine::disc_status(){
  if(!discs_.get_num_images||!discs_.get_image_index||!discs_.get_eject_state||!discs_.set_eject_state||!discs_.set_image_index)return {{"supported",false},{"count",0},{"index",0},{"ejected",false}};
  const auto count=call_core(discs_.get_num_images),index=call_core(discs_.get_image_index);if(count>8||(count&&index>=count))throw Failure("runtime_crashed");return {{"supported",count>0},{"count",count},{"index",count?index:0},{"ejected",count?call_core(discs_.get_eject_state):false}};
}
bool Engine::switch_disc(unsigned index,bool ejected){
  const auto before=disc_status();if(!before.at("supported").get<bool>()||index>=before.at("count").get<unsigned>())return false;
  auto matches_count=[&]{return call_core(discs_.get_num_images)==before.at("count").get<unsigned>();};
  if(call_core(discs_.get_image_index)==index&&call_core(discs_.get_eject_state)==ejected)return true;
  if(!call_core(discs_.get_eject_state)&&!call_core(discs_.set_eject_state,true))return false;if(!matches_count()||!call_core(discs_.get_eject_state))return false;
  if(call_core(discs_.get_image_index)!=index&&!call_core(discs_.set_image_index,index))return false;if(!matches_count()||call_core(discs_.get_image_index)!=index||!call_core(discs_.get_eject_state))return false;
  if(!ejected&&!call_core(discs_.set_eject_state,false))return false;return matches_count()&&call_core(discs_.get_image_index)==index&&call_core(discs_.get_eject_state)==ejected;
}
void Engine::reset_audio(){
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(!audio_)return;
  if(!SDL_LockAudioStream(audio_)){fault_=true;throw Failure("runtime_crashed");}
  const bool cleared=SDL_ClearAudioStream(audio_);
  if(cleared)audio_ring_.reset_quiescent();
  const bool unlocked=SDL_UnlockAudioStream(audio_);
  if(!cleared||!unlocked){fault_=true;throw Failure("runtime_crashed");}
#endif
}
void Engine::pause_core(){
  if(guest_pause_active_||guest_pause_failed_||core_call_active_||core_call_failed_)throw Failure("runtime_crashed");
  if(!loaded_||core_paused_)return;
  // Cores without a declared background producer barrier run synchronously
  // between exports. A declared barrier must provide the exact checked ABI.
  if(!api_.pause_guest){core_paused_=true;return;}

#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(hardware_video_)gl_video_.make_current();
#endif
 // Retryable before callback entry.
  guest_pause_active_=true;int result=0;
  try{result=api_.pause_guest();}
  catch(...){guest_pause_active_=false;guest_pause_failed_=true;fault_=true;omni_owned_actor_exit(1);}
  guest_pause_active_=false;
  if(result!=1){guest_pause_failed_=true;fault_=true;throw Failure("runtime_crashed");}
  core_paused_=true;
}
void Engine::pause(){
  measurement_.cancel("playback_interrupted");running_=false;finger_down_=false;pointer_down_=false;pointer_inside_=false;audio_running_.store(false);
  if(presentation_)presentation_->playback(false,static_cast<unsigned>(std::lround(volume_*100)));
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
  const bool device_paused=true; // Playback control travels to the separate host.
#else
  const bool device_paused=!audio_||SDL_PauseAudioStreamDevice(audio_);
#endif
  if(!device_paused)fault_=true;
  // Even a failed device pause must not skip the guest producer barrier.
  pause_core();
  stop_rumble();reset_audio();
  if(!device_paused)throw Failure("runtime_crashed");

#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(window_)SDL_SetWindowTitle(window_,("OmniPlay — "+title_+(close_requested_?" · Return to Game Hub to save and close":" · Paused · Esc to resume")).c_str());
#endif

}
void Engine::resume(){
  if(!loaded_||core_phase_.load()!=CorePhase::Ready)throw Failure("invalid_state");
  if(guest_pause_active_||guest_pause_failed_||core_call_active_||core_call_failed_)throw Failure("runtime_crashed");
  if(fault_)throw Failure(fault_code_.load());
  if(running_)return; // Do not reset a ring while its guest producer is active.
  pause_core(); // Finish any earlier pre-entry pause failure before resetting audio.
  if(presentation_){
    if(!presentation_->wait_focused(std::chrono::milliseconds(750)))throw Failure("permission_denied");
    const auto view=presentation_->snapshot();
    if(!view.fresh||!view.focused||!presentation_->playback(true,static_cast<unsigned>(std::lround(volume_*100))))throw Failure("permission_denied");
    // Consume the already admitted focus epoch. A later epoch still pauses on
    // the next poll, while a held Escape cannot become a new resume edge.
    presentation_epoch_=view.epoch;presentation_escape_=presentation_key(view,SDL_SCANCODE_ESCAPE);presentation_input_seen_=false;
  }
  reset_audio();
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(audio_&&!SDL_ResumeAudioStreamDevice(audio_)){fault_=true;throw Failure("runtime_crashed");}
#endif
  close_requested_=false;core_paused_=false;running_=true;started_=true;audio_running_.store(true);deadline_=Clock::now();previous_frame_=Clock::time_point{};metrics_start_=deadline_;intervals_.clear();presented_=0;dropped_=0;underruns_.store(0);presentation_due_=Clock::time_point{};
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(window_)SDL_SetWindowTitle(window_,("OmniPlay — "+title_).c_str());
#endif

}

bool Engine::rumble(unsigned port,retro_rumble_effect effect,std::uint16_t strength){
  auto* e=active_;if(!e||port>=4||!e->input_profile_.haptics||(effect!=RETRO_RUMBLE_STRONG&&effect!=RETRO_RUMBLE_WEAK))return false;
  auto& value=e->rumble_[port];std::lock_guard lock(value.mutex);if(!e->running_||!value.available)return false;(effect==RETRO_RUMBLE_STRONG?value.strong:value.weak)=strength;return true;
}
void Engine::stop_rumble(){
  for(unsigned port=0;port<4;++port){auto& value=rumble_[port];
    {std::lock_guard lock(value.mutex);value.strong=value.weak=0;}
    if(presentation_&&(value.sent_strong||value.sent_weak))presentation_->rumble(port,0,0,0);
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
    else if(pads_[port]&&(value.sent_strong||value.sent_weak))SDL_RumbleGamepad(pads_[port],0,0,0);
#endif
    value.sent_strong=value.sent_weak=0;value.sent_at=0;
  }
}
void Engine::flush_rumble(){
  if(!running_||!input_profile_.haptics){stop_rumble();return;}const auto now=runtime_ticks();
  if(presentation_){for(unsigned port=0;port<4;port++){auto& value=rumble_[port];std::uint16_t strong=0,weak=0;{std::lock_guard lock(value.mutex);if(!value.available)continue;strong=value.strong;weak=value.weak;}if(!strong&&!weak){if(value.sent_strong||value.sent_weak)presentation_->rumble(port,0,0,0);value.sent_strong=value.sent_weak=0;continue;}if(now-value.sent_at<50)continue;if(presentation_->rumble(port,strong,weak,150)){value.sent_strong=strong;value.sent_weak=weak;value.sent_at=now;}}return;}
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  for(unsigned port=0;port<4;++port){auto& value=rumble_[port];if(!pads_[port])continue;std::uint16_t strong=0,weak=0;{std::lock_guard lock(value.mutex);if(!value.available)continue;strong=value.strong;weak=value.weak;}
    if(!strong&&!weak){if(value.sent_strong||value.sent_weak)SDL_RumbleGamepad(pads_[port],0,0,0);value.sent_strong=value.sent_weak=0;continue;}
    if(now-value.sent_at<50)continue;if(SDL_RumbleGamepad(pads_[port],strong,weak,150)){value.sent_strong=strong;value.sent_weak=weak;value.sent_at=now;}else{{std::lock_guard lock(value.mutex);value.available=false;value.strong=value.weak=0;}value.sent_strong=value.sent_weak=0;SDL_RumbleGamepad(pads_[port],0,0,0);}
  }
#endif
}
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
void Engine::pad_added(SDL_JoystickID id){for(auto* pad:pads_)if(pad&&SDL_GetGamepadID(pad)==id)return;for(unsigned port=0;port<4;++port)if(!pads_[port]){auto* pad=SDL_OpenGamepad(id);if(!pad)return;pads_[port]=pad;auto& value=rumble_[port];const bool available=SDL_GetBooleanProperty(SDL_GetGamepadProperties(pad),SDL_PROP_GAMEPAD_CAP_RUMBLE_BOOLEAN,false);{std::lock_guard lock(value.mutex);value.strong=value.weak=0;value.available=available;}value.sent_strong=value.sent_weak=0;value.sent_at=0;return;}}
void Engine::pad_removed(SDL_JoystickID id){for(unsigned port=0;port<4;++port)if(pads_[port]&&SDL_GetGamepadID(pads_[port])==id){auto& value=rumble_[port];{std::lock_guard lock(value.mutex);value.available=false;value.strong=value.weak=0;}SDL_RumbleGamepad(pads_[port],0,0,0);SDL_CloseGamepad(pads_[port]);pads_[port]=nullptr;value.sent_strong=value.sent_weak=0;}}
#endif
void Engine::poll(){
  if(presentation_){
    presentation_->progress();const auto view=presentation_->snapshot();if(view.unavailable){close_requested_=true;pause();return;}
    const bool escape=presentation_key(view,SDL_SCANCODE_ESCAPE);if(view.epoch!=presentation_epoch_){presentation_epoch_=view.epoch;presentation_escape_=escape;presentation_input_seen_=false;if(running_)pause();}
    if(!view.fresh||!view.focused){presentation_escape_=false;presentation_input_seen_=false;if(running_)pause();return;}
    if(presentation_input_seen_&&escape&&!presentation_escape_&&!close_requested_&&loaded_){if(running_)pause();else resume();}presentation_escape_=escape;presentation_input_seen_=true;
    if(view.presented<presentation_count_)throw Failure("runtime_crashed");const auto count=view.presented-presentation_count_;presentation_count_=view.presented;total_presented_.store(view.presented);presented_+=static_cast<std::uint32_t>(std::min<std::uint64_t>(count,std::numeric_limits<std::uint32_t>::max()-presented_));
    for(unsigned port=0;port<4;port++){auto& value=rumble_[port];std::lock_guard lock(value.mutex);value.available=view.haptics_admitted&&view.input[64+port*20+1]!=0;if(!value.available)value.strong=value.weak=0;}
    flush_rumble();return;
  }
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  SDL_Event event;while(SDL_PollEvent(&event)){
  if(event.type==SDL_EVENT_QUIT||event.type==SDL_EVENT_WINDOW_CLOSE_REQUESTED){close_requested_=true;pause();}
  else if(event.type==SDL_EVENT_WINDOW_FOCUS_LOST||event.type==SDL_EVENT_WINDOW_MINIMIZED||event.type==SDL_EVENT_WINDOW_HIDDEN)pause();
  else if(event.type==SDL_EVENT_GAMEPAD_ADDED)pad_added(event.gdevice.which);
  else if(event.type==SDL_EVENT_GAMEPAD_REMOVED)pad_removed(event.gdevice.which);
  else if(event.type==SDL_EVENT_FINGER_DOWN&&running_&&!finger_down_&&window_&&event.tfinger.windowID==SDL_GetWindowID(window_)){
    finger_=event.tfinger.fingerID;finger_down_=true;finger_x_=event.tfinger.x;finger_y_=event.tfinger.y;
  }else if(event.type==SDL_EVENT_FINGER_MOTION&&finger_down_&&event.tfinger.fingerID==finger_){finger_x_=event.tfinger.x;finger_y_=event.tfinger.y;}
  else if((event.type==SDL_EVENT_FINGER_UP||event.type==SDL_EVENT_FINGER_CANCELED)&&finger_down_&&event.tfinger.fingerID==finger_){finger_down_=false;pointer_down_=false;}
  else if(event.type==SDL_EVENT_KEY_DOWN&&!event.key.repeat){
    if(event.key.scancode==SDL_SCANCODE_ESCAPE&&!close_requested_){if(running_)pause();else if(loaded_)resume();}
    if(event.key.scancode==SDL_SCANCODE_F11&&window_)SDL_SetWindowFullscreen(window_,!(SDL_GetWindowFlags(window_)&SDL_WINDOW_FULLSCREEN));
  }
}flush_rumble();
#endif
}
void Engine::tick(){
  if(fault_)throw Failure(fault_code_.load());
  if(shutdown_requested_.exchange(false)){close_requested_=true;pause();return;}
  if(consume_av_changes())return;
  if(!running_)return;const auto before=Clock::now();if(before<deadline_)return;
  if(previous_frame_!=Clock::time_point{}){const auto elapsed=std::chrono::duration<double,std::milli>(before-previous_frame_).count();intervals_.push_back(elapsed);if(elapsed<=5000)active_time_ms_+=elapsed;if(intervals_.size()>1000){fault_=true;}}
  previous_frame_=before;
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(hardware_video_)gl_video_.make_current();
#endif
call_core(api_.run);if(fault_)throw Failure(fault_code_.load());
  if(shutdown_requested_.exchange(false)){close_requested_=true;pause();return;}
  if(consume_av_changes())return;
  const auto period=std::chrono::duration_cast<Clock::duration>(std::chrono::duration<double>(1.0/nominal_hz_));deadline_+=period;
  const auto now=Clock::now();if(now>deadline_+period*2){const auto missed=static_cast<std::uint64_t>((now-deadline_)/period);total_dropped_+=missed;dropped_+=static_cast<std::uint32_t>(std::min<std::uint64_t>(missed,std::numeric_limits<std::uint32_t>::max()-dropped_));deadline_=now;}
  measurement_.frame(now,nominal_hz_,total_presented_.load(),total_dropped_,total_underruns_.load());
  update_metrics();
}
void Engine::update_metrics(){
  const auto now=Clock::now();const auto duration=std::chrono::duration_cast<std::chrono::milliseconds>(now-metrics_start_).count();if(duration<1000)return;
  auto sorted=intervals_;std::sort(sorted.begin(),sorted.end());auto q=[&](double fraction){return sorted.empty()?0.0:sorted[static_cast<std::size_t>((sorted.size()-1)*fraction)];};
  latest_metrics_={{"window_duration_ms",std::min<std::int64_t>(duration,300000)},{"frames_presented",presented_},{"dropped_frames",dropped_},{"audio_underruns",underruns_.exchange(0)},{"frame_time_p50_ms",q(.50)},{"frame_time_p95_ms",q(.95)},{"frame_time_p99_ms",q(.99)}};
  intervals_.clear();presented_=0;dropped_=0;metrics_start_=now;
}

void Engine::video(const void* data,unsigned width,unsigned height,std::size_t pitch){try{video_impl(data,width,height,pitch);}catch(const Failure& error){if(active_){active_->fault_code_=error.code;active_->fault_=true;}}catch(...){if(active_){active_->fault_code_="runtime_crashed";active_->fault_=true;}}}
void Engine::video_impl(const void* data,unsigned width,unsigned height,std::size_t pitch){
  auto* e=active_;if(!e||!e->running_||!data)return;
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(e->hardware_video_){
    if(data!=RETRO_HW_FRAME_BUFFER_VALID||!width||!height||width>e->gl_video_.width()||height>e->gl_video_.height())throw Failure("runtime_crashed");
    e->texture_width_=width;e->texture_height_=height;const auto now=Clock::now();if(e->battery_&&now<e->presentation_due_)return;
    if(e->battery_)e->presentation_due_=std::max(now,e->presentation_due_+std::chrono::duration_cast<Clock::duration>(std::chrono::duration<double>(1.0/30.0)));
    if(e->gl_video_.present(width,height,e->aspect_ratio_)){++e->presented_;e->total_presented_.fetch_add(1);}return;
  }
#endif
  if(!e->renderer_&&!e->presentation_)throw Failure("runtime_crashed");
  if(data==RETRO_HW_FRAME_BUFFER_VALID||width==0||height==0||width>4096||height>4096||std::uint64_t(width)*height>4096u*4096u){e->fault_=true;return;}
  const auto bytes=e->pixel_format_==RETRO_PIXEL_FORMAT_XRGB8888?4u:2u;
  if(pitch<std::size_t(width)*bytes||pitch>64u*1024u*1024u/height){e->fault_=true;return;}
  const auto now=Clock::now();if(e->battery_&&now<e->presentation_due_)return;
  if(e->battery_)e->presentation_due_=std::max(now,e->presentation_due_+std::chrono::duration_cast<Clock::duration>(std::chrono::duration<double>(1.0/30.0)));
  if(e->presentation_){const auto view=e->presentation_->snapshot();if(width>view.maximum_width||height>view.maximum_height)throw Failure("unsupported_capability");}
  if(width!=e->texture_width_||height!=e->texture_height_){
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
    if(e->texture_)SDL_DestroyTexture(e->texture_);
    if(!e->presentation_){e->texture_=SDL_CreateTexture(e->renderer_,SDL_PIXELFORMAT_ARGB8888,SDL_TEXTUREACCESS_STREAMING,static_cast<int>(width),static_cast<int>(height));if(!e->texture_){e->fault_=true;return;}}
#endif
    e->texture_width_=width;e->texture_height_=height;e->pixels_.resize(std::size_t(width)*height);
  }
  const int logical_width=e->aspect_ratio_>0?static_cast<int>(std::lround(height*e->aspect_ratio_)):static_cast<int>(width);
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(!e->presentation_)SDL_SetRenderLogicalPresentation(e->renderer_,logical_width,static_cast<int>(height),SDL_LOGICAL_PRESENTATION_LETTERBOX);
#endif
  const auto* source=static_cast<const std::uint8_t*>(data);
  for(unsigned y=0;y<height;++y)for(unsigned x=0;x<width;++x){std::uint32_t out=0xff000000u;if(bytes==4){std::uint32_t value=0;std::memcpy(&value,source+y*pitch+x*4,4);out|=value&0x00ffffffu;}else{std::uint16_t value=0;std::memcpy(&value,source+y*pitch+x*2,2);unsigned r=0,g=0,b=0;if(e->pixel_format_==RETRO_PIXEL_FORMAT_RGB565){r=((value>>11)&31)*255/31;g=((value>>5)&63)*255/63;b=(value&31)*255/31;}else{r=((value>>10)&31)*255/31;g=((value>>5)&31)*255/31;b=(value&31)*255/31;}out|=(r<<16)|(g<<8)|b;}e->pixels_[std::size_t(y)*width+x]=out;}
  if(e->presentation_){const auto stamp=std::chrono::duration_cast<std::chrono::microseconds>(now.time_since_epoch()).count();if(stamp<0||logical_width<1)throw Failure("runtime_crashed");e->presentation_->video(e->pixels_,width,height,static_cast<unsigned>(logical_width),static_cast<std::uint64_t>(stamp));return;}
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(!SDL_UpdateTexture(e->texture_,nullptr,e->pixels_.data(),static_cast<int>(width*4))||!SDL_RenderClear(e->renderer_)||!SDL_RenderTexture(e->renderer_,e->texture_,nullptr,nullptr)||!SDL_RenderPresent(e->renderer_)){e->fault_=true;return;}++e->presented_;e->total_presented_.fetch_add(1);
#else
  throw Failure("runtime_crashed"); // A device-free actor cannot render without its host.
#endif
}

std::size_t Engine::audio_batch(const std::int16_t* data,std::size_t frames){
  auto* e=active_;if(!e)return 0;
  if((!data&&frames)||frames>AudioRing::batch_limit){e->fault_=true;return 0;}
  if(!frames)return 0;
  // Preparation and the guest pause barrier may flush pending audio. Discarding
  // it here is deliberate, so a stopped device cannot stall producer retirement.
  if(!e->audio_running_.load())return frames;
  if(e->fault_)return 0;
  if(e->presentation_){
    try{return e->presentation_->audio(std::span<const std::int16_t>(data,frames*2),e->presentation_rate_.load())?frames:0;}
    catch(...){e->fault_=true;return 0;}
  }
  if(!e->audio_)return frames;
  const auto receipt=e->audio_ring_.push(std::span<const std::int16_t>(data,frames*2));
  if(!receipt.valid)e->fault_=true;
  return receipt.frames;
}
void Engine::audio_sample(std::int16_t left,std::int16_t right){const std::array<std::int16_t,2> pair{left,right};audio_batch(pair.data(),1);}
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
void SDLCALL Engine::supply_audio(void* context,SDL_AudioStream* stream,int additional,int){
  auto* e=static_cast<Engine*>(context);if(additional<=0||e->fault_)return;
  std::array<std::int16_t,8192> output{};int remaining=std::min(additional,256*1024);
  while(remaining>0){
    const auto requested=std::min<std::size_t>((std::uint64_t(remaining)+3)/4,AudioRing::output_limit);
    const auto target=std::min<std::uint64_t>(AudioRing::capacity/2,std::max<std::uint64_t>(128,static_cast<std::uint64_t>(e->sample_rate_/e->nominal_hz_*e->latency_frames_)));
    const auto receipt=e->audio_ring_.deliver(std::span<std::int16_t>(output.data(),requested*2),target,
      static_cast<std::uint64_t>(e->sample_rate_*.05),e->volume_,[stream](std::span<const std::int16_t> samples){
        return SDL_PutAudioStreamData(stream,samples.data(),static_cast<int>(samples.size_bytes()));
      });
    if(receipt.delivery!=AudioRing::Delivery::Accepted){
      // Pinned SDL may copy a prefix before a later allocation fails. Stop this
      // actor's audio and clear its queued data; never replay an unknown prefix.
      e->fault_=true;SDL_ClearAudioStream(stream);return;
    }
    if(receipt.underrun&&e->audio_running_.load()){e->underruns_.fetch_add(1);e->total_underruns_.fetch_add(1);}
    remaining-=static_cast<int>(requested*4);
  }
}

void Engine::update_pointer(){
  pointer_down_=false;pointer_inside_=false;if(!running_||!window_||(!renderer_&&!hardware_video_)||!texture_height_)return;
  float x=0,y=0;bool pressed=false;
  if(finger_down_){int width=0,height=0;if(!SDL_GetWindowSize(window_,&width,&height))return;x=finger_x_*width;y=finger_y_*height;pressed=true;}
  else{if(SDL_GetMouseFocus()!=window_)return;const auto buttons=SDL_GetMouseState(&x,&y);pressed=(buttons&SDL_BUTTON_LMASK)!=0;}
  if(!std::isfinite(x)||!std::isfinite(y))return;double nx=0,ny=0;
  if(hardware_video_){
    int width=0,height=0;if(!SDL_GetWindowSize(window_,&width,&height)||width<=0||height<=0)return;
    const auto ratio=aspect_ratio_>0?aspect_ratio_:double(texture_width_)/texture_height_;double draw_w=width,draw_h=draw_w/ratio;
    if(draw_h>height){draw_h=height;draw_w=draw_h*ratio;}if(draw_w<=0||draw_h<=0)return;
    nx=(x-(width-draw_w)/2)/draw_w;ny=(y-(height-draw_h)/2)/draw_h;
  }else{
    float logical_x=0,logical_y=0;if(!SDL_RenderCoordinatesFromWindow(renderer_,x,y,&logical_x,&logical_y))return;
    const auto width=aspect_ratio_>0?std::lround(texture_height_*aspect_ratio_):static_cast<long>(texture_width_);
    if(width<=0||!std::isfinite(logical_x)||!std::isfinite(logical_y))return;nx=logical_x/width;ny=logical_y/texture_height_;
  }
  pointer_inside_=nx>=0&&nx<1&&ny>=0&&ny<1;pointer_down_=pressed&&pointer_inside_;
  pointer_x_=static_cast<std::int16_t>(std::lround(std::clamp(nx,0.0,1.0)*65534.0-32767.0));
  pointer_y_=static_cast<std::int16_t>(std::lround(std::clamp(ny,0.0,1.0)*65534.0-32767.0));
}
#endif
void Engine::input_poll(){if(auto* e=active_){if(e->presentation_)return;
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  e->update_pointer();for(unsigned port=0;port<4;++port)for(unsigned index=0;index<2;++index)e->stick_input_[port][index]=e->input_profile_.stick(e->pads_[port],index);
#endif
}}
std::int16_t Engine::input_state(unsigned port,unsigned device,unsigned index,unsigned id){
  auto* e=active_;if(!e||!e->running_||port>=4)return 0;device&=RETRO_DEVICE_MASK;
  if(e->presentation_)return presentation_input(e->presentation_->snapshot(),e->input_profile_,e->analog_controls_,port,device,index,id);
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(device==RETRO_DEVICE_POINTER){if(port||index)return 0;if(id==RETRO_DEVICE_ID_POINTER_X)return e->pointer_x_;if(id==RETRO_DEVICE_ID_POINTER_Y)return e->pointer_y_;if(id==RETRO_DEVICE_ID_POINTER_PRESSED)return e->pointer_down_?1:0;if(id==RETRO_DEVICE_ID_POINTER_COUNT)return e->pointer_down_?1:0;
#ifdef RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN
    if(id==RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN)return e->pointer_inside_?0:1;
#endif
    return 0;}
  if(device==RETRO_DEVICE_JOYPAD&&index)return 0;
  if(device==RETRO_DEVICE_JOYPAD&&id==RETRO_DEVICE_ID_JOYPAD_MASK){std::uint16_t mask=0;for(unsigned bit=0;bit<16;++bit)if(input_state(port,RETRO_DEVICE_JOYPAD,0,bit))mask|=static_cast<std::uint16_t>(1u<<bit);return static_cast<std::int16_t>(mask);}
  if(device==RETRO_DEVICE_ANALOG){
    if(index>1||id>1)return 0;
    if(port==0&&index==0&&e->analog_controls_){const auto* keys=SDL_GetKeyboardState(nullptr);const auto negative_key=e->input_profile_.analog[id==0?2:0],positive_key=e->input_profile_.analog[id==0?3:1];const bool negative=negative_key!=SDL_SCANCODE_UNKNOWN&&keys[negative_key],positive=positive_key!=SDL_SCANCODE_UNKNOWN&&keys[positive_key];if(negative||positive)return static_cast<std::int16_t>((int(positive)-int(negative))*32767);}
    return e->pads_[port]?e->stick_input_[port][index][id]:0;
  }
  if(device!=RETRO_DEVICE_JOYPAD||id>=16)return 0;
  const std::array<SDL_GamepadButton,16> buttons{SDL_GAMEPAD_BUTTON_SOUTH,SDL_GAMEPAD_BUTTON_WEST,SDL_GAMEPAD_BUTTON_BACK,SDL_GAMEPAD_BUTTON_START,SDL_GAMEPAD_BUTTON_DPAD_UP,SDL_GAMEPAD_BUTTON_DPAD_DOWN,SDL_GAMEPAD_BUTTON_DPAD_LEFT,SDL_GAMEPAD_BUTTON_DPAD_RIGHT,SDL_GAMEPAD_BUTTON_EAST,SDL_GAMEPAD_BUTTON_NORTH,SDL_GAMEPAD_BUTTON_LEFT_SHOULDER,SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER,SDL_GAMEPAD_BUTTON_INVALID,SDL_GAMEPAD_BUTTON_INVALID,SDL_GAMEPAD_BUTTON_LEFT_STICK,SDL_GAMEPAD_BUTTON_RIGHT_STICK};
  if(e->pads_[port]){auto* pad=e->pads_[port];const auto source=e->input_profile_.buttons[id];if(buttons[source]!=SDL_GAMEPAD_BUTTON_INVALID&&SDL_GetGamepadButton(pad,buttons[source]))return 1;if(source==12&&SDL_GetGamepadAxis(pad,SDL_GAMEPAD_AXIS_LEFT_TRIGGER)>16000)return 1;if(source==13&&SDL_GetGamepadAxis(pad,SDL_GAMEPAD_AXIS_RIGHT_TRIGGER)>16000)return 1;if(!e->analog_controls_){const auto x=e->stick_input_[port][0][0],y=e->stick_input_[port][0][1];if((source==4&&y< -13107)||(source==5&&y>13107)||(source==6&&x< -13107)||(source==7&&x>13107))return 1;}}
  if(port==0){const auto code=e->input_profile_.keyboard[id];return code!=SDL_SCANCODE_UNKNOWN&&SDL_GetKeyboardState(nullptr)[code]?1:0;}return 0;
#else
  return 0;
#endif
}

void Engine::quiet_log(enum retro_log_level,const char*,...){}
std::uintptr_t Engine::hardware_framebuffer(){return active_&&active_->hardware_video_?active_->gl_video_.framebuffer():0;}
bool Engine::environment(unsigned command,void* data){try{return environment_impl(command,data);}catch(const Failure& error){if(active_){active_->fault_code_=error.code;active_->fault_=true;}return false;}catch(...){if(active_){active_->fault_code_="runtime_crashed";active_->fault_=true;}return false;}}
bool Engine::environment_impl(unsigned command,void* data){
  auto* e=active_;if(!e||(command&RETRO_ENVIRONMENT_PRIVATE))return false;
  // Experimental commands retain their libretro-defined bit in the C macros.
  // GET_INPUT_BITMASKS deliberately permits a null data pointer.
  if(command==RETRO_ENVIRONMENT_GET_INPUT_BITMASKS)return true;
  if(command==RETRO_ENVIRONMENT_SHUTDOWN){const auto phase=e->core_phase_.load();if(phase==CorePhase::Stopping)return true;if(phase==CorePhase::Ready&&e->started_)e->shutdown_requested_=true;else{e->fault_code_="content_mismatch";e->fault_=true;}return true;}
  if(!data)return false;
  switch(command){
    case RETRO_ENVIRONMENT_GET_CAN_DUPE:*static_cast<bool*>(data)=true;return true;
    case RETRO_ENVIRONMENT_GET_PREFERRED_HW_RENDER:*static_cast<retro_hw_context_type*>(data)=e->hardware_video_?RETRO_HW_CONTEXT_OPENGL_CORE:RETRO_HW_CONTEXT_NONE;return true;
    case RETRO_ENVIRONMENT_SET_HW_RENDER:{
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
      return false;
#else
      if(!e->hardware_video_)return false;auto& callback=*static_cast<retro_hw_render_callback*>(data);if(!e->gl_video_.negotiate(callback))return false;callback.get_current_framebuffer=hardware_framebuffer;callback.get_proc_address=GlVideo::procedure;return true;
#endif
    }
    case RETRO_ENVIRONMENT_SET_PIXEL_FORMAT:{auto format=*static_cast<retro_pixel_format*>(data);if(format!=RETRO_PIXEL_FORMAT_0RGB1555&&format!=RETRO_PIXEL_FORMAT_RGB565&&format!=RETRO_PIXEL_FORMAT_XRGB8888)return false;e->pixel_format_=static_cast<unsigned>(format);return true;}
    case RETRO_ENVIRONMENT_GET_SYSTEM_DIRECTORY:case RETRO_ENVIRONMENT_GET_CORE_ASSETS_DIRECTORY:*static_cast<const char**>(data)=e->system_directory_.c_str();return true;
    case RETRO_ENVIRONMENT_GET_SAVE_DIRECTORY:*static_cast<const char**>(data)=e->saved_files_.enabled()?e->core_save_directory_.c_str():".";return true;
    case RETRO_ENVIRONMENT_GET_LIBRETRO_PATH:*static_cast<const char**>(data)=e->core_path_.c_str();return true;
    case RETRO_ENVIRONMENT_GET_LOG_INTERFACE:static_cast<retro_log_callback*>(data)->log=quiet_log;return true;
    case RETRO_ENVIRONMENT_GET_VARIABLE:{auto* v=static_cast<retro_variable*>(data);if(!v->key)return false;const auto found=e->variables_.find(v->key);v->value=found==e->variables_.end()?nullptr:found->second.c_str();return found!=e->variables_.end();}
    case RETRO_ENVIRONMENT_SET_VARIABLES:{const auto* v=static_cast<const retro_variable*>(data);for(unsigned n=0;n<512&&v[n].key;++n){if(!v[n].value)continue;const std::string definition(v[n].value);const auto separator=definition.find(';');if(separator==std::string::npos)continue;auto value=definition.substr(separator+1);value.erase(0,value.find_first_not_of(' '));value=value.substr(0,value.find('|'));if(std::strlen(v[n].key)<=128&&value.size()<=256)e->variables_.emplace(v[n].key,value);}return true;}
    case RETRO_ENVIRONMENT_GET_VARIABLE_UPDATE:*static_cast<bool*>(data)=false;return true;
    case RETRO_ENVIRONMENT_GET_CORE_OPTIONS_VERSION:*static_cast<unsigned*>(data)=2;return true;
    case RETRO_ENVIRONMENT_SET_DISK_CONTROL_INTERFACE:{e->discs_=*static_cast<const retro_disk_control_callback*>(data);return true;}
    case RETRO_ENVIRONMENT_GET_DISK_CONTROL_INTERFACE_VERSION:*static_cast<unsigned*>(data)=1;return true;
    case RETRO_ENVIRONMENT_SET_DISK_CONTROL_EXT_INTERFACE:{const auto* d=static_cast<const retro_disk_control_ext_callback*>(data);e->discs_={d->set_eject_state,d->get_eject_state,d->get_image_index,d->set_image_index,d->get_num_images,d->replace_image_index,d->add_image_index};return true;}
    case RETRO_ENVIRONMENT_SET_SYSTEM_AV_INFO:{const auto av=*static_cast<const retro_system_av_info*>(data);if(!e->valid_av(av))throw Failure("unsupported_capability");std::lock_guard lock(e->av_mutex_);e->pending_av_=av;e->pending_geometry_.reset();return true;}
    case RETRO_ENVIRONMENT_SET_GEOMETRY:{const auto geometry=*static_cast<const retro_game_geometry*>(data);if(!std::isfinite(geometry.aspect_ratio)||geometry.aspect_ratio<0||geometry.aspect_ratio>10)return false;if(e->hardware_video_&&(!geometry.base_width||!geometry.base_height||geometry.base_width>e->gl_video_.width()||geometry.base_height>e->gl_video_.height()))return false;std::lock_guard lock(e->av_mutex_);e->pending_geometry_=geometry;return true;}
    case RETRO_ENVIRONMENT_GET_AUDIO_VIDEO_ENABLE:*static_cast<int*>(data)=3;return true;
    case RETRO_ENVIRONMENT_SET_CORE_OPTIONS:{const auto* definitions=static_cast<const retro_core_option_definition*>(data);for(unsigned n=0;n<512&&definitions[n].key;++n){const auto& d=definitions[n];const char* value=d.default_value?d.default_value:d.values[0].value;if(value&&std::strlen(d.key)<=128&&std::strlen(value)<=256)e->variables_.emplace(d.key,value);}return true;}
    case RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2:case RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2_INTL:{const auto* options=command==RETRO_ENVIRONMENT_SET_CORE_OPTIONS_V2?static_cast<const retro_core_options_v2*>(data):static_cast<const retro_core_options_v2_intl*>(data)->us;if(!options||!options->definitions)return false;for(unsigned n=0;n<512&&options->definitions[n].key;++n){const auto& d=options->definitions[n];const char* value=d.default_value?d.default_value:d.values[0].value;if(value&&std::strlen(d.key)<=128&&std::strlen(value)<=256)e->variables_.emplace(d.key,value);}return true;}
    case RETRO_ENVIRONMENT_SET_INPUT_DESCRIPTORS:case RETRO_ENVIRONMENT_SET_CONTROLLER_INFO:case RETRO_ENVIRONMENT_SET_SUPPORT_NO_GAME:return true;
    case RETRO_ENVIRONMENT_GET_RUMBLE_INTERFACE:if(!e->input_profile_.haptics)return false;static_cast<retro_rumble_interface*>(data)->set_rumble_state=rumble;return true;
    case RETRO_ENVIRONMENT_GET_INPUT_DEVICE_CAPABILITIES:*static_cast<std::uint64_t*>(data)=(1ull<<RETRO_DEVICE_JOYPAD)|(1ull<<RETRO_DEVICE_ANALOG)|(1ull<<RETRO_DEVICE_POINTER);return true;
    case RETRO_ENVIRONMENT_GET_INPUT_BITMASKS:return true;
    case RETRO_ENVIRONMENT_GET_USERNAME:*static_cast<const char**>(data)="OmniPlay";return true;
    case RETRO_ENVIRONMENT_GET_INPUT_MAX_USERS:*static_cast<unsigned*>(data)=4;return true;
    case RETRO_ENVIRONMENT_GET_LANGUAGE:*static_cast<unsigned*>(data)=RETRO_LANGUAGE_ENGLISH;return true;
    case RETRO_ENVIRONMENT_GET_FASTFORWARDING:*static_cast<bool*>(data)=false;return true;
    case RETRO_ENVIRONMENT_GET_TARGET_REFRESH_RATE:*static_cast<float*>(data)=static_cast<float>(e->nominal_hz_);return true;
    case RETRO_ENVIRONMENT_GET_JIT_CAPABLE:*static_cast<bool*>(data)=false;return true;
    default:return false;
  }
}

static void put32(std::vector<std::uint8_t>& target,std::size_t offset,std::uint32_t value){for(unsigned n=0;n<4;++n)target[offset+n]=static_cast<std::uint8_t>(value>>(n*8));}
static std::uint32_t get32(std::span<const std::uint8_t> source,std::size_t offset){return std::uint32_t(source[offset])|(std::uint32_t(source[offset+1])<<8)|(std::uint32_t(source[offset+2])<<16)|(std::uint32_t(source[offset+3])<<24);}
void Engine::flush_files(){
  if(!saved_files_.enabled())return;if(running_||!core_paused_)throw Failure("invalid_state");
  if(api_.flush_io_checked){const auto result=call_core(api_.flush_io_checked);if(result==0)throw Failure("storage_failure");if(result!=1){fault_=true;throw Failure("runtime_crashed");}}
  else{if(!api_.flush_io)throw Failure("runtime_crashed");call_core(api_.flush_io);}
  if(md_portable_&&md_contract_){try{if(call_core(&MdSaveContract::read,api_.describe_md_save)!=*md_contract_)throw Failure("runtime_crashed");}catch(...){md_layout_failed_=true;fault_=true;throw Failure("runtime_crashed");}}
}
std::vector<std::uint8_t> Engine::save_memory(bool allow_empty){
  const auto ram=call_core(api_.memory_size,RETRO_MEMORY_SAVE_RAM),rtc=call_core(api_.memory_size,RETRO_MEMORY_RTC);const auto* a=call_core(api_.memory_data,RETRO_MEMORY_SAVE_RAM);const auto* b=call_core(api_.memory_data,RETRO_MEMORY_RTC);
  if(!ram&&!rtc&&!allow_empty)throw Failure("unsupported_capability");if(ram>kMaxSave||rtc>kMaxSave||ram+rtc>kMaxSave-16||(ram&&!a)||(rtc&&!b))throw Failure("runtime_crashed");
  std::vector<std::uint8_t> out(16+ram+rtc);std::memcpy(out.data(),"OPRM0001",8);put32(out,8,static_cast<std::uint32_t>(ram));put32(out,12,static_cast<std::uint32_t>(rtc));if(ram)std::memcpy(out.data()+16,a,ram);if(rtc)std::memcpy(out.data()+16+ram,b,rtc);return out;
}
void Engine::validate_memory(std::span<const std::uint8_t> data){
  if(data.size()<16||std::memcmp(data.data(),"OPRM0001",8))throw Failure("incompatible_save");const auto ram=get32(data,8),rtc=get32(data,12);
  if(ram!=call_core(api_.memory_size,RETRO_MEMORY_SAVE_RAM)||rtc!=call_core(api_.memory_size,RETRO_MEMORY_RTC)||std::uint64_t(ram)+rtc+16!=data.size()||(ram&&!call_core(api_.memory_data,RETRO_MEMORY_SAVE_RAM))||(rtc&&!call_core(api_.memory_data,RETRO_MEMORY_RTC)))throw Failure("incompatible_save");
}
void Engine::load_memory(std::span<const std::uint8_t> data){
  validate_memory(data);const auto ram=get32(data,8),rtc=get32(data,12);if(ram)std::memcpy(call_core(api_.memory_data,RETRO_MEMORY_SAVE_RAM),data.data()+16,ram);if(rtc)std::memcpy(call_core(api_.memory_data,RETRO_MEMORY_RTC),data.data()+16+ram,rtc);if(reset_after_ram_load_)call_core(api_.reset);
}
std::vector<std::uint8_t> Engine::save(const std::string& kind){
  if(kind!="in_game"&&kind!="emulator_state")throw Failure("unsupported_capability");

#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(hardware_video_)gl_video_.make_current();
#endif
flush_files();
  if(kind=="in_game"){
    if(!saved_files_.enabled())return save_memory(false);
    if(!filesystem_save_memory_){if(md_portable_){if(!md_contract_)throw Failure("runtime_crashed");const auto expected=md_contract_->shapes();if(expected.empty())throw Failure("unsupported_capability");auto archive=saved_files_.read(&expected);md_contract_->validate(archive);return archive;}return saved_files_.read();}
    auto memory=save_memory(true);flush_files();auto files=saved_files_.read();const auto total=std::uint64_t(16)+memory.size()+files.size();if(total>kMaxSave)throw Failure("resource_limit");
    std::vector<std::uint8_t> out(static_cast<std::size_t>(total));std::memcpy(out.data(),"OPNM0001",8);put32(out,8,static_cast<std::uint32_t>(memory.size()));put32(out,12,static_cast<std::uint32_t>(files.size()));std::copy(memory.begin(),memory.end(),out.begin()+16);std::copy(files.begin(),files.end(),out.begin()+16+memory.size());return out;
  }
  const auto header=saved_files_.enabled()?24u:20u;const auto size=call_core(api_.serialize_size);if(!size)throw Failure("unsupported_capability");if(size>kMaxSave-header)throw Failure("resource_limit");
  const auto disc=disc_status();std::vector<std::uint8_t> state(size);if(!call_core(api_.serialize,state.data(),state.size()))throw Failure("runtime_crashed");flush_files();
  auto files=saved_files_.enabled()?saved_files_.read():std::vector<std::uint8_t>{};if(disc_status()!=disc)throw Failure("invalid_state");
  if(std::uint64_t(size)+header+files.size()>kMaxSave)throw Failure("resource_limit");
  std::vector<std::uint8_t> out(size+header+files.size());std::memcpy(out.data(),saved_files_.enabled()?"OPNS0002":"OPNS0001",8);put32(out,8,disc.at("supported").get<bool>()?disc.at("index").get<unsigned>():0xffffffffu);put32(out,12,disc.at("ejected").get<bool>()?1u:0u);put32(out,16,static_cast<std::uint32_t>(size));
  if(saved_files_.enabled())put32(out,20,static_cast<std::uint32_t>(files.size()));std::copy(state.begin(),state.end(),out.begin()+header);std::copy(files.begin(),files.end(),out.begin()+header+size);return out;
}
void Engine::retire_guest(){
  if(core_call_active_||core_call_failed_||guest_pause_active_||guest_pause_failed_)throw Failure("runtime_crashed");
  if(hardware_video_&&(gl_video_.core_callback_active()||gl_video_.core_callback_failed()))throw Failure("invalid_state");
  pause_core();core_phase_=CorePhase::Stopping;
  if(loaded_){
    call_core(api_.unload_game);loaded_=false;
  }
  // GlVideo owns its own retry-before-entry/unknown-after-entry receipts.
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(hardware_video_)gl_video_.release_core();
#endif
  if(initialized_){
    call_core(api_.deinit);initialized_=false;
  }
  discs_={};
}
void Engine::stop_content(){
  if(running_||audio_running_.load())throw Failure("invalid_state");
  retire_guest();
  if(memory_files_installed_){
    // A void unload/deinit is not proof that producers released this table.
    // Obtain the real core's quiescent acknowledgement before any save swap.
    if(call_core(api_.install_files,nullptr)!=1){fault_=true;throw Failure("runtime_crashed");}
    memory_files_installed_=false;
    memory_files_interface_=memory_files_.interface();
    // Conservatively retain callback ownership if the new acknowledgement is
    // refused; shutdown must still obtain uninstall or exit the owned actor.
    memory_files_installed_=true;
    if(call_core(api_.install_files,&memory_files_interface_)!=1){fault_=true;throw Failure("runtime_crashed");}
  }
}
bool Engine::restart_content(){
  // All old guest handles/producers retire before the private directory swap.
  stop_content();fault_=false;fault_code_="runtime_crashed";shutdown_requested_=false;core_paused_=true;core_phase_=CorePhase::Loading;
  call_core(api_.set_environment,environment);call_core(api_.set_video,video);call_core(api_.set_audio,audio_sample);call_core(api_.set_audio_batch,audio_batch);call_core(api_.set_input_poll,input_poll);call_core(api_.set_input_state,input_state);
  initialized_=true;call_core(api_.init);if(fault_)return false;retro_game_info game{};game.path=filename_.c_str();if(!info_.need_fullpath){game.data=rom_.data();game.size=rom_.size();}
  loaded_=call_core(api_.load_game,&game);if(loaded_)core_paused_=false;
  if(!loaded_||fault_)return false;complete_preparation();for(unsigned port=0;port<4;++port)call_core(api_.controller,port,RETRO_DEVICE_JOYPAD);read_av();return !fault_;
}
void Engine::load(const std::string& kind,const std::vector<std::uint8_t>& payload){
  if(payload.empty()||payload.size()>kMaxSave)throw Failure("incompatible_save");const bool restore_running=running_;pause();
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(hardware_video_)gl_video_.make_current();
#endif

  if(saved_files_.enabled()){
    std::span<const std::uint8_t> archive,core_state,memory;const auto view=std::span<const std::uint8_t>(payload);const auto before_disc=disc_status();unsigned target_index=0xffffffffu;bool target_ejected=false;
    auto valid_disc=[&](unsigned index,bool ejected){return index==0xffffffffu?!before_disc.at("supported").get<bool>()&&!ejected:before_disc.at("supported").get<bool>()&&index<before_disc.at("count").get<unsigned>();};
    if(kind=="in_game"){
      if(filesystem_save_memory_){
        if(view.size()<44||std::memcmp(view.data(),"OPNM0001",8)||get32(view,8)<16||get32(view,12)<12||std::uint64_t(get32(view,8))+get32(view,12)+16!=view.size())throw Failure("incompatible_save");
        memory=view.subspan(16,get32(view,8));archive=view.subspan(16+get32(view,8));validate_memory(memory);
      }else{archive=md_portable_?MdSaveContract::files(view):view;if(md_portable_){if(!md_contract_)throw Failure("runtime_crashed");md_contract_->validate(archive);}}
    }else if(kind=="emulator_state"){
      if(view.size()<24||std::memcmp(view.data(),"OPNS0002",8)||get32(view,12)>1||!get32(view,16)||get32(view,20)<12||std::uint64_t(get32(view,16))+get32(view,20)+24!=view.size())throw Failure("incompatible_save");
      target_index=get32(view,8);target_ejected=get32(view,12)==1;if(!valid_disc(target_index,target_ejected))throw Failure("incompatible_save");core_state=view.subspan(24,get32(view,16));archive=view.subspan(24+get32(view,16));
    }else throw Failure("unsupported_capability");
    // Full decode and memory/metadata admission precede any file replacement.
    SavedFiles::validate(archive);flush_files();const auto old_size=call_core(api_.serialize_size);if(!old_size||old_size>kMaxSave)throw Failure("resource_limit");
    std::vector<std::uint8_t> old_state(old_size);if(!call_core(api_.serialize,old_state.data(),old_state.size()))throw Failure("runtime_crashed");flush_files();const auto old_files=saved_files_.read();if(disc_status()!=before_disc)throw Failure("invalid_state");
    auto same_list=[&]{const auto current=disc_status();return current.at("supported")==before_disc.at("supported")&&current.at("count")==before_disc.at("count");};
    auto matches=[&](unsigned index,bool ejected){const auto current=disc_status();return same_list()&&(index==0xffffffffu?!current.at("supported").get<bool>():current.at("index").get<unsigned>()==index&&current.at("ejected").get<bool>()==ejected);};
    const char* rejected=nullptr;
    try{
      stop_content();saved_files_.replace(archive);if(!restart_content()||!same_list())throw Failure("incompatible_save");
      if(!core_state.empty()&&target_index!=0xffffffffu&&!switch_disc(target_index,target_ejected))throw Failure("incompatible_save");
      if(!core_state.empty()&&(!call_core(api_.unserialize,core_state.data(),core_state.size())||!matches(target_index,target_ejected)))throw Failure("incompatible_save");
      if(!memory.empty())load_memory(memory);if(fault_)throw Failure("incompatible_save");flush_files();read_av();
    }catch(const Failure& error){if(md_layout_failed_)throw Failure("runtime_crashed");rejected=std::strcmp(error.code,"storage_failure")==0?"storage_failure":std::strcmp(error.code,"resource_limit")==0?"resource_limit":"incompatible_save";}
    catch(const std::filesystem::filesystem_error&){rejected="storage_failure";}
    catch(const std::bad_alloc&){rejected="resource_limit";}
    catch(...){rejected="incompatible_save";}
    if(rejected){
      try{
        stop_content();saved_files_.replace(old_files);if(!restart_content()||!same_list())throw Failure("runtime_crashed");
        const auto index=before_disc.at("supported").get<bool>()?before_disc.at("index").get<unsigned>():0xffffffffu;const auto ejected=before_disc.at("ejected").get<bool>();
        if(index!=0xffffffffu&&!switch_disc(index,ejected))throw Failure("runtime_crashed");
        if(!call_core(api_.unserialize,old_state.data(),old_state.size())||!matches(index,ejected)||fault_)throw Failure("runtime_crashed");flush_files();read_av();
      }catch(...){fault_=true;throw Failure("runtime_crashed");}
      reset_audio();if(restore_running)resume();throw Failure(rejected);
    }
    reset_audio();if(restore_running)resume();return;
  }
  if(kind=="in_game")load_memory(payload);
  else if(kind=="emulator_state"){
    if(payload.size()<20||std::memcmp(payload.data(),"OPNS0001",8)||get32(payload,16)!=payload.size()-20||get32(payload,12)>1)throw Failure("incompatible_save");
    const auto index=get32(payload,8);const auto disc=disc_status();const auto ejected=get32(payload,12)==1;
    if((index==0xffffffffu&&(disc.at("supported").get<bool>()||ejected))||(index!=0xffffffffu&&(!disc.at("supported").get<bool>()||index>=disc.at("count").get<unsigned>())))throw Failure("incompatible_save");
    const auto before=save(kind);const bool selected=index==0xffffffffu||switch_disc(index,ejected);
    const auto matches=[&](unsigned expected,bool open){const auto current=disc_status();return current.at("count")==disc.at("count")&&(expected==0xffffffffu?!current.at("supported").get<bool>():current.at("index").get<unsigned>()==expected&&current.at("ejected").get<bool>()==open);};
    if(!selected||!call_core(api_.unserialize,payload.data()+20,payload.size()-20)||!matches(index,ejected)){
      const auto previous=get32(before,8);const auto open=get32(before,12)==1;
      if((previous!=0xffffffffu&&!switch_disc(previous,open))||!call_core(api_.unserialize,before.data()+20,before.size()-20)||!matches(previous,open)){fault_=true;throw Failure("runtime_crashed");}
      if(restore_running)resume();throw Failure("incompatible_save");
    }
  }else throw Failure("unsupported_capability");
  read_av();reset_audio();if(restore_running)resume();
}
void Engine::shutdown(){
  if(guest_pause_active_||guest_pause_failed_||core_call_active_||core_call_failed_)omni_owned_actor_exit(1);
  if(hardware_video_&&(gl_video_.core_callback_active()||gl_video_.core_callback_failed()))omni_owned_actor_exit(1);
  measurement_.cancel("runtime_stopped");
  core_phase_=CorePhase::Stopping;
  running_=false;audio_running_.store(false);
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(audio_)SDL_PauseAudioStreamDevice(audio_);
#endif
  // Unknown pause/unload/deinit ownership may not unwind through DLL/static or
  // callback-context destruction. The broker still waits for actual job exit.
  try{retire_guest();}catch(...){omni_owned_actor_exit(1);}
  if(memory_files_installed_){
    // An unconfirmed producer/handle lifetime cannot survive core unload or
    // release of its callback context. This executable is the owned actor;
    // retain all state until actual process retirement, without a stop ack.
    if(call_core(api_.install_files,nullptr)!=1)omni_owned_actor_exit(1);
    memory_files_installed_=false;
  }
  // Guest producers and GPU queues are joined before their audio sink vanishes.
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  if(audio_){SDL_DestroyAudioStream(audio_);audio_=nullptr;}
#endif
  // Core DLL static destructors also run while their graphics context is alive.
  // No retained callback may outlive the library containing its function body.
  if(object_){
    try{unload_core_module(object_);}catch(...){omni_owned_actor_exit(1);}
    object_=nullptr;
  }
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  gl_video_.destroy();
#endif
  stop_rumble();
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  for(auto*& pad:pads_)if(pad){SDL_CloseGamepad(pad);pad=nullptr;}
  if(texture_){SDL_DestroyTexture(texture_);texture_=nullptr;}if(renderer_){SDL_DestroyRenderer(renderer_);renderer_=nullptr;}if(window_){SDL_DestroyWindow(window_);window_=nullptr;}
  SDL_Quit();
#endif
  if(presentation_)presentation_->close();active_=nullptr;
}
}
