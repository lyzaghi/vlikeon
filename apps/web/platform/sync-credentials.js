import{WebSyncCrypto,readSyncCryptoConfiguration}from'./sync-crypto.js';
import{WebSyncVault}from'./sync-vault.js';
import{syncCryptoConfiguration}from'./sync-wasm-policy.js';
import{checkAbort,requireValue}from'./errors.js';

const wipe=buffer=>{if(buffer instanceof ArrayBuffer&&buffer.byteLength)new Uint8Array(buffer).fill(0);};
const accountFrom=(value,registered=false)=>({schema_version:1,account_id:value.account_id,device_id:value.device_id,root_public_key:value.root_public_key,audience:value.audience,certificate:value.certificate,registered});

// Owns the local credential lifetime; HTTP enrollment, transfer journals and
// canonical save reconciliation belong to the separate synchronization adapter.
export class WebSyncCredentials{
  #vault;#crypto=null;#generation=null;#account=null;#busy=false;#closed=false;#epoch=0;
  constructor(store,profile){this.#vault=new WebSyncVault(store,profile);}
  async status({signal}={}){
    const state=await this.#vault.status({signal});let available=false,artifactError=null;
    try{available=Boolean(syncCryptoConfiguration(await readSyncCryptoConfiguration({signal})));}
    catch(error){checkAbort(signal);artifactError=error.code??'untrusted_artifact';}
    return{...state,protocol:1,crypto_available:available,crypto_error:artifactError,unlocked:!this.#closed&&Boolean(this.#crypto&&!this.#crypto.closed&&state.connected&&this.#generation===state.generation),encrypted_sync_available:false};
  }
  async #run(action,{signal}={}){
    checkAbort(signal);requireValue(!this.#closed&&!this.#busy,'busy','The browser credential session is closed or busy.');this.#busy=true;
    try{return await action();}catch(error){this.lock();throw error;}finally{this.#busy=false;}
  }
  #guard(client,epoch,signal){checkAbort(signal);requireValue(!this.#closed&&epoch===this.#epoch&&client===this.#crypto&&!client.closed,'cancelled','The browser credential lifetime ended.');}
  async #fresh(signal){this.lock();const epoch=this.#epoch,client=await WebSyncCrypto.open({signal});if(this.#closed||epoch!==this.#epoch||signal?.aborted){client.close();checkAbort(signal);requireValue(false,'cancelled','Browser credentials were closed.');}this.#crypto=client;return client;}
  async #unlock(signal){
    const status=await this.#vault.status({signal});requireValue(status.connected,'permission_denied','Connect a synchronization account first.');
    if(this.#crypto&&!this.#crypto.closed&&this.#generation===status.generation){const client=this.#crypto,epoch=this.#epoch,account=await this.#vault.assertGeneration(status.generation,{signal});this.#guard(client,epoch,signal);this.#account=account;return;}
    const client=await this.#fresh(signal),epoch=this.#epoch,unlocked=await this.#vault.unlock(status.generation,{signal});
    try{this.#guard(client,epoch,signal);const result=await client.execute({method:'load',account_id:unlocked.account.account_id,root_public_key:unlocked.account.root_public_key,audience:unlocked.account.audience,certificate:unlocked.account.certificate,now_ms:Date.now()},unlocked.credentials,{signal});wipe(result.binary);await this.#vault.assertGeneration(status.generation,{signal});this.#guard(client,epoch,signal);this.#generation=status.generation;this.#account=unlocked.account;}
    finally{wipe(unlocked.credentials);}
  }
  async provision(mode,audience,recoveryKey='',{signal,verify=null}={}){
    return this.#run(async()=>{
      requireValue(['create','recover'].includes(mode)&&typeof audience==='string'&&typeof recoveryKey==='string'&&(verify===null||mode==='recover'&&typeof verify==='function'),'invalid_request','Browser account provisioning is invalid.');const status=await this.#vault.status({signal});requireValue(!status.connected,'conflict','Disconnect the current account before connecting another.');const client=await this.#fresh(signal),epoch=this.#epoch,command=mode==='create'?{method:'create',audience,now_ms:Date.now()}:{method:'recover',recovery_key:recoveryKey,audience,now_ms:Date.now()};let result;
      try{
        result=await client.execute(command,new ArrayBuffer(0),{signal});this.#guard(client,epoch,signal);const account=accountFrom(result.value);
        if(verify){const generation=crypto.randomUUID(),assert=async(expected,{signal:operationSignal}={})=>{checkAbort(signal);checkAbort(operationSignal);const current=await this.#vault.status({signal:operationSignal??signal});requireValue(!this.#closed&&!client.closed&&client===this.#crypto&&epoch===this.#epoch&&!current.connected&&current.generation===status.generation&&expected===generation,'permission_expired','Pending account recovery changed before enrollment.');return structuredClone(account);};
          const pending={assertGeneration:assert,sign:async(operation,{signal:operationSignal}={})=>{await assert(generation,{signal:operationSignal});const signed=await client.execute({method:'sign',operation,now_ms:Date.now()},new ArrayBuffer(0),{signal:operationSignal??signal});wipe(signed.binary);await assert(generation,{signal:operationSignal});return signed.value;}};
          await verify({credentials:pending,generation,account:structuredClone(account)});await assert(generation,{signal});account.registered=true;
        }
        const guard=()=>this.#guard(client,epoch,signal);guard();const saved=await this.#vault.store(account,result.binary,status.generation,{signal,guard});guard();this.#generation=saved.generation;this.#account=saved.account;return{account:saved.account,generation:saved.generation,...(mode==='create'?{recovery_key:result.value.recovery_key}:{})};
      }
      finally{wipe(result?.binary);if(result?.value)delete result.value.recovery_key;command.recovery_key='';}
    },{signal});
  }
  async unlock({signal}={}){return this.#run(async()=>{await this.#unlock(signal);return{generation:this.#generation,account:structuredClone(this.#account)};},{signal});}
  async assertGeneration(generation,{signal}={}){requireValue(!this.#closed,'cancelled','Browser credentials were closed.');return this.#vault.assertGeneration(generation,{signal});}
  async localStatus({signal}={}){requireValue(!this.#closed,'cancelled','Browser credentials were closed.');return this.#vault.status({signal});}
  async #command(command,binary,{signal}={}){
    await this.#unlock(signal);const generation=this.#generation,client=this.#crypto,epoch=this.#epoch;await this.#vault.assertGeneration(generation,{signal});const result=await client.execute(command,binary,{signal});
    try{await this.#vault.assertGeneration(generation,{signal});requireValue(!this.#closed&&epoch===this.#epoch&&client===this.#crypto&&!client.closed,'cancelled','The browser credential session ended before its result was consumed.');return result;}catch(error){wipe(result.binary);throw error;}
  }
  async sign(operation,{signal}={}){return this.#run(async()=>{const result=await this.#command({method:'sign',operation,now_ms:Date.now()},new ArrayBuffer(0),{signal});wipe(result.binary);return result.value;},{signal});}
  async root(operation,recoveryKey,{signal}={}){return this.#run(async()=>{const command={method:'root',operation,recovery_key:recoveryKey,now_ms:Date.now()};try{const result=await this.#command(command,new ArrayBuffer(0),{signal});wipe(result.binary);return result.value;}finally{command.recovery_key='';}},{signal});}
  async renew(recoveryKey,{signal}={}){
    return this.#run(async()=>{await this.#unlock(signal);const client=this.#crypto,epoch=this.#epoch,guard=()=>this.#guard(client,epoch,signal),command={method:'renew',recovery_key:recoveryKey,now_ms:Date.now()};try{const result=await this.#command(command,new ArrayBuffer(0),{signal});wipe(result.binary);guard();const saved=await this.#vault.updateAccount({...this.#account,certificate:result.value},this.#generation,{signal,guard});guard();this.#generation=saved.generation;this.#account=saved.account;return saved;}finally{command.recovery_key='';}},{signal});
  }
  async markRegistered(expectedGeneration,{signal}={}){
    return this.#run(async()=>{const epoch=this.#epoch,guard=()=>{checkAbort(signal);requireValue(!this.#closed&&epoch===this.#epoch,'cancelled','The credential session ended before registration was stored.');};const account=await this.#vault.assertGeneration(expectedGeneration,{signal});guard();const saved=await this.#vault.updateAccount({...account,registered:true},expectedGeneration,{signal,guard});guard();this.lock();return saved;},{signal});
  }
  async seal(metadata,binary,{signal}={}){return this.#run(()=>this.#command({method:'seal',metadata},binary,{signal}),{signal});}
  async open(header,binary,{signal}={}){return this.#run(()=>this.#command({method:'open',header},binary,{signal}),{signal});}
  async disconnect(expectedGeneration,{signal}={}){
    // Independent of a pending worker command: invalidate durable generation,
    // then retire this session. A late provision/action cannot restore it.
    this.lock();return this.#vault.disconnect(expectedGeneration,{signal});
  }
  lock(){++this.#epoch;this.#crypto?.close();this.#crypto=null;this.#generation=null;this.#account=null;}
  close(){if(this.#closed)return;this.#closed=true;this.lock();}
}
