// Browser/IndexedDB integration, executed only by the assigned QA runner.
const fs=require('node:fs'),path=require('node:path'),http=require('node:http'),assert=require('node:assert/strict');
const {chromium}=require(process.env.VLIKEON_PLAYWRIGHT_MODULE);
const root=path.resolve(process.env.VLIKEON_WEB_BUNDLE??'out/web'),output=process.env.VLIKEON_QA_OUTPUT;
const server=http.createServer((req,res)=>{
  if(req.url==='/test.html'){res.setHeader('Content-Type','text/html; charset=utf-8');res.end('<!doctype html><html lang="en"><title>Manual recovery fixture</title><link rel="stylesheet" href="/styles.css"><body><div id="recovery"></div></body></html>');return;}
  const name=decodeURIComponent(req.url.split('?')[0]),file=path.resolve(root,'.'+name);
  if(!file.startsWith(root+path.sep)||!fs.existsSync(file)||!fs.statSync(file).isFile()){res.writeHead(404);res.end();return;}
  res.setHeader('Content-Type',file.endsWith('.js')?'text/javascript; charset=utf-8':file.endsWith('.css')?'text/css; charset=utf-8':'application/octet-stream');fs.createReadStream(file).pipe(res);
});
(async()=>{
  await new Promise(resolve=>server.listen(0,'127.0.0.1',resolve));
  let browser;
  try{
    browser=await chromium.launch({executablePath:process.env.VLIKEON_CHROME,headless:true});
    const page=await browser.newPage({viewport:{width:1000,height:900}}),errors=[];
    page.on('pageerror',error=>errors.push(error.message));
    await page.goto(`http://127.0.0.1:${server.address().port}/test.html`);
    await page.evaluate(async()=>{
      const {openStore}=await import('/web/idb.js'),{WebManualRecovery}=await import('/web/manual-recovery.js'),{ManualRecoveryPanel}=await import('/manual-recovery.js'),{setLocale}=await import('/i18n.js');
      setLocale('en');const store=await openStore();
      await store.transaction(['saves','assets'],'readwrite',async tx=>{
        await tx.add('saves',{id:'lost-id-no-valid-metadata',payload:new Blob(['LOST PROGRESS'])});
        await tx.add('assets',{hash:'unknown-name',blob:new Blob(['LOST CONTENT'])});
      });
      const service=new WebManualRecovery(store);window.fixture={store,service,notifications:[],calls:[],exports:[],extracted:[],grants:new Set(),cancel:false,closed:false};
      const bridge={async request(method,p){
        fixture.calls.push({method,p});
        if(method==='hello')return{manual_recovery_protocol:1};
        if(method==='manual_recovery_list')return service.list(p.after);
        if(method==='manual_recovery_inspect')return service.inspect(p.candidate_id);
        if(method==='file_select_export'){if(fixture.cancel)throw Object.assign(new Error('User cancelled'),{code:'cancelled'});const grant_id=crypto.randomUUID();fixture.grants.add(grant_id);return{grant_id};}
        if(method==='file_select_import'){const grant_id=crypto.randomUUID();fixture.grants.add(grant_id);return{grant_id};}
        if(method==='manual_recovery_export'){if(!fixture.grants.delete(p.destination_grant_id))throw Error('missing grant');const result=await service.export(p.candidate_id,p.revision);fixture.exports.push(result);return{record:result.record,exported:true,delivery:'browser_download_started'};}
        if(method==='manual_recovery_extract'){if(!fixture.grants.delete(p.grant_id)||!fixture.grants.delete(p.destination_grant_id))throw Error('missing extraction grant');const {unpackRecovery}=await import('/manual-recovery-format.js'),result=await unpackRecovery(fixture.exports[0].blob);fixture.extracted.push(await result.payload.text());return{record:result.record,exported:true,delivery:'browser_download_started'};}
        if(method==='permission_revoke'){fixture.grants.delete(p.grant_id);return{revoked:true};}
        throw Error('Unexpected method '+method);
      }};
      new ManualRecoveryPanel(bridge,document.querySelector('#recovery'),(message,error)=>fixture.notifications.push({message,error:Boolean(error)}));
    });
    await page.getByRole('heading',{name:'Manual data recovery',exact:true}).waitFor();
    await page.getByRole('button',{name:'Inspect retained data',exact:true}).click();
    await page.getByRole('button',{name:'Inspect this copy',exact:true}).first().waitFor();
    assert.equal(await page.getByRole('button',{name:'Inspect this copy',exact:true}).count(),2);
    await page.getByRole('button',{name:'Inspect this copy',exact:true}).first().click();
    await page.getByRole('button',{name:'Export recovery package',exact:true}).waitFor();
    await page.screenshot({path:path.join(output,'manual-recovery-review.png'),fullPage:true});
    await page.setViewportSize({width:360,height:800});
    assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth<=innerWidth),true);
    await page.screenshot({path:path.join(output,'manual-recovery-mobile.png'),fullPage:true});
    await page.setViewportSize({width:1000,height:900});
    await page.getByRole('button',{name:'Export recovery package',exact:true}).click();
    await page.waitForFunction(()=>fixture.notifications.length===1);
    const exported=await page.evaluate(async()=>{const {unpackRecovery}=await import('/manual-recovery-format.js');const result=await unpackRecovery(fixture.exports[0].blob);return{record:result.record,payload:await result.payload.text(),grants:fixture.grants.size,notification:fixture.notifications[0]};});
    assert.equal(exported.record.trust,'unverified');assert.ok(['LOST PROGRESS','LOST CONTENT'].includes(exported.payload));assert.equal(exported.grants,0);assert.equal(exported.notification.error,false);
    await page.getByRole('button',{name:'Extract a recovery package',exact:true}).click();
    await page.waitForFunction(()=>fixture.notifications.length===2);
    assert.deepEqual(await page.evaluate(()=>fixture.extracted),[exported.payload]);
    assert.equal(await page.evaluate(()=>fixture.grants.size),0);
    // Stale selection must fail before a visible download is produced.
    await page.evaluate(async()=>{await fixture.store.transaction(['assets','saves'],'readwrite',async tx=>{await tx.put('assets',{hash:'unknown-name',blob:new Blob(['NEW CONTENT'])});await tx.put('saves',{id:'lost-id-no-valid-metadata',payload:new Blob(['NEW PROGRESS'])});});});
    await page.getByRole('button',{name:'Export recovery package',exact:true}).click();
    await page.waitForFunction(()=>fixture.notifications.length===3);
    assert.equal(await page.evaluate(()=>fixture.exports.length),1);assert.equal(await page.evaluate(()=>fixture.grants.size),0);
    assert.equal(await page.evaluate(()=>fixture.notifications.at(-1).error),true);
    // Cancellation revokes any handle and leaves the source records intact.
    await page.getByRole('button',{name:'Inspect this copy',exact:true}).first().click();
    await page.getByRole('button',{name:'Export recovery package',exact:true}).waitFor();
    await page.evaluate(()=>fixture.cancel=true);
    await page.getByRole('button',{name:'Export recovery package',exact:true}).click();
    await page.waitForFunction(()=>fixture.notifications.length===4);
    assert.equal(await page.evaluate(()=>fixture.grants.size),0);
    const final=await page.evaluate(async()=>({records:await fixture.store.transaction(['saves'],'readonly',tx=>tx.all('saves')),calls:fixture.calls.map(call=>call.method)}));
    assert.equal(final.records.length,1);assert.equal(final.records[0].id,'lost-id-no-valid-metadata');
    assert.ok(!final.calls.some(method=>['save_upload_commit','save_promote','library_import'].includes(method)));
    await page.screenshot({path:path.join(output,'manual-recovery-ui.png'),fullPage:true});
    assert.deepEqual(errors,[]);
    fs.writeFileSync(path.join(output,'browser-integration.json'),JSON.stringify({status:'passed',browser:await browser.version(),real_indexed_db:true,picker:'controlled bridge fixture',cases:['missing_metadata','review','package_export','package_extraction','changed_source','cancelled_export','grant_revoke','no_publication','360px_layout'],errors},null,2));
  }finally{await browser?.close();await new Promise(resolve=>server.close(resolve));}
})().catch(error=>{console.error(error);process.exitCode=1;});
