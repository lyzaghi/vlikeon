#pragma once
#include "protocol.hpp"
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
#include "../../presentation/src/raw_joystick.hpp"
#endif
#include <SDL3/SDL.h>
#include <libretro.h>
#include <array>
#include <algorithm>
#include <cmath>
#include <cctype>
#include <cstdint>
#include <set>
#include <string>
#include <vector>

namespace omni {
struct InputProfile {
  bool haptics=false;
  bool pressure_buttons=false;
  unsigned analog_deadzone_percent=15;
  std::array<SDL_Scancode,16> keyboard{SDL_SCANCODE_Z,SDL_SCANCODE_A,SDL_SCANCODE_RSHIFT,SDL_SCANCODE_RETURN,SDL_SCANCODE_UP,SDL_SCANCODE_DOWN,SDL_SCANCODE_LEFT,SDL_SCANCODE_RIGHT,SDL_SCANCODE_X,SDL_SCANCODE_S,SDL_SCANCODE_Q,SDL_SCANCODE_W,SDL_SCANCODE_E,SDL_SCANCODE_R,SDL_SCANCODE_D,SDL_SCANCODE_F};
  std::array<SDL_Scancode,4> analog{SDL_SCANCODE_I,SDL_SCANCODE_K,SDL_SCANCODE_J,SDL_SCANCODE_L};
  std::array<unsigned,16> buttons{0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15};
  struct Axis{unsigned source;bool inverted=false;unsigned sensitivity=100,curve=100;};
  std::array<Axis,4> axes{{{0},{1},{2},{3}}};

  enum class PortDevice : std::uint8_t { Auto=0, Joypad=1, AnalogJoypad=2, Mouse=3, Lightgun=4, None=5 };
  struct PortConfig {
    PortDevice device=PortDevice::Auto;
    unsigned subclass=0;
    unsigned pressure_curve=100;
    unsigned retro_device_id()const{
      unsigned base=RETRO_DEVICE_JOYPAD;
      switch(device){
        case PortDevice::None:return RETRO_DEVICE_NONE;
        case PortDevice::Auto:case PortDevice::Joypad:base=RETRO_DEVICE_JOYPAD;break;
        case PortDevice::AnalogJoypad:base=RETRO_DEVICE_ANALOG;break;
        case PortDevice::Mouse:base=RETRO_DEVICE_MOUSE;break;
        case PortDevice::Lightgun:base=RETRO_DEVICE_LIGHTGUN;break;
      }
      return subclass>0?RETRO_DEVICE_SUBCLASS(base,subclass-1u):base;
    }
  };
  std::array<PortConfig,4> controller_ports{};
#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  std::vector<presentation::SdlJoystickCalibration> sdl_raw_joysticks;
#endif

