use crate::{Database,library::Library,now_ms,util::{PendingFile,hash_file,storage_error}};
use omniplay_contracts::*;
use serde::Serialize;
use sha2::{Digest,Sha256};
use std::{collections::{BTreeMap,BTreeSet,HashMap},fs::{File,OpenOptions},io::{Read,Write},path::{Path,PathBuf},sync::{Arc,atomic::{AtomicBool,Ordering}}};
use uuid::Uuid;

const MAX_TEXT:u64=128*1024;
const MAX_ASSETS:usize=128;
fn invalid(message:&str)->PlatformError{PlatformError::new(ErrorCode::InvalidRequest,message)}
fn cancelled()->PlatformError{PlatformError::new(ErrorCode::Cancelled,"Disc import was cancelled")}

/// Native picker adapters implement this scoped source. It is never a renderer
/// path/URL interface; open must stay within the selected native folder.
pub trait DiscProvider:Send+Sync{
    fn open(&self,relative:&Path)->Result<ProvidedDiscFile>;
    /// Signal native cancellation without waiting for a provider operation.
    fn cancel(&self){}
}
pub struct ProvidedDiscFile{pub identity:PathBuf,pub byte_length:Option<u64>,pub reader:Box<dyn Read+Send>}
enum ScopeSource{Directory(PathBuf),Provider(Arc<dyn DiscProvider>)}
pub struct DiscScope {source:ScopeSource,title:String,entries:BTreeSet<String>}
impl DiscScope {
    pub fn selected(directory:&Path)->Result<Self>{
        let root=directory.canonicalize().map_err(storage_error)?;
        if !root.is_dir(){return Err(invalid("Choose a disc content folder"));}
        let mut entries=BTreeSet::new();
        for entry in std::fs::read_dir(&root).map_err(storage_error)?.take(2048){let entry=entry.map_err(storage_error)?;let kind=entry.file_type().map_err(storage_error)?;if !kind.is_file()||kind.is_symlink(){continue;}let Some(name)=entry.file_name().to_str().map(str::to_owned)else{continue;};if name.len()>512{continue;}let extension=Path::new(&name).extension().and_then(|e|e.to_str()).unwrap_or("");if ContentFormat::from_extension(extension).is_some_and(|f|matches!(f,ContentFormat::Cue|ContentFormat::M3u|ContentFormat::Chd|ContentFormat::Iso|ContentFormat::Pbp)){entries.insert(name);if entries.len()>64{return Err(invalid("Choose a folder with at most 64 disc entry points"));}}}
        if entries.is_empty(){return Err(invalid("No supported cue sheet, playlist or disc image was found in the selected folder"));}let title=root.file_name().and_then(|v|v.to_str()).unwrap_or("Game").chars().take(120).collect();Ok(Self{source:ScopeSource::Directory(root),title,entries})
    }
    pub fn provided(provider:Arc<dyn DiscProvider>,names:Vec<String>,title:String)->Result<Self>{
        if names.is_empty()||names.len()>64||title.trim().is_empty()||title.len()>512||title.chars().any(char::is_control){return Err(invalid("Native folder selection exceeds its entry/title bounds"));}
        let mut entries=BTreeSet::new();for name in names{let path=relative_reference(Path::new(""),&name)?;let extension=path.extension().and_then(|value|value.to_str()).unwrap_or("");if path.components().count()!=1||name.len()>512||path.to_str()!=Some(name.as_str())||!ContentFormat::from_extension(extension).is_some_and(|format|matches!(format,ContentFormat::Cue|ContentFormat::M3u|ContentFormat::Chd|ContentFormat::Iso|ContentFormat::Pbp))||!entries.insert(name){return Err(invalid("Native folder entry points must be distinct supported local filenames"));}}
        Ok(Self{source:ScopeSource::Provider(provider),title:title.trim().chars().take(120).collect(),entries})
    }
    pub fn entries(&self)->Vec<String>{self.entries.iter().cloned().collect()}
    pub fn title(&self)->String{self.title.clone()}
    pub fn cancellation(&self)->Option<std::sync::Weak<dyn DiscProvider>>{match &self.source{ScopeSource::Provider(provider)=>Some(Arc::downgrade(provider)),ScopeSource::Directory(_)=>None}}
    fn open(&self,relative:&Path)->Result<ProvidedDiscFile>{
        if relative.is_absolute()||relative.components().count()>8{return Err(invalid("Disc reference exceeds the selected folder scope"));}
        if let ScopeSource::Provider(provider)=&self.source{
            let raw=relative.to_str().ok_or_else(||invalid("Native disc reference must be valid UTF-8"))?;let normalized=relative_reference(Path::new(""),raw)?;if normalized!=relative{return Err(invalid("Native disc references must already be normalized"));}
            let file=provider.open(relative)?;if file.identity.as_os_str().len()>4096||file.identity.as_os_str().is_empty()||file.byte_length.is_some_and(|bytes|bytes==0||bytes>MAX_ROM_BYTES){return Err(invalid("Native disc source returned an invalid identity or length"));}return Ok(file);
        }
        let ScopeSource::Directory(root)=&self.source else{return Err(invalid("Disc source is unavailable"));};let mut current=root.clone();
        for part in relative.components(){let std::path::Component::Normal(part)=part else{return Err(invalid("Disc reference contains an invalid path component"));};current.push(part);let metadata=std::fs::symlink_metadata(&current).map_err(storage_error)?;if metadata.file_type().is_symlink(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Disc references may not follow symbolic links or junctions"));}}
        let resolved=current.canonicalize().map_err(storage_error)?;
        if !resolved.starts_with(root)||!std::fs::metadata(&resolved).map_err(storage_error)?.is_file(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Disc reference is outside the selected folder"));}
        let mut options=OpenOptions::new();options.read(true);
        #[cfg(windows)]{use std::os::windows::fs::OpenOptionsExt;options.share_mode(1);}
        let file=options.open(&resolved).map_err(storage_error)?;let bytes=file.metadata().map_err(storage_error)?.len();if bytes==0||bytes>MAX_ROM_BYTES{return Err(invalid("Referenced disc file is empty or exceeds the content bound"));}Ok(ProvidedDiscFile{identity:resolved,byte_length:Some(bytes),reader:Box::new(file)})
    }
}

#[derive(Serialize)]pub struct FolderSelection {pub grant_id:Uuid,pub suggested_title:String,pub entries:Vec<String>}
struct DirectoryGrant {owner:Uuid,expires:u64,scope:DiscScope}
#[derive(Default)]pub struct DirectoryGrants {grants:HashMap<Uuid,DirectoryGrant>}
impl DirectoryGrants {
    pub fn expire(&mut self){self.grants.retain(|_,grant|grant.expires>now_ms());}
    pub fn selected(&mut self,owner:Uuid,path:&Path)->Result<FolderSelection>{self.admit(owner,DiscScope::selected(path)?)}
    pub fn selected_provider(&mut self,owner:Uuid,provider:Arc<dyn DiscProvider>,entries:Vec<String>,title:String)->Result<FolderSelection>{self.admit(owner,DiscScope::provided(provider,entries,title)?)}
    fn admit(&mut self,owner:Uuid,scope:DiscScope)->Result<FolderSelection>{self.expire();if self.grants.len()>=16{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Too many pending folder permissions"));}let result=FolderSelection{grant_id:Uuid::new_v4(),suggested_title:scope.title(),entries:scope.entries()};self.grants.insert(result.grant_id,DirectoryGrant{owner,expires:now_ms()+300_000,scope});Ok(result)}
    pub fn take(&mut self,owner:Uuid,id:Uuid,entry:&str)->Result<DiscScope>{let grant=self.grants.get(&id).ok_or_else(||PlatformError::new(ErrorCode::PermissionExpired,"Folder permission expired; choose the folder again"))?;if grant.owner!=owner{return Err(PlatformError::new(ErrorCode::PermissionDenied,"Folder permission belongs to another client"));}if grant.expires<=now_ms(){self.grants.remove(&id);return Err(PlatformError::new(ErrorCode::PermissionExpired,"Folder permission expired; choose the folder again"));}if !grant.scope.entries.contains(entry){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Entry point was not offered by the selected folder"));}Ok(self.grants.remove(&id).expect("validated grant").scope)}
    pub fn revoke(&mut self,owner:Uuid,id:Uuid)->Result<()>{if self.grants.get(&id).is_some_and(|g|g.owner!=owner){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Folder permission belongs to another client"));}self.grants.remove(&id);Ok(())}
    pub fn revoke_owner(&mut self,owner:Uuid){self.grants.retain(|_,g|g.owner!=owner);}
}

enum Input {Reader(Box<dyn Read+Send>),Bytes(Vec<u8>)}
struct Planned {name:String,bytes:Option<u64>,input:Input}
struct Planner<'a>{scope:&'a DiscScope,encoding:CueEncoding,assets:Vec<Planned>,shared:BTreeMap<PathBuf,String>,cancel:&'a AtomicBool}
impl<'a> Planner<'a>{
    fn check(&self)->Result<()>{if self.cancel.load(Ordering::Relaxed){Err(cancelled())}else if self.assets.len()>MAX_ASSETS{Err(invalid("A disc set may reference at most 128 assets"))}else{Ok(())}}
    fn bytes(&mut self,name:String,data:Vec<u8>)->Result<String>{self.check()?;let bytes=data.len()as u64;if self.assets.len()>=MAX_ASSETS||bytes==0||bytes>MAX_TEXT*4{return Err(invalid("Normalized disc metadata exceeds its bound"));}self.assets.push(Planned{name:name.clone(),bytes:Some(bytes),input:Input::Bytes(data)});Ok(name)}
    fn file(&mut self,relative:&Path,extension:&str,fixed_name:Option<String>)->Result<String>{
        self.check()?;let file=self.scope.open(relative)?;
        if let Some(previous)=self.shared.get(&file.identity){if !previous.ends_with(&format!(".{extension}")){return Err(invalid("A disc file is referenced using incompatible types"));}return Ok(previous.clone());}
        if self.assets.len()>=MAX_ASSETS{return Err(invalid("A disc set may reference at most 128 assets"));}
        let name=fixed_name.unwrap_or_else(||format!("track-{:04}.{extension}",self.shared.len()+1));self.shared.insert(file.identity,name.clone());self.assets.push(Planned{name:name.clone(),bytes:file.byte_length,input:Input::Reader(file.reader)});Ok(name)
    }
    fn text(&self,path:&Path)->Result<String>{let mut file=self.scope.open(path)?;let mut bytes=Vec::new();file.reader.by_ref().take(MAX_TEXT+1).read_to_end(&mut bytes).map_err(disc_read_error)?;if bytes.len()as u64>MAX_TEXT{return Err(invalid("Disc metadata exceeds its text bound"));}decode_text(&bytes,self.encoding)}
    fn cue(&mut self,relative:&Path,name:String)->Result<String>{
        let text=self.text(relative)?;let parent=relative.parent().unwrap_or(Path::new(""));let mut normalized=String::new();let mut has_file=false;let mut tracks=0u32;let mut last_track=0u32;let mut index_seen=false;
        for raw in text.lines(){self.check()?;let line=raw.trim();if line.is_empty(){continue;}if line.len()>4096||line.chars().any(|c|c.is_control()&&c!='\t'){return Err(invalid("Cue sheet contains invalid control characters or oversized lines"));}
            let split=line.find(char::is_whitespace).unwrap_or(line.len());let command=line[..split].to_ascii_uppercase();let args=line[split..].trim();
            match command.as_str(){
                "FILE"=>{let(path,kind)=file_directive(args)?;let extension=match kind.as_str(){"BINARY"=>"bin","WAVE"=>"wav",_=>return Err(invalid("Cue sheet requires an unsupported track encoding"))};let source=relative_reference(parent,&path)?;let canonical=self.file(&source,extension,None)?;normalized.push_str(&format!("FILE \"{canonical}\" {kind}\n"));has_file=true;}
                "TRACK"=>{let mut values=args.split_whitespace();let number=values.next().and_then(|n|n.parse::<u32>().ok()).filter(|n|*n>=1&&*n<=99).ok_or_else(||invalid("Cue track number is invalid"))?;let mode=values.next().unwrap_or("").to_ascii_uppercase();if !has_file||(tracks>0&&!index_seen)||number<=last_track||values.next().is_some()||!["AUDIO","MODE1/2048","MODE1/2352","MODE2/2336","MODE2/2352","CDI/2336","CDI/2352"].contains(&mode.as_str()){return Err(invalid("Cue track order or sector mode is unsupported"));}last_track=number;tracks+=1;index_seen=false;normalized.push_str(&format!("TRACK {number:02} {mode}\n"));}
                "INDEX"=>{let mut values=args.split_whitespace();let number=values.next().and_then(|n|n.parse::<u32>().ok()).filter(|n|*n<=99).ok_or_else(||invalid("Cue index is invalid"))?;let time=values.next().ok_or_else(||invalid("Cue index timestamp is missing"))?;validate_time(time)?;if tracks==0||values.next().is_some(){return Err(invalid("Cue index has no track"));}if number==1{index_seen=true;}normalized.push_str(&format!("INDEX {number:02} {time}\n"));}
                "PREGAP"|"POSTGAP"=>{validate_time(args)?;if tracks==0{return Err(invalid("Cue gap has no track"));}normalized.push_str(&format!("{command} {args}\n"));}
                "FLAGS"=>{if tracks==0||args.split_whitespace().any(|v|!["DCP","PRE","4CH","SCMS"].contains(&v)){return Err(invalid("Cue flags are invalid"));}normalized.push_str(&format!("FLAGS {args}\n"));}
                "REM"|"TITLE"|"PERFORMER"|"SONGWRITER"|"CATALOG"|"ISRC"=>{normalized.push_str(&command);if !args.is_empty(){normalized.push(' ');normalized.push_str(args);}normalized.push('\n');}
                _=>return Err(invalid("Cue sheet contains an unsupported directive; external metadata files are not imported implicitly")),
            }
        }
        if tracks==0||!index_seen{return Err(invalid("Cue sheet is incomplete"));}
        self.bytes(name,normalized.into_bytes())
    }
    fn disc(&mut self,relative:&Path,name:String)->Result<String>{let extension=relative.extension().and_then(|v|v.to_str()).unwrap_or("").to_ascii_lowercase();match extension.as_str(){"cue"=>self.cue(relative,name),"chd"|"iso"|"pbp"=>self.file(relative,&extension,Some(name)),_=>Err(invalid("Playlist entry must be a local cue sheet, CHD, ISO or PBP image"))}}
}

pub fn import_bundle(database:&Database,scope:DiscScope,entry:&str,title:String,system:GameSystem,encoding:CueEncoding,cancel:&AtomicBool,mut progress:impl FnMut(ImportProgress))->Result<GameManifest>{
    if title.trim().is_empty()||title.len()>512||!scope.entries.contains(entry){return Err(invalid("Disc title or entry point is invalid"));}
    let extension=Path::new(entry).extension().and_then(|v|v.to_str()).unwrap_or("");let format=ContentFormat::from_extension(extension).ok_or_else(||invalid("Disc format is unsupported"))?;format.extension(system)?;
    let mut planner=Planner{scope:&scope,encoding,assets:Vec::new(),shared:BTreeMap::new(),cancel};let selected=Path::new(entry);let entry_name=format!("game.{}",format.extension(system)?);let mut discs=Vec::new();
    if format==ContentFormat::M3u{
        let text=planner.text(selected)?;
        for raw in text.lines(){let line=raw.trim();if line.is_empty()||line.starts_with('#'){continue;}if discs.len()>=8{return Err(invalid("Playlist exceeds eight discs"));}let relative=relative_reference(Path::new(""),line)?;let ext=relative.extension().and_then(|v|v.to_str()).unwrap_or("").to_ascii_lowercase();let name=format!("disc-{:02}.{ext}",discs.len()+1);let name=planner.disc(&relative,name)?;discs.push(name);}
        if discs.is_empty(){return Err(invalid("Playlist has no local discs"));}planner.bytes(entry_name.clone(),(discs.join("\n")+"\n").into_bytes())?;
    }else if format==ContentFormat::Cue{planner.cue(selected,entry_name.clone())?;discs.push(entry_name.clone());}
    else{planner.file(selected,format.extension(system)?,Some(entry_name.clone()))?;}
    planner.assets.sort_by(|a,b|a.name.cmp(&b.name));
    let mut known=0u64;let mut complete_size=true;for asset in &planner.assets{if let Some(bytes)=asset.bytes{known=known.checked_add(bytes).filter(|value|*value<=MAX_ROM_BYTES).ok_or_else(||PlatformError::new(ErrorCode::ResourceLimit,"Disc set exceeds its total import bound"))?;}else{complete_size=false;}}let expected_total=if complete_size{known}else{0};
    let count=planner.assets.len()as u16;let mut copied=0u64;let mut metadata=Vec::new();let directory=database.root.join("assets");
    for(index,planned)in planner.assets.into_iter().enumerate(){
        if cancel.load(Ordering::Relaxed){return Err(cancelled());}let mut pending=PendingFile::create(&directory)?;let mut hasher=Sha256::new();let mut bytes=0u64;
        let mut copy=|reader:&mut dyn Read|->Result<()>{
            let mut buffer=[0u8;65536];
            loop{
                if cancel.load(Ordering::Relaxed){return Err(cancelled());}
                let read=reader.read(&mut buffer).map_err(disc_read_error)?;
                if cancel.load(Ordering::Relaxed){return Err(cancelled());}
                if read>buffer.len(){return Err(PlatformError::new(ErrorCode::CorruptData,"Disc reader returned an invalid byte count"));}
                if read==0{break;}
                bytes=bytes.checked_add(read as u64).ok_or_else(||invalid("Disc asset size overflow"))?;
                if bytes>MAX_ROM_BYTES{return Err(PlatformError::new(ErrorCode::ResourceLimit,"Disc asset exceeds its observed byte bound"));}
                if planned.bytes.is_some_and(|expected|bytes>expected){return Err(PlatformError::new(ErrorCode::ContentMismatch,"Disc asset changed during import"));}
                let next=copied.checked_add(read as u64).filter(|value|*value<=MAX_ROM_BYTES).ok_or_else(||PlatformError::new(ErrorCode::ResourceLimit,"Disc set exceeds its observed import bound"))?;
                pending.file.write_all(&buffer[..read]).map_err(storage_error)?;hasher.update(&buffer[..read]);copied=next;
                progress(ImportProgress{phase:"copying".into(),copied_bytes:copied,total_bytes:expected_total,asset_index:(index+1)as u16,asset_count:count});
            }
            Ok(())
        };
        match planned.input{Input::Reader(mut reader)=>copy(reader.as_mut())?,Input::Bytes(data)=>copy(&mut data.as_slice())?,}
        if bytes==0||planned.bytes.is_some_and(|expected|bytes!=expected){return Err(PlatformError::new(ErrorCode::ContentMismatch,"Disc asset length changed during import or is empty"));}
        let hash=ContentHash(format!("{:x}",hasher.finalize()));let target=directory.join(&hash.0);
        if target.exists(){let(existing,length)=hash_file(&mut File::open(&target).map_err(storage_error)?,bytes)?;if existing!=hash||length!=bytes{return Err(PlatformError::new(ErrorCode::CorruptData,"Stored disc asset failed its integrity check"));}}else{pending.commit_new(&target)?;}
        metadata.push(MediaAsset{name:planned.name,content_hash:hash,byte_length:bytes});
    }
    let total=copied;let media=GameMedia{schema_version:1,format,entry_name,assets:metadata,disc_entries:discs};media.validate(system,total)?;let hash=media.identity_hash()?;
    if cancel.load(Ordering::Relaxed){return Err(cancelled());}
    let game=GameManifest{schema_version:1,id:Uuid::new_v4(),title:title.trim().into(),system,content_hash:hash,byte_length:total,region:None,nominal_refresh_hz:None,created_at_ms:now_ms(),media:Some(media)};
    let game=Library::new(database).insert_manifest(game)?;progress(ImportProgress{phase:"complete".into(),copied_bytes:total,total_bytes:total,asset_index:count,asset_count:count});Ok(game)
}
fn disc_read_error(error:std::io::Error)->PlatformError{error.get_ref().and_then(|error|error.downcast_ref::<PlatformError>()).cloned().unwrap_or_else(||storage_error(error))}

fn file_directive(args:&str)->Result<(String,String)>{
    let(path,kind)=if let Some(rest)=args.strip_prefix('"'){let end=rest.find('"').ok_or_else(||invalid("Cue filename quote is incomplete"))?;(&rest[..end],rest[end+1..].trim())}else{let split=args.find(char::is_whitespace).ok_or_else(||invalid("Cue file encoding is missing"))?;(&args[..split],args[split..].trim())};
    if path.is_empty()||path.len()>1024||kind.split_whitespace().count()!=1{return Err(invalid("Cue file directive is invalid"));}Ok((path.to_owned(),kind.to_ascii_uppercase()))
}
fn validate_time(value:&str)->Result<()>{let fields=value.split(':').map(str::parse::<u32>).collect::<std::result::Result<Vec<_>,_>>().map_err(|_|invalid("Cue timestamp is invalid"))?;if fields.len()!=3||fields[0]>999||fields[1]>=60||fields[2]>=75{return Err(invalid("Cue timestamp is outside CD sector timing"));}Ok(())}
fn relative_reference(base:&Path,value:&str)->Result<PathBuf>{
    let value=value.replace('\\',"/");if value.is_empty()||value.len()>2048||value.starts_with('/')||value.contains(':')||value.chars().any(char::is_control){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Disc references must stay inside the selected folder"));}
    let mut parts=base.components().map(|c|c.as_os_str().to_owned()).collect::<Vec<_>>();
    for part in value.split('/') {match part{"."=>{},".."=>{if parts.pop().is_none(){return Err(PlatformError::new(ErrorCode::PermissionDenied,"Disc reference escapes the selected folder"));}},""=>return Err(invalid("Disc path contains an empty component")),v=>{if v.ends_with('.')||v.ends_with(' '){return Err(invalid("Disc path contains an ambiguous component"));}parts.push(v.into());}}}
    if parts.is_empty()||parts.len()>8{return Err(invalid("Disc reference nesting exceeds its bound"));}Ok(parts.into_iter().collect())
}
fn decode_text(bytes:&[u8],encoding:CueEncoding)->Result<String>{
    let utf16=if bytes.starts_with(&[0xff,0xfe]){Some((true,&bytes[2..]))}else if bytes.starts_with(&[0xfe,0xff]){Some((false,&bytes[2..]))}else{match encoding{CueEncoding::Utf16Le=>Some((true,bytes)),CueEncoding::Utf16Be=>Some((false,bytes)),_=>None}};
    let text=if let Some((little,data))=utf16{if data.len()%2!=0{return Err(invalid("Disc metadata has an incomplete UTF-16 code unit"));}let units=data.chunks_exact(2).map(|c|if little{u16::from_le_bytes([c[0],c[1]])}else{u16::from_be_bytes([c[0],c[1]])}).collect::<Vec<_>>();String::from_utf16(&units).map_err(|_|invalid("Disc metadata is not valid UTF-16"))?}
    else{let decoder=match encoding{CueEncoding::Windows1252=>encoding_rs::WINDOWS_1252,CueEncoding::ShiftJis=>encoding_rs::SHIFT_JIS,CueEncoding::Gbk=>encoding_rs::GBK,_=>encoding_rs::UTF_8};let(text,errors)=decoder.decode_without_bom_handling(bytes);if errors{return Err(invalid("Disc metadata encoding could not be decoded; choose its actual text encoding"));}text.trim_start_matches('\u{feff}').to_owned()};
    if text.contains('\0'){return Err(invalid("Disc metadata contains a null byte"));}Ok(text)
}
