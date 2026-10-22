// Authored for the final consolidated pass; not executed during implementation.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';
const source=await readFile(new URL('../../apps/game-hub/component-ownership.js',import.meta.url),'utf8');
const {DocumentOwnership,creationMethods}=await import('data:text/javascript;base64,'+Buffer.from(source).toString('base64'));
const id='bdc32be6-d5e1-4e81-ac27-b4e71e6db10e';
const session='c122090f-8d2a-4991-80c4-3a34f9dcf8cc';
const other='b25dda1d-33b7-49c1-9a6a-29035775bf77';
const owner=()=>({native:new Set([session]),web:new Set([other]),preparations:new Map([[id,null]]),
  inputs:new Map([[id,session]]),mediaJobs:new Set([id]),mediaSets:new Map([[other,id]])});

test('transient document handles do not cross Game Hub tabs',()=>{
  const boundary=new DocumentOwnership(),first={},second={};boundary.open(first);boundary.open(second);
  assert.equal(boundary.permits(first,'session_pause',{session_id:session},owner()),true);
  assert.equal(boundary.permits(second,'session_pause',{session_id:session},{}),false);
  assert.equal(boundary.permits(second,'web_session_heartbeat',{session_id:other},{}),false);
  assert.equal(boundary.permits(second,'session_prepare_cancel',{job_id:id},{}),false);
  assert.equal(boundary.permits(second,'stream_input_frame',{grant_id:id,session_id:session},{}),false);
  assert.equal(boundary.permits(second,'stream_input_frame',{},{}),false);
  assert.equal(boundary.permits(first,'stream_input_frame',{grant_id:id,session_id:other},owner()),false);
  assert.equal(boundary.permits(second,'asset_range_read',{set_id:other},{}),false);
  assert.equal(boundary.permits(first,'shutdown',{},owner()),false);
});

test('selected grants are document- and direction-bound and consumed on success',()=>{
  const boundary=new DocumentOwnership(),first={},second={};boundary.open(first);boundary.open(second);
  boundary.accept(first,'file_select_import',{}, {grant_id:id},true);
  assert.equal(boundary.permits(first,'save_import',{grant_id:id},owner()),true);
  assert.equal(boundary.permits(second,'save_import',{grant_id:id},owner()),false);
  assert.equal(boundary.permits(first,'save_export',{destination_grant_id:id},owner()),false);
  boundary.accept(first,'save_import',{grant_id:id},{},true);
  assert.equal(boundary.permits(first,'save_import',{grant_id:id},owner()),false);
});

test('transfer IDs cannot be reused for the wrong direction or by another document',()=>{
  const boundary=new DocumentOwnership(),first={},second={};boundary.open(first);boundary.open(second);
  boundary.accept(first,'asset_read_start',{}, {transfer_id:id},true);
  assert.equal(boundary.permits(first,'transfer_read',{transfer_id:id},owner()),true);
  assert.equal(boundary.permits(first,'save_upload_chunk',{transfer_id:id},owner()),false);
  assert.equal(boundary.permits(second,'transfer_abort',{transfer_id:id},owner()),false);
  boundary.accept(first,'transfer_read',{transfer_id:id},{finished:true},true);
  assert.equal(boundary.permits(first,'transfer_read',{transfer_id:id},owner()),false);
});

test('late successful creation returns cleanup instead of granting a closed document authority',()=>{
  const boundary=new DocumentOwnership(),page={};boundary.open(page);boundary.close(page);
  assert.deepEqual(boundary.accept(page,'file_select_export',{}, {grant_id:id},false),
    [{method:'permission_revoke',params:{grant_id:id}}]);
  assert.deepEqual(boundary.accept(page,'save_read_start',{}, {transfer:{transfer_id:id}},false),
    [{method:'transfer_abort',params:{transfer_id:id}}]);
  assert.deepEqual(boundary.accept(page,'library_import_start',{}, {id},false),
    [{method:'library_import_cancel',params:{job_id:id}}]);
});

test('document close retires its grants, transfers and jobs once',()=>{
  const boundary=new DocumentOwnership(),page={};boundary.open(page);
  boundary.accept(page,'disc_select_folder',{}, {grant_id:id},true);
  boundary.accept(page,'save_upload_start',{}, {transfer_id:other},true);
  boundary.accept(page,'library_import_start',{}, {id:session},true);
  assert.deepEqual(boundary.close(page),[
    {method:'permission_revoke',params:{grant_id:id}},
    {method:'transfer_abort',params:{transfer_id:other}},
    {method:'library_import_cancel',params:{job_id:session}}]);
  assert.deepEqual(boundary.close(page),[]);
});

test('native disconnect invalidates retained handles while keeping page admission',()=>{
  const boundary=new DocumentOwnership(),page={};boundary.open(page);
  boundary.accept(page,'file_select_import',{}, {grant_id:id},true);boundary.reset();
  assert.equal(boundary.permits(page,'permission_revoke',{grant_id:id},owner()),false);
  assert.equal(boundary.permits(page,'hello',{},owner()),true);
  for(const method of ['file_select_import','file_select_export','save_upload_start','library_import_start'])assert.equal(creationMethods.has(method),true);
});

test('native maintenance jobs belong to their initiating document and late starts are cancelled',()=>{
  const boundary=new DocumentOwnership(),first={},second={};boundary.open(first);boundary.open(second);
  boundary.accept(first,'native_work_recover_start',{}, {id,state:'running'},true);
  assert.equal(boundary.permits(first,'native_work_recover_status',{job_id:id},owner()),true);
  assert.equal(boundary.permits(second,'native_work_recover_cancel',{job_id:id},owner()),false);
  boundary.accept(first,'native_work_recover_status',{job_id:id},{id,state:'complete'},false);
  assert.equal(boundary.permits(first,'native_work_recover_status',{job_id:id},owner()),true);
  boundary.accept(first,'native_work_recover_status',{job_id:id},{id,state:'complete'},true);
  assert.equal(boundary.permits(first,'native_work_recover_status',{job_id:id},owner()),false);
  boundary.close(second);
  assert.deepEqual(boundary.accept(second,'native_work_recover_start',{}, {id:other},false),
    [{method:'native_work_recover_cancel',params:{job_id:other}}]);
  boundary.accept(first,'native_work_recover_start',{}, {id:session,state:'running'},true);
  assert.deepEqual(boundary.close(first),[{method:'native_work_recover_cancel',params:{job_id:session}}]);
});