  std::int16_t shape_pressure(std::int16_t raw_positive,unsigned port)const{
    if(raw_positive<=0||port>=4)return 0;
    const double norm=std::clamp(double(raw_positive)/32767.0,0.0,1.0);
    const double curved=std::pow(norm,double(controller_ports[port].pressure_curve)/100.0);
    return static_cast<std::int16_t>(std::lround(std::clamp(curved,0.0,1.0)*32767.0));
  }

#ifndef OMNIPLAY_DEVICE_FREE_ACTOR
  std::array<std::int16_t,2> stick(SDL_Gamepad* pad,unsigned index)const{
    if(!pad||index>1)return {0,0};
    std::array<std::int16_t,4> values{};for(unsigned i=0;i<4;++i)values[i]=SDL_GetGamepadAxis(pad,static_cast<SDL_GamepadAxis>(i));
    return stick_values(values,index);
  }
#endif
  std::array<std::int16_t,2> stick_values(const std::array<std::int16_t,4>& values,unsigned index)const{
    if(index>1)return{0,0};const auto offset=index*2;
    if(axes[offset].source>=4||axes[offset+1].source>=4)return{0,0};
    auto result=stick_axes(values[axes[offset].source],values[axes[offset+1].source]);
    for(unsigned i=0;i<2;++i){const auto& tuning=axes[offset+i];double value=double(result[i])/32767.0;
      value=std::copysign(std::pow(std::abs(value),double(tuning.curve)/100.0),value)*double(tuning.sensitivity)/100.0*(tuning.inverted?-1:1);
      value=std::clamp(value,-1.0,1.0)*32767.0;result[i]=static_cast<std::int16_t>(value<0?-std::floor(-value+.5):std::floor(value+.5));}
    return result;
  }
  std::array<std::int16_t,2> stick_axes(std::int16_t raw_x,std::int16_t raw_y)const{
    if(analog_deadzone_percent>50)return{0,0};
    const double x=double(raw_x)/(raw_x<0?32768.0:32767.0),y=double(raw_y)/(raw_y<0?32768.0:32767.0),length=std::hypot(x,y),deadzone=double(analog_deadzone_percent)/100.0;
    if(length<=deadzone||length==0)return {0,0};const double scale=(std::min(1.0,length)-deadzone)/((1.0-deadzone)*length);
    const auto axis=[&](double value){const double scaled=std::clamp(value*scale,-1.0,1.0)*32767.0;return static_cast<std::int16_t>(scaled<0?-std::floor(-scaled+.5):std::floor(scaled+.5));};return {axis(x),axis(y)};
  }
  static SDL_Scancode scan(const std::string& code){
    if(code.size()==4&&code.starts_with("Key")&&code[3]>='A'&&code[3]<='Z')return static_cast<SDL_Scancode>(SDL_SCANCODE_A+code[3]-'A');
    if(code.size()==6&&code.starts_with("Digit")&&code[5]>='0'&&code[5]<='9')return code[5]=='0'?SDL_SCANCODE_0:static_cast<SDL_Scancode>(SDL_SCANCODE_1+code[5]-'1');
    if(code=="ArrowUp")return SDL_SCANCODE_UP;if(code=="ArrowDown")return SDL_SCANCODE_DOWN;if(code=="ArrowLeft")return SDL_SCANCODE_LEFT;if(code=="ArrowRight")return SDL_SCANCODE_RIGHT;
    if(code=="Space")return SDL_SCANCODE_SPACE;if(code=="Enter")return SDL_SCANCODE_RETURN;if(code=="Tab")return SDL_SCANCODE_TAB;if(code=="Backspace")return SDL_SCANCODE_BACKSPACE;if(code=="ShiftLeft")return SDL_SCANCODE_LSHIFT;if(code=="ShiftRight")return SDL_SCANCODE_RSHIFT;return SDL_SCANCODE_UNKNOWN;
  }
  bool load(const Json& value){
    try{
      if(!value.is_object()||value.size()<4||value.size()>11)return false;
      for(const auto& item:value.items()){
        const auto& k=item.key();
        if(k!="schema_version"&&k!="keyboard"&&k!="analog_keyboard"&&k!="button_sources"&&k!="haptics_enabled"&&k!="analog_deadzone_percent"&&k!="axis_tuning"&&k!="raw_gamepads"&&k!="sdl_raw_joysticks"&&k!="pressure_buttons_enabled"&&k!="controller_ports")return false;
      }
      if(!value.at("schema_version").is_number_integer()||value.at("schema_version")!=1||!value.at("keyboard").is_array()||value.at("keyboard").size()!=16||!value.at("analog_keyboard").is_array()||value.at("analog_keyboard").size()!=4||!value.at("button_sources").is_array()||value.at("button_sources").size()!=16)return false;
      InputProfile next;std::set<SDL_Scancode> used;
      if(value.contains("haptics_enabled")){if(!value.at("haptics_enabled").is_boolean())return false;next.haptics=value.at("haptics_enabled").get<bool>();}
      if(value.contains("pressure_buttons_enabled")){if(!value.at("pressure_buttons_enabled").is_boolean())return false;next.pressure_buttons=value.at("pressure_buttons_enabled").get<bool>();}
      if(value.contains("analog_deadzone_percent")){const auto& raw=value.at("analog_deadzone_percent");if(!raw.is_number_integer()||raw<0||raw>50)return false;next.analog_deadzone_percent=raw.get<unsigned>();}
      if(value.contains("axis_tuning")){
        const auto& list=value.at("axis_tuning");if(!list.is_array()||list.size()!=4)return false;std::set<unsigned> sources;
        for(unsigned i=0;i<4;++i){const auto& raw=list[i];
          if(!raw.is_object()||raw.size()!=4||!raw.contains("source")||!raw.contains("inverted")||!raw.contains("sensitivity_percent")||!raw.contains("curve_percent")||!raw["source"].is_number_integer()||raw["source"]<0||raw["source"]>3||!raw["inverted"].is_boolean()||!raw["sensitivity_percent"].is_number_integer()||raw["sensitivity_percent"]<50||raw["sensitivity_percent"]>200||!raw["curve_percent"].is_number_integer()||raw["curve_percent"]<50||raw["curve_percent"]>300)return false;
          next.axes[i]={raw["source"].get<unsigned>(),raw["inverted"].get<bool>(),raw["sensitivity_percent"].get<unsigned>(),raw["curve_percent"].get<unsigned>()};if(!sources.insert(next.axes[i].source).second)return false;
        }
      }
      // Raw Web Gamepad calibration is applied by the owned Game Hub provider
      // before its logical input wire. SDL keeps its own device mapping; never
      // reinterpret a browser device identifier as an SDL GUID or host path.
      if(value.contains("raw_gamepads")){
        const auto& raw=value.at("raw_gamepads");if(!raw.is_array()||!raw.empty())return false;
      }
      if(value.contains("sdl_raw_joysticks")){
        const auto& list=value.at("sdl_raw_joysticks");
        if(!list.is_array()||list.size()>4)return false;
#ifdef OMNIPLAY_DEVICE_FREE_ACTOR
        if(!list.empty())return false;
#else
        std::set<std::string> seen_guids;
        for(const auto& entry:list){
          if(!entry.is_object()||entry.size()!=6)return false;
          const auto guid=entry.at("guid_hex").get<std::string>();
          const int ax_cnt=entry.at("axis_count").get<int>(),btn_cnt=entry.at("button_count").get<int>(),hat_cnt=entry.at("hat_count").get<int>();
          if(guid.size()!=32||!std::all_of(guid.begin(),guid.end(),[](unsigned char c){return std::isxdigit(c)!=0;})||ax_cnt<0||ax_cnt>32||btn_cnt<0||btn_cnt>64||hat_cnt<0||hat_cnt>8||(ax_cnt==0&&btn_cnt==0&&hat_cnt==0))return false;
          const auto key=guid+":"+std::to_string(ax_cnt)+":"+std::to_string(btn_cnt);
          if(!seen_guids.insert(key).second)return false;
          const auto& jaxes=entry.at("axes");const auto& jbtns=entry.at("buttons");
          if(!jaxes.is_array()||jaxes.size()!=6||!jbtns.is_array()||jbtns.size()!=16)return false;
          presentation::SdlJoystickCalibration cal;cal.guid_hex=guid;cal.axis_count=ax_cnt;cal.button_count=btn_cnt;
          std::set<int> used_axes;
          for(std::size_t a=0;a<6;++a){
            const auto& ja=jaxes[a];if(!ja.is_object()||ja.size()!=4)return false;
            int src=-1;if(!ja.at("source").is_null()){src=ja.at("source").get<int>();if(src<0||src>=ax_cnt||!used_axes.insert(src).second)return false;}
            const int mn=ja.at("minimum").get<int>(),ct=ja.at("center").get<int>(),mx=ja.at("maximum").get<int>();
            if(mn<-32768||mx>32767||ct-mn<256||mx-ct<256)return false;
            cal.axes[a]={src,static_cast<std::int16_t>(mn),static_cast<std::int16_t>(ct),static_cast<std::int16_t>(mx)};
          }
          for(std::size_t b=0;b<16;++b){
            const auto& jb=jbtns[b];if(!jb.is_object()||jb.size()!=4)return false;
            const auto kind_str=jb.at("kind").get<std::string>();
            const int idx=jb.at("index").get<int>(),hmask=jb.at("hat_mask").get<int>(),thr=jb.at("threshold_percent").get<int>();
            if(thr<1||thr>100||idx<0||hmask<0)return false;
            presentation::RawBindKind kind=presentation::RawBindKind::None;
            if(kind_str=="none"){if(idx!=0||hmask!=0)return false;}
            else if(kind_str=="button"){if(idx>=btn_cnt||hmask!=0)return false;kind=presentation::RawBindKind::Button;}
            else if(kind_str=="axis_positive"){if(idx>=ax_cnt||hmask!=0)return false;kind=presentation::RawBindKind::AxisPositive;}
            else if(kind_str=="axis_negative"){if(idx>=ax_cnt||hmask!=0)return false;kind=presentation::RawBindKind::AxisNegative;}
            else if(kind_str=="hat"){if(idx>=hat_cnt||(hmask!=1&&hmask!=2&&hmask!=4&&hmask!=8))return false;kind=presentation::RawBindKind::Hat;}
            else return false;
            cal.buttons[b]={kind,static_cast<std::uint8_t>(idx),static_cast<std::uint8_t>(hmask),static_cast<std::uint8_t>(thr)};
          }
          next.sdl_raw_joysticks.push_back(std::move(cal));
        }
#endif
      }
      if(value.contains("controller_ports")){
        const auto& ports=value.at("controller_ports");if(!ports.is_array()||ports.size()!=4)return false;
        for(unsigned p=0;p<4;++p){
          const auto& jp=ports[p];if(!jp.is_object()||jp.size()!=3)return false;
          const auto dev=jp.at("device").get<std::string>();
          const int sub=jp.at("subclass").get<int>(),curve=jp.at("pressure_curve_percent").get<int>();
          if(sub<0||sub>16||curve<50||curve>300)return false;
          PortDevice kind=PortDevice::Auto;
          if(dev=="auto")kind=PortDevice::Auto;
          else if(dev=="joypad")kind=PortDevice::Joypad;
          else if(dev=="analog_joypad")kind=PortDevice::AnalogJoypad;
          else if(dev=="mouse")kind=PortDevice::Mouse;
          else if(dev=="lightgun")kind=PortDevice::Lightgun;
          else if(dev=="none"){if(sub!=0)return false;kind=PortDevice::None;}
          else return false;
          next.controller_ports[p]={kind,static_cast<unsigned>(sub),static_cast<unsigned>(curve)};
        }
      }
      const auto keys=[&](const Json& values,auto& destination){for(std::size_t index=0;index<destination.size();++index){const auto& entry=values[index];if(entry.is_null()){destination[index]=SDL_SCANCODE_UNKNOWN;continue;}if(!entry.is_string())return false;const auto name=entry.get<std::string>();if(name.size()>16)return false;const auto code=scan(name);if(code==SDL_SCANCODE_UNKNOWN||!used.insert(code).second)return false;destination[index]=code;}return true;};
      if(!keys(value.at("keyboard"),next.keyboard)||!keys(value.at("analog_keyboard"),next.analog))return false;std::set<unsigned> buttons;
      for(unsigned index=0;index<16;++index){const auto& raw=value.at("button_sources")[index];if(!raw.is_number_integer()||raw<0||raw>15)return false;const auto source=raw.get<unsigned>();if(!buttons.insert(source).second)return false;next.buttons[index]=source;}
      *this=next;return true;
    }catch(...){return false;}
  }
};
}
