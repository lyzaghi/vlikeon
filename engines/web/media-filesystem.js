/** Read-only Emscripten JS FS over already verified, exclusively held OPFS files. */
export function mountMedia(core,files,mountpoint='/game'){
  if(!['/game','/system'].includes(mountpoint))throw new Error('The runtime filesystem mountpoint is invalid.');
  const FS=core.FS,C=core._om_web_fs_constant;
  if(!FS||typeof C!=='function'||['createNode','mount','unmount','ErrnoError'].some(key=>typeof FS[key]!=='function'))throw new Error('This installed emulator does not support file-backed game media.');
  const constants={io:C(1),invalid:C(2),missing:C(3),readonly:C(4),directory:C(5),notDirectory:C(6),notImplemented:C(8),dirMode:C(20),fileMode:C(21),accessMode:C(22),readOnly:C(23),seekSet:C(24),seekCur:C(25),seekEnd:C(26),append:C(28),create:C(29),truncate:C(30),exclusive:C(31)};
  const error=key=>new FS.ErrnoError(constants[key]),deny=()=>{throw error('readonly');};
  const nodeOps={
    getattr(node){return{dev:1,ino:node.id,mode:node.mode,nlink:1,uid:0,gid:0,rdev:0,size:node.record?node.record.entry.byte_length:4096,atime:new Date(0),mtime:new Date(0),ctime:new Date(0),blksize:4096,blocks:Math.ceil((node.record?.entry.byte_length??4096)/4096)};},
    setattr:deny,mknod:deny,rename:deny,unlink:deny,rmdir:deny,symlink:deny,
    lookup(parent,name){if(parent.record)throw error('notDirectory');const value=parent.children.get(name);if(!value)throw error('missing');return value;},
    readdir(node){if(node.record)throw error('notDirectory');return['.','..',...node.children.keys()];},
    readlink(){throw error('invalid');}
  };
  const streamOps={
    open(stream){if((stream.flags&constants.accessMode)!==constants.readOnly||(stream.flags&(constants.append|constants.create|constants.truncate|constants.exclusive)))throw error('readonly');},
    close(){},flush(){},
    read(stream,buffer,offset,length,position){
      const record=stream.node.record;if(!record)throw error('directory');
      if(!ArrayBuffer.isView(buffer)||buffer.BYTES_PER_ELEMENT!==1||![offset,length,position].every(Number.isSafeInteger)||offset<0||length<0||position<0||offset>buffer.byteLength||length>buffer.byteLength-offset)throw error('invalid');
      const count=Math.min(length,Math.max(0,record.entry.byte_length-position),1024*1024);if(!count)return 0;
      try{const output=new Uint8Array(buffer.buffer,buffer.byteOffset+offset,count),read=record.handle.read(output,{at:position});if(!Number.isSafeInteger(read)||read<=0||read>count)throw error('io');return read;}catch{throw error('io');}
    },
    write:deny,allocate:deny,msync:deny,
    mmap(){throw error('notImplemented');},
    llseek(stream,offset,whence){
      if(!Number.isSafeInteger(offset))throw error('invalid');let position=offset;
      if(whence===constants.seekCur)position+=stream.position;else if(whence===constants.seekEnd)position+=stream.node.record?.entry.byte_length??0;else if(whence!==constants.seekSet)throw error('invalid');
      if(!Number.isSafeInteger(position)||position<0)throw error('invalid');return position;
    }
  };
  const backend={mount(){
    const create=(parent,name,directory)=>{const node=FS.createNode(parent,name,(directory?constants.dirMode:constants.fileMode)|(directory?0o555:0o444),0);if(directory)node.children=new Map();node.node_ops=nodeOps;node.stream_ops=streamOps;return node;};
    const root=create(null,'/',true);
    for(const[name,record]of files){
      if(typeof name!=='string'||!name.length||name.length>240)throw error('invalid');const parts=name.split('/');
      if(parts.length>(mountpoint==='/system'?4:1)||parts.some(part=>!part||part==='.'||part==='..'||/[\\\x00-\x1f]/.test(part)))throw error('invalid');
      let parent=root;for(const part of parts.slice(0,-1)){let node=parent.children.get(part);if(node?.record)throw error('invalid');if(!node){node=create(parent,part,true);parent.children.set(part,node);}parent=node;}
      const leaf=parts.at(-1);if(parent.children.has(leaf))throw error('invalid');const node=create(parent,leaf,false);node.record=record;parent.children.set(leaf,node);
    }return root;
  }};
  FS.mount(backend,{},mountpoint);let mounted=true;
  return()=>{if(mounted){FS.unmount(mountpoint);mounted=false;}};
}
