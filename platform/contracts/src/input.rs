use crate::{ErrorCode,PlatformError,Result};
use serde::{Serialize,Deserialize};
use std::collections::BTreeSet;

#[derive(Clone,Debug,Serialize)]
pub struct GameInputProfile{pub schema_version:u32,pub keyboard:Vec<Option<String>>,pub analog_keyboard:Vec<Option<String>>,pub button_sources:Vec<u8>,pub haptics_enabled:bool,pub analog_deadzone_percent:u8,pub axis_tuning:Vec<AxisTuning>,pub raw_gamepads:Vec<RawGamepadProfile>}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct InputProfileFields{schema_version:u32,keyboard:Vec<Option<String>>,analog_keyboard:Vec<Option<String>>,button_sources:Vec<u8>,#[serde(default)]haptics_enabled:bool,#[serde(default="default_analog_deadzone_percent")]analog_deadzone_percent:u8,#[serde(default="default_axis_tuning")]axis_tuning:Vec<AxisTuning>,#[serde(default)]raw_gamepads:Vec<RawGamepadProfile>}
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
    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->std::result::Result<Self,D::Error>{let InputProfileFields{schema_version,keyboard,analog_keyboard,button_sources,haptics_enabled,analog_deadzone_percent,axis_tuning,raw_gamepads}=object_fields(deserializer)?;Ok(Self{schema_version,keyboard,analog_keyboard,button_sources,haptics_enabled,analog_deadzone_percent,axis_tuning,raw_gamepads})}
}
impl Default for GameInputProfile{
    fn default()->Self{Self{schema_version:1,keyboard:["KeyZ","KeyA","ShiftRight","Enter","ArrowUp","ArrowDown","ArrowLeft","ArrowRight","KeyX","KeyS","KeyQ","KeyW","KeyE","KeyR","KeyD","KeyF"].into_iter().map(|v|Some(v.into())).collect(),analog_keyboard:["KeyI","KeyK","KeyJ","KeyL"].into_iter().map(|v|Some(v.into())).collect(),button_sources:(0..16u8).collect(),haptics_enabled:false,analog_deadzone_percent:default_analog_deadzone_percent(),axis_tuning:default_axis_tuning(),raw_gamepads:Vec::new()}}
}
pub fn allowed_game_key(value:&str)->bool{
    matches!(value,"ArrowUp"|"ArrowDown"|"ArrowLeft"|"ArrowRight"|"Space"|"Enter"|"Tab"|"Backspace"|"ShiftLeft"|"ShiftRight")||value.strip_prefix("Key").is_some_and(|v|v.len()==1&&v.as_bytes()[0].is_ascii_uppercase())||value.strip_prefix("Digit").is_some_and(|v|v.len()==1&&v.as_bytes()[0].is_ascii_digit())
}
impl GameInputProfile{
    pub fn validate(&self)->Result<()>{
        let invalid=||PlatformError::new(ErrorCode::InvalidRequest,"Game input mappings must use distinct supported keys and a complete button permutation");
        if self.schema_version!=1||self.keyboard.len()!=16||self.analog_keyboard.len()!=4||self.button_sources.len()!=16||self.analog_deadzone_percent>50{return Err(invalid());}
        let mut keys=BTreeSet::new();for value in self.keyboard.iter().chain(self.analog_keyboard.iter()).flatten(){if !allowed_game_key(value)||!keys.insert(value){return Err(invalid());}}
        let buttons:BTreeSet<u8>=self.button_sources.iter().copied().collect();if buttons.len()!=16||buttons.iter().any(|v|*v>15){return Err(invalid());}
        if self.axis_tuning.len()!=4||self.axis_tuning.iter().any(|axis|axis.source>3||!(50..=200).contains(&axis.sensitivity_percent)||!(50..=300).contains(&axis.curve_percent))||self.axis_tuning.iter().map(|a|a.source).collect::<BTreeSet<_>>().len()!=4||self.raw_gamepads.len()>4{return Err(invalid());}
        let mut devices=BTreeSet::new();for device in &self.raw_gamepads{device.validate()?;if !devices.insert((&device.device_id,device.axis_count,device.button_count)){return Err(invalid());}}Ok(())
    }
}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TouchVisibility{Auto,Always,Hidden}
#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum TouchLayout{Auto,Buttons,Sticks}
#[derive(Clone,Debug,Serialize)]
pub struct GameTouchProfile{pub schema_version:u32,pub visibility:TouchVisibility,pub layout:TouchLayout,pub left_handed:bool,pub size_percent:u8,pub opacity_percent:u8,pub custom_layout:Option<CustomTouchLayout>}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TouchProfileFields{schema_version:u32,visibility:TouchVisibility,layout:TouchLayout,left_handed:bool,size_percent:u8,opacity_percent:u8,#[serde(default)]custom_layout:Option<CustomTouchLayout>}
impl<'de>Deserialize<'de>for GameTouchProfile{
    fn deserialize<D:serde::Deserializer<'de>>(deserializer:D)->std::result::Result<Self,D::Error>{let TouchProfileFields{schema_version,visibility,layout,left_handed,size_percent,opacity_percent,custom_layout}=object_fields(deserializer)?;Ok(Self{schema_version,visibility,layout,left_handed,size_percent,opacity_percent,custom_layout})}
}
impl Default for GameTouchProfile{fn default()->Self{Self{schema_version:1,visibility:TouchVisibility::Auto,layout:TouchLayout::Auto,left_handed:false,size_percent:100,opacity_percent:100,custom_layout:None}}}
impl GameTouchProfile{
    pub fn validate(&self)->Result<()>{if self.schema_version!=1||!(80..=140).contains(&self.size_percent)||!(40..=100).contains(&self.opacity_percent){return Err(PlatformError::new(ErrorCode::InvalidRequest,"Touch controls need a supported layout, 80–140% size and 40–100% opacity"));}if let Some(layout)=&self.custom_layout{layout.validate()?;}Ok(())}
}

// Optional fields preserve legacy profiles. Device identifiers stay in this
// local per-game preference; they are not controller-event history or telemetry.
// A nested Option is nullable but still a required key. An explicit decoder
// keeps Serde from treating an omitted axis source as implicit None.
fn required_input_value<'de,D,T>(decoder:D)->std::result::Result<T,D::Error>where D:serde::Deserializer<'de>,T:Deserialize<'de>{T::deserialize(decoder)}
macro_rules! keyed_input_type {
    ($name:ident,$fields:ident,{$($field:ident:$kind:ty),+ $(,)?}) => {
        #[derive(Clone,Debug,Serialize)]pub struct $name{$(pub $field:$kind),+}
        #[derive(Deserialize)]#[serde(deny_unknown_fields)]struct $fields{$(#[serde(deserialize_with="required_input_value")]$field:$kind),+}
        impl<'de>Deserialize<'de>for $name{fn deserialize<D:serde::Deserializer<'de>>(decoder:D)->std::result::Result<Self,D::Error>{let $fields{$($field),+}=object_fields(decoder)?;Ok(Self{$($field),+})}}
    }
}
keyed_input_type!(AxisTuning,AxisTuningFields,{source:u8,inverted:bool,sensitivity_percent:u16,curve_percent:u16});
pub fn default_axis_tuning()->Vec<AxisTuning>{(0..4).map(|source|AxisTuning{source,inverted:false,sensitivity_percent:100,curve_percent:100}).collect()}
keyed_input_type!(RawAxisCalibration,RawAxisFields,{source:Option<u8>,minimum:i16,center:i16,maximum:i16});
#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]#[serde(rename_all="snake_case")]
pub enum RawButtonKind{None,Button,AxisPositive,AxisNegative}
keyed_input_type!(RawButtonBinding,RawButtonFields,{kind:RawButtonKind,index:u8,threshold_percent:u8});
keyed_input_type!(RawGamepadProfile,RawGamepadFields,{device_id:String,axis_count:u8,button_count:u8,axes:Vec<RawAxisCalibration>,buttons:Vec<RawButtonBinding>});
impl RawGamepadProfile{
    pub fn validate(&self)->Result<()>{
        let invalid=||PlatformError::new(ErrorCode::InvalidRequest,"Raw controller calibration is outside its device bounds");
        if self.device_id.is_empty()||self.device_id.len()>512||self.device_id.chars().any(char::is_control)||self.axis_count>32||self.button_count>64||(self.axis_count==0&&self.button_count==0)||self.axes.len()!=4||self.buttons.len()!=16{return Err(invalid());}
        let mut axes=BTreeSet::new();for axis in &self.axes{if axis.minimum< -1000||axis.maximum>1000||i32::from(axis.center)-i32::from(axis.minimum)<50||i32::from(axis.maximum)-i32::from(axis.center)<50||axis.source.is_some_and(|source|source>=self.axis_count||!axes.insert(source)){return Err(invalid());}}
        for binding in &self.buttons{
            let invalid_source=match binding.kind{RawButtonKind::None=>binding.index!=0,RawButtonKind::Button=>binding.index>=self.button_count,RawButtonKind::AxisPositive|RawButtonKind::AxisNegative=>binding.index>=self.axis_count};
            if !(1..=100).contains(&binding.threshold_percent)||invalid_source{return Err(invalid());}
        }
        Ok(())
    }
}
keyed_input_type!(TouchPlacement,TouchPlacementFields,{id:u8,x:u16,y:u16,size_percent:u16,visible:bool});
keyed_input_type!(CustomTouchLayout,CustomTouchFields,{height:u16,controls:Vec<TouchPlacement>});
impl CustomTouchLayout{pub fn validate(&self)->Result<()>{
    if !(400..=1200).contains(&self.height)||self.controls.len()!=18||self.controls.iter().any(|v|v.id>17||v.x>1000||v.y>1000||!(50..=200).contains(&v.size_percent))||self.controls.iter().map(|v|v.id).collect::<BTreeSet<_>>().len()!=18{return Err(PlatformError::new(ErrorCode::InvalidRequest,"Custom touch layout needs 18 distinct bounded controls"));}Ok(())
}}
