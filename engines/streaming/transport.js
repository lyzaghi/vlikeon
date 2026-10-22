import{RelayClient}from'./relay.js';
import{StreamError,requireValue,UUID,seal,open,bytes}from'./crypto.js';
function sleep(ms,signal){return new Promise(resolve=>{if(signal.aborted){resolve();return;}const done=()=>{clearTimeout(timer);signal.removeEventListener('abort',done);resolve();},timer=setTimeout(done,ms);signal.addEventListener('abort',done,{once:true});});}
export class SignalTransport extends EventTarget{
  #store;#identity;#pair;#relay;#owner=crypto.randomUUID();#abort=new AbortController();#running=false;#queue=Promise.resolve();#seen=new Set();#cursor=0;
  constructor(store,identity,pair,allowed){super();this.#store=store;this.#identity=identity;this.#pair=pair;this.#relay=new RelayClient(identity,pair.audience,allowed);this.#cursor=pair.cursor;}
  get pair(){const{key,...record}=this.#pair;return record;}
  get closed(){return this.#abort.signal.aborted;}
  async ice(){const value=await this.#relay.request({type:'ice_credentials',grant:this.#pair.grant},{signal:this.#abort.signal});requireValue(Array.isArray(value.ice_servers)&&value.ice_servers.length<=8&&Number.isSafeInteger(value.expires_at_ms)&&value.expires_at_ms>Date.now(),'invalid_request','ICE authorization is invalid.');return value;}
  async start(){requireValue(!this.#running&&!this.closed&&this.#pair.relay_confirmed,'invalid_state','The signaling receiver cannot start.');await this.#store.lease(this.#pair.id,this.#owner);if(this.closed){await this.#store.release(this.#pair.id,this.#owner);throw new StreamError('cancelled','Signaling was closed.');}this.#running=true;void this.#poll();}
  send(sessionId,value){const task=this.#queue.then(()=>this.#send(sessionId,value));this.#queue=task.then(()=>{},()=>{});return task;}
  async #send(sessionId,value){
    requireValue(!this.closed&&UUID.test(sessionId),'invalid_state','The remote-play session is closed.');await this.#store.pair(this.#pair.id);const sequence=await this.#store.reserve(this.#pair.id),packet={message_id:crypto.randomUUID(),session_id:sessionId,sequence,expires_at_ms:Date.now()+60000};
    const encrypted=await seal(this.#pair.key,this.#pair.id,this.#identity.public_key,packet,value),operation={type:'send',grant:this.#pair.grant,...packet,...encrypted};let last;
    for(let attempt=0;attempt<3&&!this.closed;attempt++){
      try{await this.#relay.request(operation,{signal:this.#abort.signal});return;}catch(error){last=error;if(error.code!=='network_unavailable')throw error;await sleep(250*(attempt+1),this.#abort.signal);}
    }throw last??new StreamError('cancelled','Signaling was closed.');
  }
  async #poll(){
    let failedSince=null;
    while(!this.closed){
      try{
        await this.#store.pair(this.#pair.id);await this.#store.lease(this.#pair.id,this.#owner);
        const response=await this.#relay.request({type:'receive',grant:this.#pair.grant,after:this.#cursor,limit:8},{signal:this.#abort.signal});requireValue(Array.isArray(response.packets)&&response.packets.length<=8,'invalid_request','Signaling inbox is invalid.');
        for(const packet of response.packets){
          requireValue(!this.closed&&Number.isSafeInteger(packet.cursor)&&packet.cursor>this.#cursor&&Number.isSafeInteger(packet.sequence)&&packet.sequence>0&&packet.sequence<=1000000&&UUID.test(packet.message_id)&&UUID.test(packet.session_id)&&Number.isSafeInteger(packet.expires_at_ms)&&packet.expires_at_ms>Date.now()-30000&&packet.expires_at_ms<=Date.now()+90000,'unauthorized','Signaling replay, timestamp or identity was rejected.');
          const peer=this.#pair.role==='host'?this.#pair.client_public_key:this.#pair.host_public_key;requireValue(packet.sender_public_key===peer&&!this.#seen.has(packet.message_id),'unauthorized','Signaling sender or message identity is invalid.');
          const value=await open(this.#pair.key,this.#pair.id,peer,packet);requireValue(value&&typeof value==='object'&&!Array.isArray(value)&&bytes(JSON.stringify(value)).length<=48*1024,'invalid_request','Decrypted signaling data is invalid.');await this.#store.accept(this.#pair.id,this.#owner,packet);this.#cursor=packet.cursor;this.#seen.add(packet.message_id);if(this.#seen.size>256)this.#seen.delete(this.#seen.values().next().value);
          this.dispatchEvent(new CustomEvent('signal',{detail:{session_id:packet.session_id,value}}));
        }
        failedSince=null;this.dispatchEvent(new CustomEvent('relay_state',{detail:{connected:true}}));await sleep(response.packets.length===8?50:750,this.#abort.signal);
      }catch(error){
        if(this.closed)break;
        if(error.code==='network_unavailable'||error.code==='resource_limit'){failedSince??=performance.now();this.dispatchEvent(new CustomEvent('relay_state',{detail:{connected:false}}));if(performance.now()-failedSince<20000){await sleep(1500,this.#abort.signal);continue;}}
        this.dispatchEvent(new CustomEvent('transport_error',{detail:{code:error.code??'runtime_crashed',message:error.message}}));await this.close();break;
      }
    }
  }
  async close(){if(this.closed)return;this.#abort.abort();this.#running=false;await this.#store.release(this.#pair.id,this.#owner).catch(()=>{});}
}
