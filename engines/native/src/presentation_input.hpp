#pragma once
#include "input_profile.hpp"
#include "client.hpp"
#include <bit>
#include <libretro.h>

namespace omni {
inline bool presentation_key(const presentation::Snapshot& value,SDL_Scancode key){const auto code=static_cast<unsigned>(key);return value.fresh&&value.focused&&code>0&&code<512&&(value.input[code/8]&(1u<<(code%8)));}
inline bool presentation_pad(const presentation::Snapshot& value,unsigned port){return value.fresh&&value.focused&&port<4&&value.input[64+port*20]!=0;}
inline std::int16_t presentation_axis(const presentation::Snapshot& value,unsigned port,unsigned axis){if(!presentation_pad(value,port)||axis>=6)return 0;return std::bit_cast<std::int16_t>(presentation::u16(value.input,64+port*20+8+axis*2));}
inline std::array<std::int16_t,2> presentation_stick(const presentation::Snapshot& value,const InputProfile& profile,unsigned port,unsigned index){if(!presentation_pad(value,port)||index>1)return{0,0};return profile.stick_axes(presentation_axis(value,port,index?2:0),presentation_axis(value,port,index?3:1));}
inline std::int16_t presentation_input(const presentation::Snapshot& value,const InputProfile& profile,bool analog_controls,unsigned port,unsigned device,unsigned index,unsigned id){
  if(!value.fresh||!value.focused||port>=4)return 0;device&=RETRO_DEVICE_MASK;
  if(device==RETRO_DEVICE_POINTER){if(port||index)return 0;if(id==RETRO_DEVICE_ID_POINTER_X)return std::bit_cast<std::int16_t>(presentation::u16(value.input,148));if(id==RETRO_DEVICE_ID_POINTER_Y)return std::bit_cast<std::int16_t>(presentation::u16(value.input,150));if(id==RETRO_DEVICE_ID_POINTER_PRESSED||id==RETRO_DEVICE_ID_POINTER_COUNT)return value.input[146]?1:0;
#ifdef RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN
    if(id==RETRO_DEVICE_ID_POINTER_IS_OFFSCREEN)return value.input[145]?0:1;
#endif
    return 0;
  }
  if(device==RETRO_DEVICE_JOYPAD&&index)return 0;
  if(device==RETRO_DEVICE_JOYPAD&&id==RETRO_DEVICE_ID_JOYPAD_MASK){std::uint16_t mask=0;for(unsigned bit=0;bit<16;bit++)if(presentation_input(value,profile,analog_controls,port,RETRO_DEVICE_JOYPAD,0,bit))mask|=std::uint16_t(1u<<bit);return std::bit_cast<std::int16_t>(mask);}
  if(device==RETRO_DEVICE_ANALOG){if(index>1||id>1)return 0;if(!port&&!index&&analog_controls){const bool negative=presentation_key(value,profile.analog[id?0:2]),positive=presentation_key(value,profile.analog[id?1:3]);if(negative||positive)return static_cast<std::int16_t>((int(positive)-int(negative))*32767);}return presentation_stick(value,profile,port,index)[id];}
  if(device!=RETRO_DEVICE_JOYPAD||id>=16)return 0;const auto source=profile.buttons[id];if(source>=16)return 0;
  constexpr std::array<SDL_GamepadButton,16> buttons{SDL_GAMEPAD_BUTTON_SOUTH,SDL_GAMEPAD_BUTTON_WEST,SDL_GAMEPAD_BUTTON_BACK,SDL_GAMEPAD_BUTTON_START,SDL_GAMEPAD_BUTTON_DPAD_UP,SDL_GAMEPAD_BUTTON_DPAD_DOWN,SDL_GAMEPAD_BUTTON_DPAD_LEFT,SDL_GAMEPAD_BUTTON_DPAD_RIGHT,SDL_GAMEPAD_BUTTON_EAST,SDL_GAMEPAD_BUTTON_NORTH,SDL_GAMEPAD_BUTTON_LEFT_SHOULDER,SDL_GAMEPAD_BUTTON_RIGHT_SHOULDER,SDL_GAMEPAD_BUTTON_INVALID,SDL_GAMEPAD_BUTTON_INVALID,SDL_GAMEPAD_BUTTON_LEFT_STICK,SDL_GAMEPAD_BUTTON_RIGHT_STICK};
  if(presentation_pad(value,port)){const auto button=buttons[source];if(button!=SDL_GAMEPAD_BUTTON_INVALID&&(presentation::u32(value.input,64+port*20+4)&(1u<<static_cast<unsigned>(button))))return 1;if(source==12&&presentation_axis(value,port,4)>16000)return 1;if(source==13&&presentation_axis(value,port,5)>16000)return 1;if(!analog_controls){const auto stick=presentation_stick(value,profile,port,0);if((source==4&&stick[1]<-13107)||(source==5&&stick[1]>13107)||(source==6&&stick[0]<-13107)||(source==7&&stick[0]>13107))return 1;}}
  return port==0&&presentation_key(value,profile.keyboard[id])?1:0;
}
}
