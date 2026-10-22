import{requireValue,checkAbort,utf8,fail}from'./errors.js';
import{discReference,discExtension,folderFile}from'./disc-source.js';
const MAX_TEXT=128*1024,MAX_ASSETS=128,MAX_BYTES=4*1024**3;
// Match Rust str::trim/split_whitespace instead of JS's differing BOM/NEL set.
const WS='\\u0009-\\u000d\\u0020\\u0085\\u00a0\\u1680\\u2000-\\u200a\\u2028\\u2029\\u202f\\u205f\\u3000';
const SPACE=new RegExp('['+WS+']','u'),SPLIT=new RegExp('['+WS+']+','u'),TRIM=new RegExp('^['+WS+']+|['+WS+']+$','gu');
const trim=value=>value.replace(TRIM,''),words=value=>trim(value)?trim(value).split(SPLIT):[];
const asciiUpper=value=>value.replace(/[a-z]/g,c=>c.toUpperCase());
function number(value){requireValue(typeof value==='string'&&value.length>0&&!/[^+0-9]/.test(value)&&/^\+?[0-9]+$/.test(value),'invalid_request','A cue number is invalid.');const result=Number(value);requireValue(Number.isSafeInteger(result)&&result>=0&&result<=0xffffffff,'invalid_request','A cue number exceeds its bound.');return result;}
function timestamp(value){const fields=value.split(':').map(number);requireValue(fields.length===3&&fields[0]<=999&&fields[1]<60&&fields[2]<75,'invalid_request','A cue timestamp is outside CD sector timing.');}
function decode(bytes,encoding){
  let label=({auto:'utf-8',utf8:'utf-8',windows1252:'windows-1252',shift_jis:'shift_jis',gbk:'gbk',utf16_le:'utf-16le',utf16_be:'utf-16be'})[encoding],data=bytes,utf16=false;
  requireValue(typeof label==='string','unsupported_capability','The disc text encoding is unsupported.');
  if(bytes[0]===0xff&&bytes[1]===0xfe){label='utf-16le';data=bytes.subarray(2);utf16=true;}
  else if(bytes[0]===0xfe&&bytes[1]===0xff){label='utf-16be';data=bytes.subarray(2);utf16=true;}else utf16=label.startsWith('utf-16');
  requireValue(!utf16||data.length%2===0,'invalid_request','Disc metadata has an incomplete UTF-16 unit.');let text;
  try{text=new TextDecoder(label,{fatal:true,ignoreBOM:true}).decode(data);}catch{fail('invalid_request','Disc metadata could not be decoded. Choose its actual text encoding.');}
  if(!utf16)text=text.replace(/^\ufeff+/u,'');requireValue(!text.includes('\0'),'invalid_request','Disc metadata contains a null byte.');return text;
}
function directive(args){
  let path,kind;if(args.startsWith('"')){const end=args.indexOf('"',1);requireValue(end>=1,'invalid_request','A cue filename quote is incomplete.');path=args.slice(1,end);kind=trim(args.slice(end+1));}
  else{const at=args.search(SPACE);requireValue(at>=0,'invalid_request','A cue file encoding is missing.');path=args.slice(0,at);kind=trim(args.slice(at));}
  requireValue(path.length>0&&utf8(path).length<=1024&&words(kind).length===1,'invalid_request','A cue FILE directive is invalid.');return{path,kind:asciiUpper(kind)};
}
export async function planDiscBundle(scope,entry,system,encoding,{signal}={}){
  checkAbort(signal);requireValue(scope?.entries?.includes(entry),'permission_denied','The disc entry point was not offered by the selected folder.');
  const extension=discExtension(entry),format=extension==='m3u8'?'m3u':extension;
  requireValue(['ps1','psp'].includes(system)&&(['cue','m3u'].includes(format)?system==='ps1':['chd','iso','pbp'].includes(format)),'unsupported_capability','This disc format does not belong to the selected system.');
  requireValue(['auto','utf8','windows1252','shift_jis','gbk','utf16_le','utf16_be'].includes(encoding),'unsupported_capability','The disc text encoding is unsupported.');
  const assets=[],shared=new Map(),discs=[];let total=0;
  function add(name,blob){checkAbort(signal);requireValue(assets.length<MAX_ASSETS&&blob instanceof Blob&&blob.size>0&&blob.size<=MAX_BYTES&&total+blob.size<=MAX_BYTES,'resource_limit','The disc set exceeds 128 assets or four GiB.');assets.push({name,blob});total+=blob.size;return name;}
  function addText(name,text){const bytes=utf8(text);requireValue(bytes.length>0&&bytes.length<=MAX_TEXT*4,'resource_limit','Normalized disc metadata exceeds its limit.');return add(name,new Blob([bytes],{type:'application/octet-stream'}));}
  function file(path,ext,fixed=null){
    checkAbort(signal);const blob=folderFile(scope,path);if(shared.has(path)){const previous=shared.get(path);requireValue(previous.endsWith('.'+ext),'invalid_request','One disc file is referenced using incompatible types.');return previous;}
    const name=fixed??`track-${String(shared.size+1).padStart(4,'0')}.${ext}`;add(name,blob);shared.set(path,name);return name;
  }
  async function text(path){checkAbort(signal);const blob=folderFile(scope,path);requireValue(blob.size<=MAX_TEXT,'resource_limit','Disc metadata exceeds 128 KiB.');const bytes=new Uint8Array(await blob.arrayBuffer());checkAbort(signal);requireValue(bytes.length===blob.size&&bytes.length<=MAX_TEXT,'content_mismatch','Disc metadata changed while being read.');return decode(bytes,encoding);}
  async function cue(path,name){
    const source=await text(path),parent=path.includes('/')?path.slice(0,path.lastIndexOf('/')):'';let output='',hasFile=false,tracks=0,lastTrack=0,indexSeen=false;
    for(const raw of source.split('\n')){
      checkAbort(signal);const line=trim(raw);if(!line)continue;requireValue(utf8(line).length<=4096&&!/[\u0000-\u0008\u000a-\u001f\u007f-\u009f]/.test(line),'invalid_request','The cue contains invalid controls or an oversized line.');
      const at=line.search(SPACE),command=asciiUpper(at<0?line:line.slice(0,at)),args=at<0?'':trim(line.slice(at));
      if(command==='FILE'){const item=directive(args),ext=item.kind==='BINARY'?'bin':item.kind==='WAVE'?'wav':null;requireValue(ext,'unsupported_capability','The cue requires an unsupported track encoding.');const canonical=file(discReference(parent,item.path),ext);output+=`FILE "${canonical}" ${item.kind}\n`;hasFile=true;}
      else if(command==='TRACK'){const values=words(args);requireValue(values.length===2,'invalid_request','A cue TRACK is incomplete.');const n=number(values[0]),mode=asciiUpper(values[1]);requireValue(n>=1&&n<=99&&hasFile&&(!tracks||indexSeen)&&n>lastTrack&&['AUDIO','MODE1/2048','MODE1/2352','MODE2/2336','MODE2/2352','CDI/2336','CDI/2352'].includes(mode),'invalid_request','Cue track order or sector mode is unsupported.');tracks++;lastTrack=n;indexSeen=false;output+=`TRACK ${String(n).padStart(2,'0')} ${mode}\n`;}
      else if(command==='INDEX'){const values=words(args);requireValue(values.length===2&&tracks>0,'invalid_request','A cue INDEX has no track or timestamp.');const n=number(values[0]);requireValue(n<=99,'invalid_request','A cue INDEX is invalid.');timestamp(values[1]);if(n===1)indexSeen=true;output+=`INDEX ${String(n).padStart(2,'0')} ${values[1]}\n`;}
      else if(command==='PREGAP'||command==='POSTGAP'){timestamp(args);requireValue(tracks>0,'invalid_request','A cue gap has no track.');output+=`${command} ${args}\n`;}
      else if(command==='FLAGS'){requireValue(tracks>0&&words(args).every(value=>['DCP','PRE','4CH','SCMS'].includes(value)),'invalid_request','Cue flags are invalid.');output+=`FLAGS ${args}\n`;}
      else if(['REM','TITLE','PERFORMER','SONGWRITER','CATALOG','ISRC'].includes(command))output+=command+(args?' '+args:'')+'\n';
      else fail('unsupported_capability','The cue contains an unsupported directive. Auxiliary files are not imported implicitly.');
    }
    requireValue(tracks>0&&indexSeen,'invalid_request','The cue sheet is incomplete.');return addText(name,output);
  }
  async function disc(path,name){const ext=discExtension(path);if(ext==='cue')return cue(path,name);requireValue(['chd','iso','pbp'].includes(ext),'unsupported_capability','A playlist entry must be a local CUE, CHD, ISO or PBP image.');return file(path,ext,name);}
  const entryName=`game.${format}`;
  if(format==='m3u'){
    const source=await text(entry);for(const raw of source.split('\n')){checkAbort(signal);const line=trim(raw);if(!line||line.startsWith('#'))continue;requireValue(discs.length<8,'resource_limit','A playlist may contain at most eight discs.');const path=discReference('',line);discs.push(await disc(path,`disc-${String(discs.length+1).padStart(2,'0')}.${discExtension(path)}`));}
    requireValue(discs.length>0,'invalid_request','The playlist has no local discs.');addText(entryName,discs.join('\n')+'\n');
  }else if(format==='cue'){await cue(entry,entryName);discs.push(entryName);}else file(entry,format,entryName);
  assets.sort((a,b)=>a.name<b.name?-1:a.name>b.name?1:0);requireValue(new Set(assets.map(asset=>asset.name)).size===assets.length,'invalid_request','Normalized disc names are duplicated.');return{format,entry_name:entryName,disc_entries:discs,assets,total};
}
