use crate::{ContentHash,ErrorCode,GameSystem,PlatformError,Result};
use serde::{Deserialize,Serialize};
use sha2::{Digest,Sha256};

#[derive(Clone,Copy,Debug,PartialEq,Eq,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum ContentFormat {RawRom,Chd,Cue,M3u,Iso,Pbp,Cso,N64Z64,N64V64,N64N64}
impl ContentFormat {
    pub fn from_extension(value:&str)->Option<Self>{match value.trim_start_matches('.').to_ascii_lowercase().as_str(){"chd"=>Some(Self::Chd),"cue"=>Some(Self::Cue),"m3u"|"m3u8"=>Some(Self::M3u),"iso"=>Some(Self::Iso),"pbp"=>Some(Self::Pbp),"cso"=>Some(Self::Cso),"z64"=>Some(Self::N64Z64),"v64"=>Some(Self::N64V64),"n64"=>Some(Self::N64N64),_=>None}}
    pub fn extension(self,system:GameSystem)->Result<&'static str>{
        let valid=match self{Self::RawRom=>!matches!(system,GameSystem::Pc|GameSystem::Web),Self::Cue|Self::M3u=>system==GameSystem::Ps1,Self::Chd|Self::Iso|Self::Pbp=>matches!(system,GameSystem::Ps1|GameSystem::Psp),Self::Cso=>system==GameSystem::Psp,Self::N64Z64|Self::N64V64|Self::N64N64=>system==GameSystem::N64};
        if !valid{return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Content format does not match the selected system"));}
        Ok(match self{Self::Chd=>"chd",Self::Cue=>"cue",Self::M3u=>"m3u",Self::Iso=>"iso",Self::Pbp=>"pbp",Self::Cso=>"cso",Self::N64Z64=>"z64",Self::N64V64=>"v64",Self::N64N64=>"n64",Self::RawRom=>match system{GameSystem::Nes=>"nes",GameSystem::Gb=>"gb",GameSystem::Gbc=>"gbc",GameSystem::Snes=>"sfc",GameSystem::Gba=>"gba",GameSystem::MegaDrive=>"gen",GameSystem::Ps1=>"bin",GameSystem::N64=>"z64",GameSystem::Nds=>"nds",GameSystem::Psp=>"iso",_=>return Err(PlatformError::new(ErrorCode::UnsupportedCapability,"Content format is unsupported"))}})
    }
}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MediaAsset {pub name:String,pub content_hash:ContentHash,pub byte_length:u64}
#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GameMedia {pub schema_version:u32,pub format:ContentFormat,pub entry_name:String,pub assets:Vec<MediaAsset>,#[serde(default)]pub disc_entries:Vec<String>}
impl GameMedia {
    pub fn identity_hash(&self)->Result<ContentHash>{
        if self.assets.len()==1&&!matches!(self.format,ContentFormat::Cue|ContentFormat::M3u){return Ok(self.assets[0].content_hash.clone());}
        let bytes=serde_json::to_vec(self).map_err(|_|PlatformError::new(ErrorCode::InvalidRequest,"Media identity could not be encoded"))?;
        let mut hash=Sha256::new();hash.update(b"omniplay.media.v1\0");hash.update(bytes);Ok(ContentHash(format!("{:x}",hash.finalize())))
    }
    pub fn validate(&self,system:GameSystem,total:u64)->Result<()> {
        let extension=self.format.extension(system)?;
        if self.entry_name!=format!("game.{extension}"){return Err(invalid());}
        if self.schema_version!=1||self.assets.is_empty()||self.assets.len()>128||self.disc_entries.len()>8{return Err(invalid());}
        let mut bytes=0u64;let mut previous=None;let mut names=std::collections::BTreeSet::new();
        for asset in &self.assets{
            if !valid_media_name(&asset.name)||asset.byte_length==0||asset.byte_length>crate::MAX_ROM_BYTES||previous.is_some_and(|name:&str|name>=asset.name.as_str()){return Err(invalid());}
            ContentHash::parse(asset.content_hash.0.clone())?;
            bytes=bytes.checked_add(asset.byte_length).ok_or_else(invalid)?;previous=Some(asset.name.as_str());names.insert(asset.name.as_str());
        }
        if bytes!=total||bytes>crate::MAX_ROM_BYTES||!names.contains(self.entry_name.as_str())||self.disc_entries.iter().any(|v|!names.contains(v.as_str())){return Err(invalid());}
        if matches!(self.format,ContentFormat::Cue|ContentFormat::M3u)&&self.assets.len()<2{return Err(invalid());}
        if self.format==ContentFormat::M3u&&self.disc_entries.is_empty(){return Err(invalid());}
        Ok(())
    }
    pub fn entry(&self)->Result<&MediaAsset>{self.assets.iter().find(|a|a.name==self.entry_name).ok_or_else(invalid)}
}
pub fn valid_media_name(value:&str)->bool {
    if value.is_empty()||value.len()>64||!value.bytes().all(|v|v.is_ascii_lowercase()||v.is_ascii_digit()||[b'-',b'.'].contains(&v)){return false;}
    let Some((stem,extension))=value.rsplit_once('.')else{return false;};
    let stem_valid=stem=="game"||stem.strip_prefix("track-").is_some_and(|v|v.len()==4&&v.bytes().all(|c|c.is_ascii_digit()))||stem.strip_prefix("disc-").is_some_and(|v|v.len()==2&&v.bytes().all(|c|c.is_ascii_digit()));
    stem_valid&&["nes","gb","gbc","sfc","gba","gen","bin","z64","v64","n64","nds","iso","chd","cue","m3u","pbp","cso","wav"].contains(&extension)
}
fn invalid()->PlatformError{PlatformError::new(ErrorCode::InvalidRequest,"Media manifest violates the bounded content policy")}

#[derive(Clone,Copy,Debug,Serialize,Deserialize)]
#[serde(rename_all="snake_case")]
pub enum CueEncoding {Auto,Utf8,Windows1252,ShiftJis,Gbk,Utf16Le,Utf16Be}

#[derive(Clone,Debug,Serialize,Deserialize)]
pub struct ImportProgress {pub phase:String,pub copied_bytes:u64,pub total_bytes:u64,pub asset_index:u16,pub asset_count:u16}
impl Default for ImportProgress {fn default()->Self{Self{phase:"preparing".into(),copied_bytes:0,total_bytes:0,asset_index:0,asset_count:0}}}

#[derive(Clone,Debug,Serialize,Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DiscStatus {pub supported:bool,pub count:u32,pub index:u32,pub ejected:bool}
impl DiscStatus {pub fn validate(&self)->Result<()>{if self.count>8||(self.supported&&(self.count==0||self.index>=self.count))||(!self.supported&&(self.count!=0||self.index!=0||self.ejected)){return Err(invalid());}Ok(())}}
