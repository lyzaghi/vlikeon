#pragma once
#include "protocol.hpp"
#include <SDL3/SDL.h>
#include <array>
#include <algorithm>
#include <cmath>
#include <cstdint>
#include <set>
#include <string>

namespace omni {
struct InputProfile {
  bool haptics=false;
  unsigned analog_deadzone_percent=15;
  std::array<SDL_Scancode,16> keyboard{SDL_SCANCODE_Z,SDL_SCANCODE_A,SDL_SCANCODE_RSHIFT,SDL_SCANCODE_RETURN,SDL_SCANCODE_UP,SDL_SCANCODE_DOWN,SDL_SCANCODE_LEFT,SDL_SCANCODE_RIGHT,SDL_SCANCODE_X,SDL_SCANCODE_S,SDL_SCANCODE_Q,SDL_SCANCODE_W,SDL_SCANCODE_E,SDL_SCANCODE_R,SDL_SCANCODE_D,SDL_SCANCODE_F};
  std::array<SDL_Scancode,4> analog{SDL_SCANCODE_I,SDL_SCANCODE_K,SDL_SCANCODE_J,SDL_SCANCODE_L};
  std::array<unsigned,16> buttons{0,1,2,3,4,5,6,7,8,9,10,11,12,13,14,15};
  struct Axis{unsigned source;bool inverted=false;unsigned sensitivity=100,curve=100;};
  std::array<Axis,4> axes{{{0},{1},{2},{3}}};
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
      if(!value.is_object()||value.size()<4||value.size()>8)return false;
      for(const auto& item:value.items())if(item.key()!="schema_version"&&item.key()!="keyboard"&&item.key()!="analog_keyboard"&&item.key()!="button_sources"&&item.key()!="haptics_enabled"&&item.key()!="analog_deadzone_percent"&&item.key()!="axis_tuning"&&item.key()!="raw_gamepads")return false;
      if(!value.at("schema_version").is_number_integer()||value.at("schema_version")!=1||!value.at("keyboard").is_array()||value.at("keyboard").size()!=16||!value.at("analog_keyboard").is_array()||value.at("analog_keyboard").size()!=4||!value.at("button_sources").is_array()||value.at("button_sources").size()!=16)return false;
      InputProfile next;std::set<SDL_Scancode> used;
      if(value.contains("haptics_enabled")){if(!value.at("haptics_enabled").is_boolean())return false;next.haptics=value.at("haptics_enabled").get<bool>();}
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
      const auto keys=[&](const Json& values,auto& destination){for(std::size_t index=0;index<destination.size();++index){const auto& entry=values[index];if(entry.is_null()){destination[index]=SDL_SCANCODE_UNKNOWN;continue;}if(!entry.is_string())return false;const auto name=entry.get<std::string>();if(name.size()>16)return false;const auto code=scan(name);if(code==SDL_SCANCODE_UNKNOWN||!used.insert(code).second)return false;destination[index]=code;}return true;};
      if(!keys(value.at("keyboard"),next.keyboard)||!keys(value.at("analog_keyboard"),next.analog))return false;std::set<unsigned> buttons;
      for(unsigned index=0;index<16;++index){const auto& raw=value.at("button_sources")[index];if(!raw.is_number_integer()||raw<0||raw>15)return false;const auto source=raw.get<unsigned>();if(!buttons.insert(source).second)return false;next.buttons[index]=source;}
      *this=next;return true;
    }catch(...){return false;}
  }
};
}
