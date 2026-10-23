import{requireValue}from'./crypto.js';

export class NativeInputForwarder extends EventTarget{
  #bridge;#session;#pair;#identity;#grant=null;#closed=false;#latest=null;#pendingRelease=false;#pumping=false;#statusBusy=false;#sequence=0;#timer=0;#abort=new AbortController();#arming=false;#armGeneration=0;
  constructor(bridge,session,pair,identity){
    super();this.#bridge=bridge;this.#session=session;this.#pair=pair;this.#identity=identity;const options={signal:this.#abort.signal};
    session.addEventListener('input',event=>{if(this.#closed||!this.#grant)return;const value=event.detail;if(this.#latest){for(const key of['dx','dy','wheel'])value.mouse[key]=Math.max(-2048,Math.min(2048,value.mouse[key]+this.#latest.mouse[key]));}this.#latest=value;void this.#pump();},options);
    session.addEventListener('release_input',()=>void this.release(),options);session.addEventListener('state',event=>{if(['stopping','closed'].includes(event.detail.state))void this.close();},options);
    session.addEventListener('input_expired',()=>void this.revoke('input_timeout'),options);
  }
  async games(){const list=await this.#bridge.request('installed_game_processes');requireValue(Array.isArray(list)&&list.length<=36,'invalid_request','Installed game process list is invalid.');const games=[];for(const process of list.filter(p=>p.running)){const game=await this.#bridge.request('library_get',{game_id:process.game_id});if(game.system==='pc'&&!games.some(g=>g.id===game.id))games.push(game);}return games;}
  async arm(gameId){
    requireValue(!this.#closed&&!this.#arming&&!this.#grant&&this.#session.active&&this.#session.role==='host'&&this.#pair.input_allowed&&this.#session.id,'invalid_state','Native game control cannot be authorized in this state.');this.#arming=true;const session_id=this.#session.id,generation=++this.#armGeneration;
    try{
      const capture=this.#session.captureSnapshot?.owned_capture;
      requireValue(capture?.protocol===1&&capture.game_id===gameId&&capture.session_id===session_id,'permission_denied','Capture this owned game window before authorizing its input.');
      const grant=await this.#bridge.request('stream_input_arm',{game_id:gameId,pair_id:this.#pair.id,session_id,host_public_key:this.#identity.public_key,grant:this.#pair.grant,capture_id:capture.capture_id},120000);
      if(this.#closed||generation!==this.#armGeneration||!this.#session.active||session_id!==this.#session.id){await this.#bridge.request('stream_input_revoke',{grant_id:grant.grant_id,session_id},15000).catch(()=>{});return;}
      requireValue(grant.session_id===session_id&&grant.pair_id===this.#pair.id&&grant.game_id===gameId&&grant.keyboard_mouse===true&&grant.gamepad===false,'unauthorized','Native input grant identity is invalid.');
      this.#grant=grant;this.#sequence=0;this.#session.armInput(true);this.dispatchEvent(new CustomEvent('status',{detail:grant}));this.#timer=setInterval(()=>void this.#status(),1000);
    }finally{this.#arming=false;}
  }
  async #pump(){
    if(this.#pumping)return;this.#pumping=true;let requestGrant=null;
    try{while((this.#pendingRelease||this.#latest)&&this.#grant&&!this.#closed){const grant=this.#grant;requestGrant=grant.grant_id;let value;
      if(this.#pendingRelease){this.#pendingRelease=false;value=await this.#bridge.request('stream_input_release',{grant_id:grant.grant_id,session_id:grant.session_id},5000);}
      else{const frame=this.#latest;this.#latest=null;value=await this.#bridge.request('stream_input_frame',{grant_id:grant.grant_id,session_id:grant.session_id,sequence:++this.#sequence,frame},5000);}
      if(this.#grant?.grant_id!==grant.grant_id)break;this.#observe(value);
    }}
    catch(error){if(this.#grant?.grant_id===requestGrant){this.dispatchEvent(new CustomEvent('input_error',{detail:{message:error.message}}));await this.revoke();}}
    finally{this.#pumping=false;if((this.#pendingRelease||this.#latest)&&this.#grant&&!this.#closed)queueMicrotask(()=>void this.#pump());}
  }
  #observe(value){
    requireValue(this.#grant&&value.grant_id===this.#grant.grant_id&&value.session_id===this.#grant.session_id&&value.pair_id===this.#pair.id,'unauthorized','Native input status changed identity.');this.#grant=value;this.dispatchEvent(new CustomEvent('status',{detail:value}));if(value.state==='revoked'){this.#grant=null;this.#latest=null;this.#pendingRelease=false;clearInterval(this.#timer);this.#session.armInput(false);}
  }
  async #status(){const grant=this.#grant;if(!grant||this.#closed||this.#pumping||this.#statusBusy)return;this.#statusBusy=true;try{const value=await this.#bridge.request('stream_input_status',{grant_id:grant.grant_id,session_id:grant.session_id},5000);if(this.#grant?.grant_id===grant.grant_id)this.#observe(value);}catch{if(this.#grant?.grant_id===grant.grant_id)await this.revoke();}finally{this.#statusBusy=false;}}
  async release(){this.#latest=null;if(!this.#grant||this.#closed)return;this.#pendingRelease=true;await this.#pump();}
  async revoke(reason='host_stopped'){++this.#armGeneration;const grant=this.#grant;this.#grant=null;this.#latest=null;this.#pendingRelease=false;clearInterval(this.#timer);this.#session.armInput(false);if(grant){this.dispatchEvent(new CustomEvent('status',{detail:{...grant,state:'revoked',reason}}));await this.#bridge.request('stream_input_revoke',{grant_id:grant.grant_id,session_id:grant.session_id},15000).catch(()=>{});}}
  async close(){if(this.#closed)return;this.#closed=true;this.#abort.abort();await this.revoke();}
}
