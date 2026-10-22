/* Filled only by tools/assemble_web.py from an explicit source inventory. */
const BUILD='__OMNIPLAY_BUILD_ID__',MANIFEST_HASH='__OMNIPLAY_MANIFEST_HASH__';
const PREFIX=`omniplay-web:${new URL(self.registration.scope).pathname}:`,CACHE=PREFIX+BUILD;
let inventoryPromise=null;
const hex=bytes=>Array.from(new Uint8Array(bytes),value=>value.toString(16).padStart(2,'0')).join('');
async function boundedBytes(response,limit){
  if(!response.ok||!response.body)throw new Error('Asset response unavailable');
  const reader=response.body.getReader(),parts=[];let size=0;
  try{for(;;){const{done,value}=await reader.read();if(done)break;size+=value.byteLength;if(size>limit)throw new Error('Asset response exceeds declared length');parts.push(value);}}
  catch(error){await reader.cancel().catch(()=>{});throw error;}
  const bytes=new Uint8Array(size);let offset=0;for(const part of parts){bytes.set(part,offset);offset+=part.length;}return bytes;
}
async function digest(bytes){return hex(await crypto.subtle.digest('SHA-256',bytes));}
function assetUrl(path){const url=new URL(path,self.registration.scope);if(url.origin!==self.location.origin||!url.href.startsWith(self.registration.scope))throw new Error('Asset escapes app scope');return url.href;}
async function inventory(){
  if(!inventoryPromise)inventoryPromise=(async()=>{
    const cache=await caches.open(CACHE),url=assetUrl('asset-manifest.json'),stored=await cache.match(url),response=stored??await fetch(url,{cache:'no-store',credentials:'same-origin',redirect:'error'}),bytes=await boundedBytes(response,1024*1024);
    if(await digest(bytes)!==MANIFEST_HASH)throw new Error('App inventory integrity failure');const data=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes));
    if(data.schema_version!==1||data.build_id!==BUILD||!Array.isArray(data.assets)||data.assets.length>4096)throw new Error('App inventory mismatch');
    const entries=new Map();
    for(const entry of data.assets){
      if(typeof entry.path!=='string'||entry.path.length>512||entry.path.split('/').some(part=>!/^[A-Za-z0-9_.-]+$/.test(part)||part==='.'||part==='..')||entries.has(entry.path)||!Number.isSafeInteger(entry.bytes)||entry.bytes<=0||entry.bytes>512*1024*1024||!/^[a-f0-9]{64}$/.test(entry.sha256)||typeof entry.mime!=='string'||typeof entry.precache!=='boolean')throw new Error('App asset declaration invalid');
      entries.set(entry.path,entry);
    }
    await cache.put(url,new Response(bytes,{headers:{'Content-Type':'application/json','Cache-Control':'no-store'}}));return entries;
  })().catch(error=>{inventoryPromise=null;throw error;});return inventoryPromise;
}
function responseFor(bytes,entry){
  const headers=new Headers({'Content-Type':entry.mime,'Content-Length':String(bytes.byteLength),'X-Content-Type-Options':'nosniff','Cross-Origin-Resource-Policy':'same-origin','Cross-Origin-Opener-Policy':'same-origin','Cross-Origin-Embedder-Policy':'require-corp','Referrer-Policy':'no-referrer','Cache-Control':'no-cache'});
  if(entry.path==='index.html')headers.set('Content-Security-Policy',"default-src 'none'; script-src 'self' 'wasm-unsafe-eval'; worker-src 'self'; style-src 'self'; img-src 'self' data:; font-src 'self'; connect-src 'self' __OMNIPLAY_CONNECT_SOURCES__; media-src 'self' blob:; manifest-src 'self'; object-src 'none'; base-uri 'none'; form-action 'none'; frame-ancestors 'none'");
  return new Response(bytes,{status:200,headers});
}
async function fetchAsset(entry,{mustCache=false}={}){
  const response=await fetch(assetUrl(entry.path),{cache:'no-store',credentials:'same-origin',redirect:'error'}),bytes=await boundedBytes(response,entry.bytes);
  if(bytes.length!==entry.bytes||await digest(bytes)!==entry.sha256)throw new Error('App asset integrity failure');const result=responseFor(bytes,entry);
  try{await(await caches.open(CACHE)).put(assetUrl(entry.path),result.clone());}catch(error){if(mustCache)throw error;}return result;
}
self.addEventListener('install',event=>event.waitUntil((async()=>{
  try{const entries=await inventory();for(const entry of entries.values())if(entry.precache)await fetchAsset(entry,{mustCache:true});}
  catch(error){await caches.delete(CACHE);throw error;}
  // Do not force an update into another tab's running game. The browser activates
  // this worker after clients of the prior worker have closed normally.
})()));
self.addEventListener('activate',event=>event.waitUntil((async()=>{for(const name of await caches.keys())if(name.startsWith(PREFIX)&&name!==CACHE)await caches.delete(name);await self.clients.claim();})()));
self.addEventListener('fetch',event=>{
  const request=event.request,url=new URL(request.url);if(request.method!=='GET'||url.origin!==self.location.origin||!url.href.startsWith(self.registration.scope))return;
  event.respondWith((async()=>{
    const entries=await inventory();let relative=url.href.slice(self.registration.scope.length);if(relative===''&&request.mode==='navigate')relative='index.html';const entry=entries.get(relative);
    // User games, saves, account requests and unknown URLs are never placed in
    // the application cache. Only explicit inventoried static resources qualify.
    if(!entry)return fetch(request);
    const cached=await(await caches.open(CACHE)).match(assetUrl(relative));if(cached)return cached;
    try{return await fetchAsset(entry);}catch{return new Response('An OmniPlay resource is unavailable. Reconnect to download the approved app resources.',{status:503,headers:{'Content-Type':'text/plain; charset=utf-8','Cache-Control':'no-store'}});}
  })());
});
self.addEventListener('message',event=>{
  const port=event.ports?.[0];if(!port||event.data?.method!=='cache_status')return;
  event.waitUntil((async()=>{const entries=await inventory(),cache=await caches.open(CACHE),cores=[];for(const entry of entries.values())if(entry.path.startsWith('cores/')&&entry.path.endsWith('.wasm'))cores.push({path:entry.path,cached:Boolean(await cache.match(assetUrl(entry.path))),bytes:entry.bytes});port.postMessage({build_id:BUILD,cores});port.close();})());
});
