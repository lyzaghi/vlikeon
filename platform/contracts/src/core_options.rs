use crate::{ErrorCode,PlatformError,Result};
use serde::{Deserialize,Serialize};
use std::collections::{BTreeMap,BTreeSet};

pub const MAX_CORE_OPTION_PROFILE_BYTES:usize=16384;
pub const MAX_CORE_OPTION_POLICY_BYTES:usize=65536;
fn invalid()->PlatformError{PlatformError::new(ErrorCode::InvalidRequest,"Core options require a bounded declared choice for every key")}
pub fn core_option_identifier(value:&str)->bool{!value.is_empty()&&value.len()<=128&&value.bytes().all(|c|c.is_ascii_alphanumeric()||matches!(c,b'_'|b'-'))&&!matches!(value,"__proto__"|"constructor"|"prototype")}
fn exposable(value:&str)->bool{let value=value.to_ascii_lowercase();!["jit","dynarec","cpucore","cpu_core","fast_memory","backend","renderer","render_mode","rdp-plugin","rsp-plugin","thread","wlan","network","server","sdcard","firmware","bios_path","sysfile","cheat","directory","_path"].iter().any(|part|value.contains(part))}
fn option_value(value:&str)->bool{!value.is_empty()&&value.len()<=128&&value.bytes().all(|c|(32..=126).contains(&c))}
fn label(value:&str,max:usize)->bool{!value.is_empty()&&value.len()<=max&&!value.chars().any(char::is_control)}
fn object_fields<'de,D,T>(deserializer:D)->std::result::Result<T,D::Error>where D:serde::Deserializer<'de>,T:Deserialize<'de>{
    struct ObjectVisitor<T>(std::marker::PhantomData<T>);
    impl<'de,T:Deserialize<'de>>serde::de::Visitor<'de>for ObjectVisitor<T>{
        type Value=T;
        fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.write_str("a keyed core option object")}
        fn visit_map<M:serde::de::MapAccess<'de>>(self,map:M)->std::result::Result<T,M::Error>{T::deserialize(serde::de::value::MapAccessDeserializer::new(map))}
    }
    deserializer.deserialize_map(ObjectVisitor::<T>(std::marker::PhantomData))
}
fn unique_values<'de,D:serde::Deserializer<'de>>(deserializer:D)->std::result::Result<BTreeMap<String,String>,D::Error>{
    struct Values;
    impl<'de>serde::de::Visitor<'de>for Values{
        type Value=BTreeMap<String,String>;
        fn expecting(&self,f:&mut std::fmt::Formatter)->std::fmt::Result{f.write_str("at most 64 distinct core option keys")}
        fn visit_map<M:serde::de::MapAccess<'de>>(self,mut map:M)->std::result::Result<Self::Value,M::Error>{
            let mut result=BTreeMap::new();while let Some((key,value))=map.next_entry::<String,String>()?{
                if result.len()>=64||!core_option_identifier(&key)||!option_value(&value)||result.insert(key,value).is_some(){return Err(serde::de::Error::custom("Invalid or duplicate core option key/value"));}
            }Ok(result)
        }
    }deserializer.deserialize_map(Values)
}

