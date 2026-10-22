// Authored for item 16. Do not execute during source implementation.
import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

const source = await readFile(new URL('../../apps/game-hub/component-context.js', import.meta.url), 'utf8');
const {isGameHubSender, gameHubContext} = await import('data:text/javascript;base64,' + Buffer.from(source).toString('base64'));
const id = 'knanejoacaeomhkfeciafoclmmnogdpg';
const documentId = 'f25e00e5-e7c6-4e04-8882-b2f608c49c11';
const url = `chrome-extension://${id}/index.html`;
const page = (sender = {}, name = 'omniplay-ui') => ({name, sender:{id, documentId, url, frameId:0, tab:{incognito:false}, ...sender}});
const context = (overrides = {}) => ({documentId, contextType:'TAB', frameId:0, incognito:false,
  windowId:4, tabId:12, documentUrl:url, ...overrides});

test('Game Hub document authority', async t => {
  let resolver = async () => [context()];
  const calls = [];
  globalThis.chrome = {runtime:{id, getContexts:async filter => {calls.push(filter); return resolver(filter);}}};

  await t.test('uses the live browser tab/window instead of renderer-supplied identities', async () => {
    const current = await gameHubContext({...page(), windowId:99, tabId:200});
    assert.equal(current.windowId,4);
    assert.equal(current.tabId,12);
    assert.deepEqual(calls.at(-1),{documentIds:[documentId],contextTypes:['TAB']});
  });

  await t.test('allows the product hash route without granting query URLs', async () => {
    assert.equal(isGameHubSender(page({url:url+'#browser'})),true);
    assert.equal(isGameHubSender(page({url:url+'?source=external'})),false);
    assert.equal(isGameHubSender(page({url:`chrome-extension://${id}/player.html`})),false);
  });

  await t.test('rejects foreign, private, subframe and missing-identity senders before querying', async () => {
    const count = calls.length;
    for (const candidate of [page({id:'other'}), page({frameId:1}), page({frameId:undefined}),
      page({documentId:undefined}), page({tab:{incognito:true}}), page({},'another-port'),
      page({url:'https://example.com/index.html'})]) {
      assert.equal(isGameHubSender(candidate),false);
      await assert.rejects(gameHubContext(candidate),{code:'permission_denied'});
    }
    assert.equal(calls.length,count);
  });

  await t.test('does not trust a stale sender after its document disappears or changes context', async () => {
    for (const invalid of [[], [context({documentId:'another'})], [context({contextType:'POPUP'})],
      [context({incognito:true})], [context({frameId:2})], [context({tabId:-1})],
      [context({windowId:NaN})], [context({documentUrl:url+'?elevate=1'})]]) {
      resolver=async()=>invalid;
      await assert.rejects(gameHubContext(page()),{code:'permission_denied'});
    }
  });

  await t.test('rechecks caller liveness after the asynchronous browser lookup', async () => {
    let resolve;
    resolver=()=>new Promise(complete=>{resolve=complete;});
    const expired=Object.assign(new Error('document closed'),{code:'deadline_exceeded'});
    const pending=gameHubContext(page(),()=>{throw expired;});
    resolve([context()]);
    await assert.rejects(pending,error=>error===expired);
  });

  delete globalThis.chrome;
});
