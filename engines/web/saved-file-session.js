import{SavedFiles}from'./saved-files.js';
import{decodeFiles,FILE_LIMITS}from'./saved-file-codec.js';
import{MdPortableFiles,portableMdFiles,MD_PORTABLE_FORMAT}from'./md-portable.js';

const encoder=new TextEncoder(),MAGIC={in_game:'OPWH0001',emulator_state:'OPWF0001'};
function invalid(message='The persistent-file save envelope is invalid.'){throw new Error(message);}
function bytes(value){if(value instanceof ArrayBuffer)return new Uint8Array(value);if(value instanceof Uint8Array)return value;invalid();}
function envelope(kind,base,files){
  base=bytes(base);files=bytes(files);const magic=MAGIC[kind],total=16+base.length+files.length;
  if(!magic||base.length<1||files.length<12||total>FILE_LIMITS.bytes)invalid('The combined game state and persistent files exceed the save limit.');
  const output=new Uint8Array(total),view=new DataView(output.buffer);output.set(encoder.encode(magic));view.setUint32(8,base.length,true);view.setUint32(12,files.length,true);output.set(base,16);output.set(files,16+base.length);return output.buffer;
}
function parse(kind,payload){
  const data=bytes(payload),magic=MAGIC[kind];if(!magic||data.length<29||data.length>FILE_LIMITS.bytes)invalid();
  const view=new DataView(data.buffer,data.byteOffset,data.byteLength),baseSize=view.getUint32(8,true),fileSize=view.getUint32(12,true),expected=encoder.encode(magic);
  if(expected.some((value,index)=>data[index]!==value)||!baseSize||fileSize<12||16+baseSize+fileSize!==data.length)invalid();
  const base=data.subarray(16,16+baseSize),files=data.subarray(16+baseSize);decodeFiles(files);return{base,files};
}

// Only explicitly admitted adaptations can opt in. The core-specific barrier
// drains guest/private producers; pthread builds must join savedata work and
// serialize FS access back to this actor. fflush alone cannot establish that.
export class SavedFileSession{
  #core;#tree;#closed=false;#barrier;#portable=null;
  constructor(core,policy,barrier=null,portable=null){
    if(!policy||Object.keys(policy).length!==3||policy.protocol!==1||typeof policy.threads!=='boolean'||policy.io!==(policy.threads?'pthread_joined_barrier_v1':'single_threaded_barrier_v1'))invalid('The installed core has no supported persistent-file I/O contract.');
    if(typeof core._om_web_flush_save_io!=='function'||typeof core._om_web_flush_files!=='function')invalid('The installed core is missing its persistent-file barrier.');
    if(barrier!==null&&typeof barrier!=='function')invalid('The persistent-file barrier is invalid.');
    this.#core=core;this.#barrier=barrier??(()=>core._om_web_flush_save_io());this.#tree=new SavedFiles(core);
    if(portable!==null)this.#portable=new MdPortableFiles(core,portable);
  }
  get portableFormat(){return this.#portable?MD_PORTABLE_FORMAT:null;}
  get inGameAvailable(){return this.#portable?this.#portable.available():true;}
  flush(){
    if(this.#closed||this.#barrier()!==1||this.#core._om_web_flush_files()!==0)invalid('The emulator could not finish writing persistent game files.');
    this.#portable?.contract();
  }
  capture(kind,captureBase){
    this.flush();
    if(kind==='in_game'&&this.#portable){if(!this.#portable.available())throw Object.assign(new Error('This game has no persistent save memory. Use a snapshot to save its current position.'),{code:'unsupported_capability'});const archive=this.#tree.archive(entries=>this.#portable.admitEntries(entries));this.#portable.validate(archive);return archive;}
    const base=captureBase(kind);this.flush();return envelope(kind,base,this.#tree.archive());
  }
  restore(kind,payload,hooks){
    // Decode the complete archive and inner state before stopping or changing
    // any part of the healthy guest. Backups remain separate bounded buffers.
    let target;
    if(kind==='in_game'&&this.#portable){const files=portableMdFiles(payload);this.#portable.validate(files);const base=new Uint8Array(16);base.set(encoder.encode('OPRM0001'));target={base,files};}
    else target=parse(kind,payload);
    const parsed=hooks.parseBase(kind,target.base);
    this.flush();const previous=hooks.parseBase('emulator_state',hooks.captureBase('emulator_state'));this.flush();const previousFiles=this.#tree.archive();
    try{
      this.#replace(target.files,hooks);
      if(!hooks.applyBase(kind,parsed))invalid('The emulator rejected the restored game progress.');
      this.flush();
    }catch(error){
      if(error.unrecoverable)throw error;
      try{
        this.#replace(previousFiles,hooks);
        if(!hooks.applyBase('emulator_state',previous))invalid();
        this.flush();
      }catch{
        throw Object.assign(new Error('The emulator could not recover its previous persistent files and state. Reopen the game; earlier committed saves remain available.'),{unrecoverable:true});
      }
      throw new Error('The emulator rejected the saved progress. Its previous persistent files and state were restored.',{cause:error});
    }
  }
  #replace(archive,hooks){
    hooks.stopGuest();this.#tree.assertNoOpen();this.#tree.dispose();
    // Assign before population, so a failed allocation/import is itself
    // replaceable during rollback without keeping an old mounted reference.
    this.#tree=new SavedFiles(this.#core);this.#tree.restore(archive);hooks.bootGuest();
  }
  dispose(){if(this.#closed)return;this.#tree.dispose();this.#portable?.dispose();this.#portable=null;this.#closed=true;}
}