#[derive(Clone,Debug,Serialize)]
pub struct GameCoreOptions{pub schema_version:u32,pub values:BTreeMap<String,String>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct ProfileFields{schema_version:u32,#[serde(deserialize_with="unique_values")]values:BTreeMap<String,String>}
impl<'de>Deserialize<'de>for GameCoreOptions{fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error>{let ProfileFields{schema_version,values}=object_fields(d)?;Ok(Self{schema_version,values})}}
impl Default for GameCoreOptions{fn default()->Self{Self{schema_version:1,values:BTreeMap::new()}}}
impl GameCoreOptions{
    pub fn validate(&self)->Result<()>{if self.schema_version!=1||self.values.len()>64||self.values.iter().any(|(k,v)|!core_option_identifier(k)||!option_value(v))||serde_json::to_vec(self).map_err(|_|invalid())?.len()>MAX_CORE_OPTION_PROFILE_BYTES{return Err(invalid());}Ok(())}
}

#[derive(Clone,Debug,Serialize)]
pub struct CoreOptionChoice{pub value:String,pub label_vi:String,pub label_en:String}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct ChoiceFields{value:String,label_vi:String,label_en:String}
impl<'de>Deserialize<'de>for CoreOptionChoice{fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error>{let ChoiceFields{value,label_vi,label_en}=object_fields(d)?;Ok(Self{value,label_vi,label_en})}}
#[derive(Clone,Debug,Serialize)]
pub struct CoreOptionDefinition{pub key:String,pub label_vi:String,pub label_en:String,pub description_vi:String,pub description_en:String,pub default_value:String,pub choices:Vec<CoreOptionChoice>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct DefinitionFields{key:String,label_vi:String,label_en:String,description_vi:String,description_en:String,default_value:String,choices:Vec<CoreOptionChoice>}
impl<'de>Deserialize<'de>for CoreOptionDefinition{fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error>{let DefinitionFields{key,label_vi,label_en,description_vi,description_en,default_value,choices}=object_fields(d)?;Ok(Self{key,label_vi,label_en,description_vi,description_en,default_value,choices})}}
#[derive(Clone,Debug,Serialize)]
pub struct CoreOptionPolicy{pub schema_version:u32,pub definitions:Vec<CoreOptionDefinition>}
#[derive(Deserialize)]#[serde(deny_unknown_fields)]
struct PolicyFields{schema_version:u32,definitions:Vec<CoreOptionDefinition>}
impl<'de>Deserialize<'de>for CoreOptionPolicy{fn deserialize<D:serde::Deserializer<'de>>(d:D)->std::result::Result<Self,D::Error>{let PolicyFields{schema_version,definitions}=object_fields(d)?;Ok(Self{schema_version,definitions})}}
impl Default for CoreOptionPolicy{fn default()->Self{Self{schema_version:1,definitions:Vec::new()}}}
impl CoreOptionPolicy{
    pub fn validate(&self)->Result<()>{
        if self.schema_version!=1||self.definitions.len()>32||serde_json::to_vec(self).map_err(|_|invalid())?.len()>MAX_CORE_OPTION_POLICY_BYTES{return Err(invalid());}
        let mut keys=BTreeSet::new();let mut count=0;
        for definition in &self.definitions{
            if !core_option_identifier(&definition.key)||!exposable(&definition.key)||!keys.insert(&definition.key)||!label(&definition.label_vi,160)||!label(&definition.label_en,160)||!label(&definition.description_vi,512)||!label(&definition.description_en,512)||definition.choices.is_empty()||definition.choices.len()>32{return Err(invalid());}
            count+=definition.choices.len();if count>256{return Err(invalid());}let mut values=BTreeSet::new();
            for choice in &definition.choices{if !option_value(&choice.value)||!values.insert(choice.value.clone())||!label(&choice.label_vi,160)||!label(&choice.label_en,160){return Err(invalid());}}
            if !values.contains(&definition.default_value){return Err(invalid());}
        }Ok(())
    }
    pub fn resolve(&self,profile:&GameCoreOptions)->Result<BTreeMap<String,String>>{
        self.validate()?;profile.validate()?;let mut result:BTreeMap<_,_>=self.definitions.iter().map(|d|(d.key.clone(),d.default_value.clone())).collect();
        for(key,value)in &profile.values{
            let declared=self.definitions.iter().find(|d|d.key==*key).ok_or_else(||PlatformError::new(ErrorCode::ContentMismatch,"Saved core options no longer belong to this installed runtime").recover("Review or reset this runtime's game options"))?;
            if !declared.choices.iter().any(|choice|choice.value==*value){return Err(PlatformError::new(ErrorCode::ContentMismatch,"A saved core option is unavailable in this installed runtime").recover("Review or reset this runtime's game options"));}
            result.insert(key.clone(),value.clone());
        }Ok(result)
    }
}
