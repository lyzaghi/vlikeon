import{DOMAINS,UUID,StreamError,requireValue,bytes,b64,unb64,sha,random,sign,verify,inspect,newExchange,pairKey,validGrant,documentHash,label,audience,fingerprint,exactKeys}from'./crypto.js';
import{RelayClient}from'./relay.js';

function inviteBody(value,relay){
  const now=Date.now();requireValue(exactKeys(value,['schema_version','id','audience','host_public_key','label','secret_hash','created_at_ms','expires_at_ms'])&&value.schema_version===1&&UUID.test(value.id)&&value.audience===relay&&Number.isSafeInteger(value.created_at_ms)&&Number.isSafeInteger(value.expires_at_ms)&&value.created_at_ms<=now+30000&&value.expires_at_ms>now&&value.expires_at_ms>value.created_at_ms&&value.expires_at_ms-value.created_at_ms<=300000&&/^[a-f0-9]{64}$/.test(value.secret_hash),'permission_expired','The pairing invitation is invalid or expired.');label(value.label);requireValue(unb64(value.host_public_key,32).length===32,'invalid_request','Host identity is invalid.');return value;
}
function helloBody(value,invite){
  requireValue(exactKeys(value,['schema_version','invite_id','client_public_key','client_ecdh','client_nonce','label','created_at_ms'])&&value.schema_version===1&&value.invite_id===invite.id&&Number.isSafeInteger(value.created_at_ms)&&Math.abs(value.created_at_ms-Date.now())<=300000,'invalid_request','Pairing request is invalid.');label(value.label);requireValue(unb64(value.client_public_key,32).length===32&&unb64(value.client_nonce,32).length===32,'invalid_request','Client pairing identity is invalid.');const ecdh=unb64(value.client_ecdh,65);requireValue(ecdh.length===65&&ecdh[0]===4,'invalid_request','Client exchange key is invalid.');return value;
}
export async function comparisonCode(invite,hello){const hash=await sha(bytes(JSON.stringify(['omniplay.streaming.pair-code.v1',invite.payload,hello.payload])));return(parseInt(hash.slice(0,8),16)%1000000).toString().padStart(6,'0');}

