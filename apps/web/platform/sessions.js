import{UUID,identifier,integer,requireValue}from'./errors.js';

const terminal=state=>['stopped','failed'].includes(state);
const transitions={ready:['running','stopped','failed'],running:['running','paused','stopped','failed'],paused:['running','paused','stopped','failed']};
export class WebSessions {
  #store;#catalog;#owner=crypto.randomUUID();#active=new Map();#queue=Promise.resolve();#closed=false;
  constructor(store,catalog){this.#store=store;this.#catalog=catalog;}
  async begin(gameId,runtimeId,{signal}={}){
    requireValue(!this.#closed&&UUID.test(gameId)&&identifier(runtimeId),'invalid_request','Observed session identity is invalid.');
    const now=Date.now(),info=await this.#store.transaction(['games','leases'],'readwrite',async tx=>{
      const game=await tx.get('games',gameId),core=this.#catalog.find(core=>core.id===runtimeId);
      requireValue(game&&core?.distribution_approved===true&&core.security_revoked!==true&&core.systems.includes(game.system),'untrusted_artifact','This content needs an approved bundled runtime.');
      const leases=await tx.all('leases');let live=0;
      for(const lease of leases){
        if(!terminal(lease.info.state)&&lease.expires_at_ms<=now){lease.info.state='failed';lease.info.error={code:'runtime_crashed',message:'The web game stopped reporting its lifecycle.',recoverable:true};lease.info.updated_at_ms=now;await tx.put('leases',lease);}
        if(!terminal(lease.info.state))++live;
      }
      requireValue(live<2,'resource_limit','Close an active game before starting another.');
      const result={id:crypto.randomUUID(),game_id:gameId,runtime_id:runtimeId,runtime_kind:'web_wasm',state:'ready',capabilities:['pause','keyboard_mouse','gamepad','touch_controls'],created_at_ms:now,updated_at_ms:now,error:null,close_requested:false};
      await tx.add('leases',{id:result.id,owner:this.#owner,game_id:gameId,info:result,expires_at_ms:now+30000});
      // Historical lifecycle rows are not save evidence and are bounded separately.
      const old=leases.filter(lease=>terminal(lease.info.state)).sort((a,b)=>b.info.updated_at_ms-a.info.updated_at_ms);for(const lease of old.slice(31))await tx.delete('leases',lease.id);return result;
    },signal);
    this.#active.set(info.id,{info,last_seen:performance.now()});
    if(this.#active.size>32)for(const[id,value]of this.#active)if(terminal(value.info.state))this.#active.delete(id);
    if(this.#closed){await this.heartbeat(info.id,'failed').catch(()=>{});requireValue(false,'cancelled','The browser page closed during session preparation.');}return structuredClone(info);
  }
  heartbeat(id,state,{signal}={}){const result=this.#queue.then(()=>this.#heartbeat(id,state,signal));this.#queue=result.then(()=>{},()=>{});return result;}
  async #heartbeat(id,state,signal){
    const observed=this.#active.get(id);requireValue(observed&&!terminal(observed.info.state)&&transitions[observed.info.state]?.includes(state),'invalid_state','Observed session transition is invalid.');
    const now=performance.now(),elapsed=observed.info.state==='running'?Math.min(15000,Math.max(0,Math.floor(now-observed.last_seen))):0,wall=Date.now();
    const info=await this.#store.transaction(['games','leases'],'readwrite',async tx=>{
      const lease=await tx.get('leases',id);requireValue(lease?.owner===this.#owner&&!terminal(lease.info.state),'invalid_state','The web session has ended. Reopen the game from saved progress.');
      const game=await tx.get('games',observed.info.game_id);requireValue(game&&integer(game.playtime_ms,0,Number.MAX_SAFE_INTEGER-elapsed),'corrupt_data','Observed game playtime is invalid.');
      if(elapsed)await tx.put('games',{...game,playtime_ms:game.playtime_ms+elapsed,last_played_at_ms:wall});
      const result={...lease.info,state,updated_at_ms:wall};await tx.put('leases',{...lease,info:result,expires_at_ms:wall+(state==='paused'?600000:30000)});return result;
    },signal);
    observed.info=info;observed.last_seen=now;return structuredClone(info);
  }
  list(){return Array.from(this.#active.values(),value=>structuredClone(value.info));}
  activeFor(gameId){return Array.from(this.#active.values()).some(value=>value.info.game_id===gameId&&!terminal(value.info.state));}
  async close(){
    this.#closed=true;
    const ids=Array.from(this.#active.values()).filter(value=>!terminal(value.info.state)).map(value=>value.info.id);
    for(const id of ids)await this.heartbeat(id,'failed').catch(()=>{});this.#active.clear();
  }
}
