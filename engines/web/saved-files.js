import{FILE_LIMITS,filePath,fileAlias,encodeFiles,decodeFiles}from'./saved-file-codec.js';
const BLOCK=65536;
export class SavedFiles {
  #FS;#C;#root=null;#all=new Set();#allocated=0;#mounted=false;
  constructor(core){
    this.#FS=core.FS;const C=core._om_web_fs_constant;
    if(!this.#FS||typeof C!=='function'||!Array.isArray(this.#FS.streams)||typeof this.#FS.destroyNode!=='function')throw new Error('This installed emulator does not support bounded persistent files.');
    this.#C={io:C(1),invalid:C(2),missing:C(3),isDirectory:C(5),notDirectory:C(6),badFile:C(7),unsupported:C(8),dir:C(20),file:C(21),seekSet:C(24),seekCur:C(25),seekEnd:C(26),denied:C(27),space:C(32),notEmpty:C(33),exists:C(34),typeMask:C(35),memory:C(36),tooBig:C(37)};
    this.#FS.mount({mount:()=>this.#make(null,'/',true)}, {},'/saves');this.#mounted=true;
  }
  #error(key){return new this.#FS.ErrnoError(this.#C[key]);}
  #path(node){const parts=[];let current=node;while(current!==this.#root){if(!current||!this.#all.has(current)||!current.linked)throw this.#error('missing');parts.push(current.name);current=current.parent;if(parts.length>FILE_LIMITS.depth)throw this.#error('invalid');}return parts.reverse().join('/');}
  #childPath(parent,name){try{const base=this.#path(parent),path=base?base+'/'+name:name;filePath(path);return path;}catch{throw this.#error('invalid');}}
  #name(parent,name,exclude=null){this.#childPath(parent,name);const alias=fileAlias(name);for(const child of parent.children.values())if(child!==exclude&&fileAlias(child.name)===alias)throw this.#error('exists');}
  #sweep(){const opened=new Set(this.#FS.streams.filter(Boolean).map(stream=>stream.node));for(const node of this.#all)if(node!==this.#root&&!node.linked&&!opened.has(node)){this.#allocated-=node.blocks.size*BLOCK;node.blocks.clear();this.#all.delete(node);}}
  #budget(sizeDelta=0,entriesDelta=0,filesDelta=0,nameDelta=0){
    this.#sweep();let bytes=12,files=filesDelta,entries=entriesDelta;
    for(const node of this.#all){if(node===this.#root)continue;entries++;if(!node.directory)files++;bytes+=12+node.pathBytes+(node.directory?0:node.size);}
    if(entries>FILE_LIMITS.entries||files>FILE_LIMITS.files||bytes+sizeDelta+nameDelta>FILE_LIMITS.bytes)throw this.#error('space');
  }
  #make(parent,name,directory){
    let path='';if(parent){if(!parent.directory)throw this.#error('notDirectory');this.#name(parent,name);path=this.#childPath(parent,name);this.#budget(0,1,directory?0:1,12+new TextEncoder().encode(path).length);}
    const node=this.#FS.createNode(parent,name,(directory?this.#C.dir|0o700:this.#C.file|0o600),0);
    Object.assign(node,{directory,children:new Map(),blocks:new Map(),size:0,linked:true,pathBytes:new TextEncoder().encode(path).length,atime:Date.now(),mtime:Date.now(),ctime:Date.now()});this.#all.add(node);if(!parent)this.#root=node;else parent.children.set(name,node);
    node.node_ops=this.#nodeOps();node.stream_ops=this.#streamOps();return node;
  }
  #resize(node,size){
    if(node.directory)throw this.#error('isDirectory');if(!Number.isSafeInteger(size)||size<0||size>FILE_LIMITS.bytes)throw this.#error('tooBig');this.#budget(size-node.size);
    if(size<node.size){for(const[index]of node.blocks)if(index*BLOCK>=size){node.blocks.delete(index);this.#allocated-=BLOCK;}if(size%BLOCK)node.blocks.get(Math.floor(size/BLOCK))?.fill(0,size%BLOCK);}
    node.size=size;node.mtime=node.ctime=Date.now();
  }
  #read(node,buffer,offset,length,position){
    if(node.directory)throw this.#error('isDirectory');this.#buffer(buffer,offset,length,position);const count=Math.min(length,Math.max(0,node.size-position));let copied=0;
    while(copied<count){const at=position+copied,index=Math.floor(at/BLOCK),within=at%BLOCK,amount=Math.min(count-copied,BLOCK-within),block=node.blocks.get(index),output=buffer.subarray(offset+copied,offset+copied+amount);if(block)output.set(block.subarray(within,within+amount));else output.fill(0);copied+=amount;}node.atime=Date.now();return count;
  }
  #buffer(buffer,offset,length,position){if(!ArrayBuffer.isView(buffer)||buffer.BYTES_PER_ELEMENT!==1||![offset,length,position].every(Number.isSafeInteger)||offset<0||length<0||position<0||offset>buffer.byteLength-length||position>FILE_LIMITS.bytes)throw this.#error('invalid');}
  #write(node,buffer,offset,length,position){
    if(node.directory)throw this.#error('isDirectory');this.#buffer(buffer,offset,length,position);if(length>FILE_LIMITS.bytes-position)throw this.#error('space');if(!length)return 0;const end=position+length;this.#budget(Math.max(0,end-node.size));
    const missing=[];for(let index=Math.floor(position/BLOCK);index<=Math.floor((end-1)/BLOCK);index++)if(!node.blocks.has(index))missing.push(index);
    if(this.#allocated+missing.length*BLOCK>FILE_LIMITS.bytes)throw this.#error('space');let staged;
    try{staged=missing.map(index=>[index,new Uint8Array(BLOCK)]);}catch{throw this.#error('memory');}
    for(const[index,block]of staged)node.blocks.set(index,block);this.#allocated+=staged.length*BLOCK;
    let copied=0;while(copied<length){const at=position+copied,within=at%BLOCK,count=Math.min(length-copied,BLOCK-within);node.blocks.get(Math.floor(at/BLOCK)).set(buffer.subarray(offset+copied,offset+copied+count),within);copied+=count;}
    node.size=Math.max(node.size,end);node.mtime=node.ctime=Date.now();return length;
  }
  #detach(parent,name,directory){const node=parent.children.get(name);if(!node)throw this.#error('missing');if(node.directory!==directory)throw this.#error(directory?'notDirectory':'isDirectory');if(directory&&node.children.size)throw this.#error('notEmpty');parent.children.delete(name);node.linked=false;parent.mtime=parent.ctime=Date.now();}
  #rename(node,parent,name){
    this.#path(node);this.#path(parent);if(!parent.directory)throw this.#error('notDirectory');let current=parent;while(current!==this.#root){if(current===node)throw this.#error('invalid');current=current.parent;}
    const target=parent.children.get(name);this.#childPath(parent,name);const alias=fileAlias(name);for(const child of parent.children.values())if(child!==node&&child!==target&&fileAlias(child.name)===alias)throw this.#error('exists');if(target===node&&node.name===name&&node.parent===parent)return;
    if(target&&target!==node){if(target.directory!==node.directory)throw this.#error(target.directory?'isDirectory':'notDirectory');if(target.directory&&target.children.size)throw this.#error('notEmpty');}
    const oldPath=this.#path(node),newPath=this.#childPath(parent,name),changes=[];let nameDelta=0;
    for(const item of this.#all){if(!item.linked||item===this.#root)continue;const path=this.#path(item);if(path===oldPath||path.startsWith(oldPath+'/')){const value=newPath+path.slice(oldPath.length);try{filePath(value);}catch{throw this.#error('invalid');}const bytes=new TextEncoder().encode(value).length;changes.push([item,bytes]);nameDelta+=bytes-item.pathBytes;}}
    // destroyNode evicts by inode identity, retaining open-stream storage here.
    // It is idempotent with newer FS.rename wrappers that repeat that eviction;
    // older wrappers need this explicit removal to avoid a stale target lookup.
    this.#budget(0,0,0,nameDelta);if(target&&target!==node){target.linked=false;this.#FS.destroyNode(target);}node.parent.children.delete(node.name);parent.children.set(name,node);node.name=name;node.parent=parent;for(const[item,size]of changes)item.pathBytes=size;node.ctime=parent.mtime=parent.ctime=Date.now();
  }
  #nodeOps(){return{
    getattr:node=>({dev:1,ino:node.id,mode:node.mode,nlink:node.linked?1:0,uid:0,gid:0,rdev:0,size:node.directory?4096:node.size,atime:new Date(node.atime),mtime:new Date(node.mtime),ctime:new Date(node.ctime),blksize:BLOCK,blocks:node.blocks.size*BLOCK/512}),
    setattr:(node,attr)=>{if(attr.mode!==undefined&&((attr.mode&this.#C.typeMask)!==(node.mode&this.#C.typeMask)))throw this.#error('denied');if(attr.size!==undefined)this.#resize(node,attr.size);if(attr.mode!==undefined)node.mode=attr.mode;for(const key of['atime','mtime','ctime'])if(attr[key]!==undefined){if(!Number.isFinite(attr[key]))throw this.#error('invalid');node[key]=attr[key];}},
    lookup:(parent,name)=>{if(!parent.directory)throw this.#error('notDirectory');const node=parent.children.get(name);if(!node)throw this.#error('missing');return node;},
    mknod:(parent,name,mode)=>{const type=mode&this.#C.typeMask;if(type!==this.#C.dir&&type!==this.#C.file)throw this.#error('denied');return this.#make(parent,name,type===this.#C.dir);},
    rename:(node,parent,name)=>this.#rename(node,parent,name),unlink:(parent,name)=>this.#detach(parent,name,false),rmdir:(parent,name)=>this.#detach(parent,name,true),
    readdir:node=>{if(!node.directory)throw this.#error('notDirectory');return['.','..',...node.children.keys()];},symlink:()=>{throw this.#error('denied');},readlink:()=>{throw this.#error('invalid');}
  };}
  #streamOps(){return{
    open:()=>{},close:()=>{},flush:()=>{},read:(stream,buffer,offset,length,position)=>this.#read(stream.node,buffer,offset,length,position),write:(stream,buffer,offset,length,position)=>this.#write(stream.node,buffer,offset,length,position),
    llseek:(stream,offset,whence)=>{if(!Number.isSafeInteger(offset))throw this.#error('invalid');let position=offset;if(whence===this.#C.seekCur)position+=stream.position;else if(whence===this.#C.seekEnd)position+=stream.node.size;else if(whence!==this.#C.seekSet)throw this.#error('invalid');if(!Number.isSafeInteger(position)||position<0||position>FILE_LIMITS.bytes)throw this.#error('invalid');return position;},
    allocate:()=>{throw this.#error('unsupported');},mmap:()=>{throw this.#error('unsupported');},msync:()=>{throw this.#error('unsupported');}
  };}
  archive(admit=null){const entries=[];for(const node of this.#all)if(node!==this.#root&&node.linked){const name=this.#path(node);entries.push({name,directory:node.directory,size:node.directory?0:node.size,read:(output,at)=>this.#read(node,output,0,output.byteLength,at)});}if(admit!==null){if(typeof admit!=='function')throw new Error('The persistent-file admission is invalid.');admit(entries);}return encodeFiles(entries);}
  restore(archive){
    const entries=decodeFiles(archive);if(this.#root.children.size||this.#FS.streams.some(stream=>stream&&this.#all.has(stream.node)))throw new Error('Persistent files can only be restored into an unused mounted tree.');
    const sorted=[...entries].sort((a,b)=>a.name.split('/').length-b.name.split('/').length||(a.directory===b.directory?0:a.directory?-1:1));
    for(const entry of sorted){const parts=filePath(entry.name);let parent=this.#root;for(let i=0;i<parts.length-1;i++){let child=parent.children.get(parts[i]);if(!child)child=this.#make(parent,parts[i],true);if(!child.directory)throw this.#error('notDirectory');parent=child;}let node=parent.children.get(parts.at(-1));if(node){if(!entry.directory||!node.directory)throw this.#error('exists');}else node=this.#make(parent,parts.at(-1),entry.directory);if(!entry.directory){let at=0;const block=new Uint8Array(BLOCK);while(at<entry.size){const count=entry.read(block.subarray(0,Math.min(BLOCK,entry.size-at)),at);if(!Number.isInteger(count)||count<=0||count>Math.min(BLOCK,entry.size-at))throw this.#error('io');this.#write(node,block,0,count,at);at+=count;}}}
  }
  assertNoOpen(){if(this.#FS.streams.some(stream=>stream&&this.#all.has(stream.node)))throw new Error('The emulator did not close its persistent-file handles.');}
  dispose(){if(!this.#mounted)return;this.assertNoOpen();this.#FS.unmount('/saves');this.#mounted=false;for(const node of this.#all)node.blocks.clear();this.#all.clear();this.#root=null;this.#allocated=0;}
}
