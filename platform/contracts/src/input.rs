use crate::{ErrorCode,PlatformError,Result};
use serde::{Serialize,Deserialize};
use std::collections::BTreeSet;

#[derive(Clone,Debug,Serialize)]
pub struct GameInputProfile{pub schema_version:u32,pub keyboard:Vec<Option<String>>,pub analog_keyboard:Vec<Option<String>>,pub button_sources:Vec<u8>,pub haptics_enabled:bool,pub analog_deadzone_percent:u8}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputProfileFields{schema_version:u32,keyboard:Vec<Option<String>>,analog_keyboard:Vec<Option<String>>,button_sources:Vec<u8>,#[serde(default)]haptics_enabled:bool,#[serde(default="default_analog_deadzone_percent")]analog_deadzone_percent:u8}
fn default_analog_deadzone_percent()->u8{15}
// serde_json's ordinary struct decoder also accepts positional arrays. Profiles
// require object keys; keep MapAccess so duplicate-field rejection is preserved.
fn object_fields<'de,D,T>(deserializer:D)->std::result::Result<T,D::Error>where D:serde::Deserializer<'de>,T:Deserialize<'de>{
    struct ObjectVisitor<T>(std::marker::PhantomData<T>);
    impl<'de,T:Deserialize<'de>>serde::de::Visitor<'de>for ObjectVisitor<T>{
        type Value=T;
        fn expecting(&self,formatter:&mut std::fmt::Formatter)->std::fmt::Result{formatter.write_str("a keyed input profile object")}
        fn visit_map<M:serde::de::MapAccess<'de>>(self,map:M)->std::result::Result<T,M::Error>{T::deserialize(serde::de::value::MapAccessDeserializer::new(map))}
    }
    deserializer.deserialize_map(ObjectVisitor::<T>(std::marker::PhantomData))
}
impl<'de>Deserialize<'de>for GameInputProfile{
    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->std::result::Result<Self,D::Error>{let InputProfileFields{schema_version,keyboard,analog_keyboard,button_sources,haptics_enabled,analog_deadzone_percent}=object_fields(deserializer)?;Ok(Self{schema_version,keyboard,analog_keyboard,button_sources,haptics_enabled,analog_deadzone_percent})}
}
impl Default for GameInputProfile{
    fn default()->Self{Self{schema_version:1,keyboard:["KeyZ","KeyA","ShiftRight","Enter","ArrowUp","ArrowDown","ArrowLeft","ArrowRight","KeyX","KeyS","KeyQ","KeyW","KeyE","KeyR","KeyD","KeyF"].into_iter().map(|v|Some(v.into())).collect(),analog_keyboard:["KeyI","KeyK","KeyJ","KeyL"].into_iter().map(|v|Some(v.into())).collect(),button_sources:(0..16u8).collect(),haptics_enabled:false,analog_deadzone_percent:default_analog_deadzone_percent()}}
}
pub fn allowed_game_key(value:&str)->bool{
    matches!(value,"ArrowUp"|"ArrowDown"|"ArrowLeft"|"ArrowRight"|"Space"|"Enter"|"Tab"|"Backspace"|"ShiftLeft"|"ShiftRight")||value.strip_prefix("Key").is_some_and(|v|v.len()==1&&v.as_bytes()[0].is_ascii_uppercase())||value.strip_prefix("Digit").is_some_and(|v|v.len()==1&&v.as_bytes()[0].is_ascii_digit())
}
impl GameInputProfile{
    pub fn validate(&self)->Result<()>{
        let invalid=||PlatformError::new(ErrorCode::InvalidRequest,"Game input mappings must use distinct supported keys and a complete button permutation");
        if self.schema_version!=1||self.keyboard.len()!=16||self.analog_keyboard.len()!=4||self.button_sources.len()!=16||self.analog_deadzone_percent>50{return Err(invalid());}
        let mut keys=BTreeSet::new();for value in self.keyboard.iter().chain(self.analog_keyboard.iter()).flatten(){if !allowed_game_key(value)||!keys.insert(value){return Err(invalid());}}
        let buttons:BTreeSet<u8>=self.button_sources.iter().copied().collect();if buttons.len()!=16||buttons.iter().any(|v|*v>15){return Err(invalid());}Ok(())
    }
}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TouchVisibility{Auto,Always,Hidden}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TouchLayout{Auto,Buttons,Sticks}
#[derive(Clone,Debug,Serialize)]
pub struct GameTouchProfile{pub schema_version:u32,pub visibility:TouchVisibility,pub layout:TouchLayout,pub left_handed:bool,pub size_percent:u8,pub opacity_percent:u8}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TouchProfileFields{schema_version:u32,visibility:TouchVisibility,layout:TouchLayout,left_handed:bool,size_percent:u8,opacity_percent:u8}
impl<'de>Deserialize<'de>for GameTouchProfile{
    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->std::result::Result<Self,D::Error>{let TouchProfileFields{schema_version,visibility,layout,left_handed,size_percent,opacity_percent}=object_fields(deserializer)?;Ok(Self{schema_version,visibility,layout,left_handed,size_percent,opacity_percent})}
}
impl Default for GameTouchProfile{fn default()->Self{Self{schema_version:1,visibility:TouchVisibility::Auto,layout:TouchLayout::Auto,left_handed:false,size_percent:100,opacity_percent:100}}}
impl GameTouchProfile{
    pub fn validate(&self)->Result<()>{if self.schema_version!=1||!(80..=140).contains(&self.size_percent)||!(40..=100).contains(&self.opacity_percent){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Touch controls need a supported layout, 80–140% size and 40–100% opacity"));}Ok(())}
}
