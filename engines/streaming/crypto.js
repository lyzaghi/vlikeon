const UTF8=new TextEncoder(),DECODER=new TextDecoder('utf-8',{fatal:true});
export const DOMAINS={request:'omniplay.streaming.request.v1\0',invite:'omniplay.streaming.invite.v1\0',hello:'omniplay.streaming.hello.v1\0',grant:'omniplay.streaming.grant.v1\0'};
export const UUID=/^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$/;
export class StreamError extends Error{constructor(code,message){super(message);this.code=code;this.recoverable=true;}}
export function requireValue(value,code,message){if(!value)throw new StreamError(code,message);}
export function bytes(value){return UTF8.encode(value);}
export function b64(data){const input=new Uint8Array(data);let out='';for(let i=0;i<input.length;i+=8192)out+=String.fromCharCode(...input.subarray(i,i+8192));return btoa(out).replaceAll('+','-').replaceAll('/','_').replaceAll('=','');}
export function unb64(value,max){requireValue(typeof value==='string'&&value.length>0&&value.length<=Math.ceil(max*4/3)&&/^[A-Za-z0-9_-]+$/.test(value),'invalid_request','Streaming encoding is invalid.');let raw;try{raw=atob(value.replaceAll('-','+').replaceAll('_','/'));}catch{throw new StreamError('invalid_request','Streaming encoding is invalid.');}const data=Uint8Array.from(raw,c=>c.charCodeAt(0));requireValue(data.length<=max&&b64(data)===value,'invalid_request','Streaming encoding exceeds its bound.');return data;}
export async function sha(data){return Array.from(new Uint8Array(await crypto.subtle.digest('SHA-256',data)),v=>v.toString(16).padStart(2,'0')).join('');}
export function random(n){return crypto.getRandomValues(new Uint8Array(n));}
export function audience(value){let url;try{url=new URL(value);}catch{throw new StreamError('invalid_request','Enter a configured HTTPS signaling origin.');}requireValue(value.length<=256&&url.protocol==='https:'&&url.origin===value&&!url.username&&!url.password,'invalid_request','Use an exact configured HTTPS signaling origin.');return value;}
export function label(value){requireValue(typeof value==='string'&&bytes(value).length>0&&bytes(value).length<=120&&!/[\x00-\x1f\x7f]/.test(value),'invalid_request','Device label is invalid.');return value;}
export function exactKeys(value,keys){return value&&typeof value==='object'&&!Array.isArray(value)&&Object.keys(value).length===keys.length&&Object.keys(value).every(key=>keys.includes(key));}
export async function newIdentity(){
  requireValue(isSecureContext&&crypto?.subtle,'unsupported_capability','Secure browser cryptography is required for remote play.');
  let key;try{key=await crypto.subtle.generateKey({name:'Ed25519'},false,['sign','verify']);}catch{throw new StreamError('unsupported_capability','This browser does not support the required remote-play identity algorithm.');}
  return{private_key:key.privateKey,public_key:b64(await crypto.subtle.exportKey('raw',key.publicKey))};
}
export async function validateIdentity(identity){
  requireValue(identity?.private_key instanceof CryptoKey&&identity.private_key.type==='private'&&identity.private_key.algorithm.name==='Ed25519'&&!identity.private_key.extractable&&identity.private_key.usages.length===1&&identity.private_key.usages[0]==='sign'&&unb64(identity.public_key,32).length===32,'corrupt_data','Streaming device identity needs recovery.');
  // A persisted key must prove possession of the pinned public half before use.
  const proof=await sign(identity,DOMAINS.hello,{purpose:'identity_check'});await verify(identity.public_key,DOMAINS.hello,proof,4096);return identity;
}
export async function sign(identity,domain,value){const raw=bytes(JSON.stringify(value));requireValue(raw.length<=96*1024,'resource_limit','Streaming signed data exceeds its bound.');const prefix=bytes(domain),message=new Uint8Array(prefix.length+raw.length);message.set(prefix);message.set(raw,prefix.length);return{payload:b64(raw),signature:b64(await crypto.subtle.sign('Ed25519',identity.private_key,message))};}
export function inspect(document,max=8192){requireValue(exactKeys(document,['payload','signature']),'invalid_request','Signed streaming document is invalid.');let value;try{value=JSON.parse(DECODER.decode(unb64(document.payload,max)));}catch(error){if(error instanceof StreamError)throw error;throw new StreamError('invalid_request','Signed streaming JSON is invalid.');}return value;}
export async function verify(publicKey,domain,document,max=8192){
  const value=inspect(document,max),keyBytes=unb64(publicKey,32),signature=unb64(document.signature,64);requireValue(keyBytes.length===32&&signature.length===64,'unauthorized','Streaming signature is invalid.');
  const key=await crypto.subtle.importKey('raw',keyBytes,'Ed25519',false,['verify']),raw=unb64(document.payload,max),prefix=bytes(domain),message=new Uint8Array(prefix.length+raw.length);message.set(prefix);message.set(raw,prefix.length);
  requireValue(await crypto.subtle.verify('Ed25519',key,signature,message),'unauthorized','Streaming signature verification failed.');return value;
}
export async function documentHash(document){return sha(bytes(JSON.stringify({payload:document.payload,signature:document.signature})));}
export async function fingerprint(publicKey){const value=await sha(unb64(publicKey,32));return value.match(/.{1,4}/g).join(' ');}
export async function newExchange(){const pair=await crypto.subtle.generateKey({name:'ECDH',namedCurve:'P-256'},false,['deriveBits']);return{private_key:pair.privateKey,public_key:b64(await crypto.subtle.exportKey('raw',pair.publicKey))};}
export async function pairKey(exchange,peerPublic,grantDocument){
  const raw=unb64(peerPublic,65);requireValue(raw.length===65&&raw[0]===4,'invalid_request','Pairing exchange key is invalid.');const peer=await crypto.subtle.importKey('raw',raw,{name:'ECDH',namedCurve:'P-256'},false,[]);
  const material=new Uint8Array(await crypto.subtle.deriveBits({name:'ECDH',public:peer},exchange.private_key,256));
  try{const base=await crypto.subtle.importKey('raw',material,'HKDF',false,['deriveKey']);const salt=await crypto.subtle.digest('SHA-256',bytes(JSON.stringify({payload:grantDocument.payload,signature:grantDocument.signature})));return await crypto.subtle.deriveKey({name:'HKDF',hash:'SHA-256',salt,info:bytes('omniplay.streaming.signal-key.v1')},base,{name:'AES-GCM',length:256},false,['encrypt','decrypt']);}finally{material.fill(0);}
}
export function validGrant(grant,relay,now=Date.now()){
  requireValue(exactKeys(grant,['schema_version','id','invite_id','audience','host_public_key','client_public_key','client_hello_hash','client_ecdh','host_ecdh','client_nonce','host_nonce','input_allowed','created_at_ms','expires_at_ms'])&&grant.schema_version===1&&UUID.test(grant.id)&&UUID.test(grant.invite_id)&&grant.audience===relay&&typeof grant.input_allowed==='boolean'&&Number.isSafeInteger(grant.created_at_ms)&&Number.isSafeInteger(grant.expires_at_ms)&&grant.created_at_ms<=now+30000&&grant.expires_at_ms>now&&grant.expires_at_ms>grant.created_at_ms&&grant.expires_at_ms-grant.created_at_ms<=90*86400000&&/^[a-f0-9]{64}$/.test(grant.client_hello_hash),'unauthorized','Pairing authorization is invalid or expired.');
  for(const field of['host_public_key','client_public_key'])requireValue(unb64(grant[field],32).length===32,'unauthorized','Pairing identity is invalid.');requireValue(grant.host_public_key!==grant.client_public_key,'unauthorized','Pairing endpoints must be distinct.');
  for(const field of['client_ecdh','host_ecdh']){const raw=unb64(grant[field],65);requireValue(raw.length===65&&raw[0]===4,'unauthorized','Pairing exchange is invalid.');}for(const field of['client_nonce','host_nonce'])requireValue(unb64(grant[field],32).length===32,'unauthorized','Pairing nonce is invalid.');return grant;
}
export function signalAad(pairId,sender,packet){return bytes(JSON.stringify(['omniplay.streaming.signal.v1',pairId,sender,packet.message_id,packet.session_id,packet.sequence,packet.expires_at_ms]));}
export async function seal(key,pairId,sender,packet,value){const plaintext=bytes(JSON.stringify(value));requireValue(plaintext.length<=48*1024-16,'resource_limit','Streaming signal exceeds its bound.');const iv=random(12),ciphertext=await crypto.subtle.encrypt({name:'AES-GCM',iv,additionalData:signalAad(pairId,sender,packet),tagLength:128},key,plaintext);return{iv:b64(iv),ciphertext:b64(ciphertext)};}
export async function open(key,pairId,sender,packet){const iv=unb64(packet.iv,12),ciphertext=unb64(packet.ciphertext,48*1024);requireValue(iv.length===12&&ciphertext.length>=16,'unauthorized','Encrypted signal is invalid.');let raw;try{raw=await crypto.subtle.decrypt({name:'AES-GCM',iv,additionalData:signalAad(pairId,sender,packet),tagLength:128},key,ciphertext);}catch{throw new StreamError('unauthorized','Encrypted signaling authentication failed.');}try{return JSON.parse(DECODER.decode(raw));}catch{throw new StreamError('invalid_request','Encrypted signaling payload is invalid.');}}
