// Authored source only. Execution is deferred to a frozen Antigravity batch.
// These cases exercise actual forwarder async boundaries with fake native/RTC
// endpoints; they do not qualify process exit or real controller removal.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
const uri=source=>'data:text/javascript;base64,'+Buffer.from(source).toString('base64');
const cryptoSource=await readFile(new URL('../../engines/streaming/crypto.js',import.meta.url),'utf8');
const source=await readFile(new URL('../../engines/streaming/native-input.js',import.meta.url),'utf8');
const {NativeInputForwarder}=await import(uri(source.replace("'./crypto.js'",JSON.stringify(uri(cryptoSource)))));
if(typeof globalThis.CustomEvent==='undefined')globalThis.CustomEvent=class extends Event{constructor(type,options={}){super(type);this.detail=options.detail;}};
const id=n=>`00000000-0000-4000-8000-${String(n).padStart(12,'0')}`;
const grant={grant_id:id(1),game_id:id(2),pair_id:id(3),session_id:id(4),
  keyboard_mouse:true,gamepad:false,expires_at_ms:Date.now()+60000,state:'waiting_for_target',reason:null};
const revoked=()=>({...grant,state:'revoked',reason:'host_stopped'});
const deferred=()=>{let resolve,reject;const promise=new Promise((yes,no)=>{resolve=yes;reject=no;});return{promise,resolve,reject};};
class Session extends EventTarget{
  active=true;role='host';id=grant.session_id;armed=false;
  captureSnapshot={owned_capture:{protocol:1,game_id:grant.game_id,session_id:grant.session_id,capture_id:id(5)}};
  armInput(enabled){this.armed=enabled;}
}
function fixture(t){
  const session=new Session(),calls=[],statuses=[];
  const handlers={stream_input_arm:async()=>({...grant}),stream_input_revoke:async()=>revoked(),
    stream_input_status:async()=>revoked(),stream_input_frame:async()=>({...grant,state:'active'})};
  const bridge={async request(method,params){calls.push({method,params});assert.ok(handlers[method],method);return handlers[method](params);}};
  const forwarder=new NativeInputForwarder(bridge,session,{id:grant.pair_id,input_allowed:true,grant:{}},{public_key:'fixture'});
  forwarder.addEventListener('status',event=>statuses.push(event.detail));
  t.after(()=>forwarder.close());return{forwarder,session,calls,statuses,handlers,arm:()=>forwarder.arm(grant.game_id)};
}
test('revocation blocks input immediately and waits for one exact native receipt',async t=>{
  const f=fixture(t);await f.arm();const gate=deferred();f.handlers.stream_input_revoke=()=>gate.promise;
  const first=f.forwarder.revoke(),second=f.forwarder.revoke();assert.equal(first,second);
  assert.equal(f.session.armed,false);await Promise.resolve();
  assert.equal(f.statuses.at(-1).state,'revoking');assert.ok(!f.statuses.some(value=>value.state==='revoked'));
  await assert.rejects(f.arm());gate.resolve(revoked());await first;
  assert.equal(f.statuses.at(-1).state,'revoked');assert.equal(f.calls.filter(call=>call.method==='stream_input_revoke').length,1);
});
test('lost reply retains unresolved ownership and a later status can confirm it',async t=>{
  const f=fixture(t);await f.arm();f.handlers.stream_input_revoke=async()=>{throw Error('lost reply');};
  await f.forwarder.revoke();assert.equal(f.statuses.at(-1).state,'cleanup_unknown');await assert.rejects(f.arm());
  await f.forwarder.revoke();assert.equal(f.statuses.at(-1).state,'revoked');
  assert.equal(f.calls.filter(call=>call.method==='stream_input_revoke').length,1);
  assert.equal(f.calls.filter(call=>call.method==='stream_input_status').length,1);
});
test('pending cleanup never becomes permission to authorize another grant',async t=>{
  const f=fixture(t);await f.arm();const pending={...grant,state:'cleanup_required',reason:'provider_failure'};
  f.handlers.stream_input_revoke=async()=>pending;f.handlers.stream_input_status=async()=>pending;
  await f.forwarder.revoke();assert.equal(f.statuses.at(-1).state,'cleanup_required');await assert.rejects(f.arm());
  await f.forwarder.revoke();assert.equal(f.statuses.at(-1).state,'cleanup_required');assert.equal(f.session.armed,false);
});
test('foreign native revocation identity cannot clear retained ownership',async t=>{
  const f=fixture(t);await f.arm();f.handlers.stream_input_revoke=async()=>({...revoked(),game_id:id(99)});
  await f.forwarder.revoke();assert.equal(f.statuses.at(-1).state,'cleanup_unknown');await assert.rejects(f.arm());
});
test('late frame reply cannot overwrite confirmed revocation',async t=>{
  const f=fixture(t);await f.arm();const frame=deferred();f.handlers.stream_input_frame=()=>frame.promise;
  f.session.dispatchEvent(new CustomEvent('input',{detail:{schema_version:1,keys:[],mouse:{dx:0,dy:0,wheel:0,buttons:0},pads:[]}}));
  await f.forwarder.revoke();frame.resolve({...grant,state:'active'});await new Promise(setImmediate);
  assert.equal(f.statuses.at(-1).state,'revoked');assert.equal(f.session.armed,false);
});
test('late authorization after close is retired without being armed',async t=>{
  const f=fixture(t),approval=deferred();f.handlers.stream_input_arm=()=>approval.promise;
  f.handlers.stream_input_revoke=async()=>({...grant,state:'cleanup_required',reason:'provider_failure'});
  const arm=f.arm();await f.forwarder.close();approval.resolve({...grant});await arm;
  assert.equal(f.session.armed,false);assert.equal(f.statuses.at(-1).state,'cleanup_required');
  assert.equal(f.calls.filter(call=>call.method==='stream_input_revoke').length,1);
});
test('an initial terminal native result cannot arm the remote input channel',async t=>{
  const f=fixture(t);f.handlers.stream_input_arm=async()=>({...grant,state:'cleanup_required',reason:'provider_failure'});
  await f.arm();assert.equal(f.session.armed,false);assert.equal(f.statuses.at(-1).state,'cleanup_required');
  await assert.rejects(f.arm());
});