export class PairingManager{
  #store;#allowed;#identity;#invite=null;#joining=null;
  constructor(store,identity,allowed){this.#store=store;this.#identity=identity;this.#allowed=allowed;}
  async create(relay,name,{signal}={}){
    if(this.#invite?.value.expires_at_ms<=Date.now())this.#invite=null;requireValue(!this.#invite,'busy','Close the current invitation before creating another.');audience(relay);const now=Date.now(),secret=random(32),value={schema_version:1,id:crypto.randomUUID(),audience:relay,host_public_key:this.#identity.public_key,label:label(name),secret_hash:await sha(secret),created_at_ms:now,expires_at_ms:now+300000},document=await sign(this.#identity,DOMAINS.invite,value),client=new RelayClient(this.#identity,relay,this.#allowed);
    await client.request({type:'create_invite',invite:document},{signal});this.#invite={value,document,client,secret:b64(secret)};
    return{code:'OPRP1.'+b64(bytes(JSON.stringify({invite:document,secret:b64(secret)}))),expires_at_ms:value.expires_at_ms,host_fingerprint:await fingerprint(this.#identity.public_key)};
  }
  async pending({signal}={}){
    const invite=this.#invite;requireValue(invite&&invite.value.expires_at_ms>Date.now(),'permission_expired','The invitation expired. Create another to continue pairing.');const result=await invite.client.request({type:'poll_invite',invite_id:invite.value.id},{signal});requireValue(Array.isArray(result.requests)&&result.requests.length<=8,'invalid_request','Pairing response is invalid.');const entries=[];
    for(const document of result.requests){const untrusted=inspect(document,4096),hello=await verify(untrusted.client_public_key,DOMAINS.hello,document,4096);helloBody(hello,invite.value);entries.push({document,hello,comparison_code:await comparisonCode(invite.document,document),fingerprint:await fingerprint(hello.client_public_key)});}return entries;
  }
  async approve(document,inputAllowed,{signal}={}){
    const invite=this.#invite;requireValue(invite&&invite.value.expires_at_ms>Date.now()&&typeof inputAllowed==='boolean','permission_expired','The invitation expired.');const untrusted=inspect(document,4096),hello=await verify(untrusted.client_public_key,DOMAINS.hello,document,4096);helloBody(hello,invite.value);
    const exchange=await newExchange(),now=Date.now(),grant={schema_version:1,id:crypto.randomUUID(),invite_id:invite.value.id,audience:invite.value.audience,host_public_key:this.#identity.public_key,client_public_key:hello.client_public_key,client_hello_hash:await documentHash(document),client_ecdh:hello.client_ecdh,host_ecdh:exchange.public_key,client_nonce:hello.client_nonce,host_nonce:b64(random(32)),input_allowed:inputAllowed,created_at_ms:now,expires_at_ms:now+90*86400000},signed=await sign(this.#identity,DOMAINS.grant,grant),key=await pairKey(exchange,hello.client_ecdh,signed);
    // Persist before releasing authorization to the relay. A failed request can
    // be retried from the retained unconfirmed pair rather than losing its key.
    await this.#store.savePair({id:grant.id,role:'host',audience:grant.audience,host_public_key:grant.host_public_key,client_public_key:grant.client_public_key,label:hello.label,input_allowed:inputAllowed,grant:signed,key,created_at_ms:now,expires_at_ms:grant.expires_at_ms});
    await invite.client.request({type:'approve_pair',invite_id:grant.invite_id,client_public_key:grant.client_public_key,grant:signed},{signal});await this.#store.confirm(grant.id);return{id:grant.id,label:hello.label};
  }
  async retryApproval(id,{signal}={}){const record=await this.#store.pair(id);requireValue(record.role==='host'&&!record.relay_confirmed,'invalid_state','This pair does not need approval recovery.');const grant=inspect(record.grant),relay=new RelayClient(this.#identity,record.audience,this.#allowed);try{await relay.request({type:'ice_credentials',grant:record.grant},{signal});}catch(error){if(!['unauthorized','not_found'].includes(error.code))throw error;await relay.request({type:'approve_pair',invite_id:grant.invite_id,client_public_key:record.client_public_key,grant:record.grant},{signal});}await this.#store.confirm(id);}
  async decline(publicKey,{signal}={}){const invite=this.#invite;requireValue(invite,'invalid_state','No invitation is open.');await invite.client.request({type:'decline_pair',invite_id:invite.value.id,client_public_key:publicKey},{signal});}
  async closeInvite({signal}={}){const invite=this.#invite;if(!invite)return;this.#invite=null;if(invite.value.expires_at_ms>Date.now())await invite.client.request({type:'close_invite',invite_id:invite.value.id},{signal});}
  async join(code,name,{signal}={}){
    requireValue(!this.#joining&&typeof code==='string'&&code.startsWith('OPRP1.')&&code.length<=16000,'invalid_request','Enter a current OmniPlay pairing invitation.');let invitation;try{invitation=JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(unb64(code.slice(6),10000)));}catch{throw new StreamError('invalid_request','Pairing invitation encoding is invalid.');}
    requireValue(exactKeys(invitation,['invite','secret']),'invalid_request','Pairing invitation is invalid.');const raw=inspect(invitation.invite,4096),relay=audience(raw.audience),invite=await verify(raw.host_public_key,DOMAINS.invite,invitation.invite,4096);inviteBody(invite,relay);
    const secret=unb64(invitation.secret,32);requireValue(secret.length===32&&await sha(secret)===invite.secret_hash&&invite.host_public_key!==this.#identity.public_key,'unauthorized','Pairing invitation secret or endpoint is invalid.');
    const exchange=await newExchange(),hello={schema_version:1,invite_id:invite.id,client_public_key:this.#identity.public_key,client_ecdh:exchange.public_key,client_nonce:b64(random(32)),label:label(name),created_at_ms:Date.now()},document=await sign(this.#identity,DOMAINS.hello,hello),client=new RelayClient(this.#identity,relay,this.#allowed);
    await client.request({type:'join_invite',invite_id:invite.id,secret:invitation.secret,hello:document},{signal});this.#joining={invite,document,hello,exchange,client};
    return{host_label:invite.label,host_fingerprint:await fingerprint(invite.host_public_key),comparison_code:await comparisonCode(invitation.invite,document),expires_at_ms:invite.expires_at_ms};
  }
  async collect({signal}={}){
    const joining=this.#joining;requireValue(joining&&joining.invite.expires_at_ms>Date.now(),'permission_expired','The pairing request expired.');const result=await joining.client.request({type:'pairing_status',invite_id:joining.invite.id},{signal});requireValue(['pending','declined','approved'].includes(result.state),'invalid_request','Pairing status is invalid.');if(result.state!=='approved')return{state:result.state};
    const grant=await verify(joining.invite.host_public_key,DOMAINS.grant,result.grant,8192);validGrant(grant,joining.invite.audience);
    requireValue(grant.invite_id===joining.invite.id&&grant.client_public_key===this.#identity.public_key&&grant.host_public_key===joining.invite.host_public_key&&grant.client_hello_hash===await documentHash(joining.document)&&grant.client_ecdh===joining.exchange.public_key&&grant.client_nonce===joining.hello.client_nonce,'unauthorized','Pairing authorization does not match this device or invitation.');
    const key=await pairKey(joining.exchange,grant.host_ecdh,result.grant);await this.#store.savePair({id:grant.id,role:'client',audience:grant.audience,host_public_key:grant.host_public_key,client_public_key:grant.client_public_key,label:joining.invite.label,input_allowed:grant.input_allowed,grant:result.grant,key,created_at_ms:grant.created_at_ms,expires_at_ms:grant.expires_at_ms});await this.#store.confirm(grant.id);this.#joining=null;return{state:'approved',id:grant.id};
  }
  cancelJoin(){this.#joining=null;}
  async revoke(id,{signal}={}){const pair=await this.#store.revocation(id);requireValue(pair,'not_found','Pairing is unavailable.');await this.#store.revoke(id);if(pair.expires_at_ms>Date.now())try{await new RelayClient(this.#identity,pair.audience,this.#allowed).request({type:'revoke_pair',grant:pair.grant},{signal});}catch(error){if(error.code!=='not_found')throw error;}await this.#store.confirmRevocation(id);}
}
