import test from 'node:test';
import assert from 'node:assert/strict';
import {readFile} from 'node:fs/promises';

function installMinimalDom() {
  const makeNode = (tag) => {
    const listeners = new Map();
    const attrs = new Map();
    const el = {
      tagName: String(tag).toUpperCase(),
      className: '',
      textContent: '',
      type: '',
      hidden: false,
      disabled: false,
      children: [],
      setAttribute(k, v) { attrs.set(k, String(v)); },
      getAttribute(k) { return attrs.get(k) ?? null; },
      addEventListener(type, fn) {
        if (!listeners.has(type)) listeners.set(type, []);
        listeners.get(type).push(fn);
      },
      dispatchEvent(type) {
        for (const fn of listeners.get(type) ?? []) fn({type});
      },
      click() {
        if (!el.disabled) el.dispatchEvent('click');
      },
      append(...items) { el.children.push(...items); },
      replaceChildren(...items) { el.children = [...items]; },
    };
    return el;
  };
  globalThis.document = {
    hidden: false,
    createElement: makeNode,
    addEventListener() {},
  };
  globalThis.window = {
    addEventListener() {},
  };
  return makeNode;
}

const makeNode = installMinimalDom();
const maintSrc = await readFile(new URL('../../apps/game-hub/native-work-maintenance.js', import.meta.url), 'utf8');
const bundledSrc = maintSrc.replace(
  "import {currentLocale} from './i18n.js';",
  "const currentLocale = () => 'en';"
);
const {NativeWorkMaintenancePanel} = await import('data:text/javascript;base64,' + Buffer.from(bundledSrc).toString('base64'));

function makeStatus(overrides = {}) {
  return {
    schema_version: 1,
    works: 2,
    reserved_works: 1,
    staged_works: 1,
    retiring_works: 0,
    reserved_bytes: 16 * 1024 * 1024,
    reserved_files: 4,
    maximum_works: 64,
    maximum_bytes: 32 * 1024 ** 3,
    maximum_files: 4096,
    physical_bytes: 12 * 1024 * 1024,
    recovery_running: false,
    ...overrides,
  };
}

test('NativeWorkMaintenancePanel accepts both physical_bytes integer and null and rejects negative/out-of-range values', async () => {
  const notifications = [];
  const calls = [];
  let nextStatus = makeStatus({physical_bytes: 12 * 1024 * 1024});
  const bridge = {
    addEventListener() {},
    async request(method, params) {
      calls.push({method, params});
      if (method === 'native_work_status') return nextStatus;
      throw new Error('unexpected_method:' + method);
    },
  };
  const root = makeNode('div');
  const panel = new NativeWorkMaintenancePanel(bridge, root, (msg, err) => notifications.push({msg, err}));
  panel.setAvailable(true);

  await panel.refresh();
  assert.equal(notifications.length, 0);
  const section = root.children[0];
  const storageText = section.children[2].textContent;
  assert.match(storageText, /16\.0 MiB reserved/);
  assert.match(storageText, /12\.0 MiB on disk/);

  // Legacy null physical_bytes remains supported
  nextStatus = makeStatus({physical_bytes: null});
  await panel.refresh();
  assert.equal(notifications.length, 0);
  assert.match(section.children[2].textContent, /not a measurement of actual disk allocation/);

  // Invalid negative physical_bytes is rejected fail-closed
  nextStatus = makeStatus({physical_bytes: -1});
  await panel.refresh();
  assert.equal(notifications.length, 1);
  assert.equal(notifications[0].err, true);
});

test('NativeWorkMaintenancePanel quarantines retained legacy/incomplete entries via native_work_quarantine_legacy', async () => {
  const notifications = [];
  const calls = [];
  let currentWorks = 2;
  const jobId = '4a58f79b-2d2e-4f7c-8a53-68f0b1817b21';
  const bridge = {
    addEventListener() {},
    async request(method, params) {
      calls.push({method, params});
      if (method === 'native_work_status') {
        return makeStatus({
          works: currentWorks,
          reserved_works: currentWorks,
          staged_works: 0,
          physical_bytes: currentWorks * 4096,
        });
      }
      if (method === 'native_work_recover_start') {
        return {
          id: jobId,
          state: 'complete',
          started_at_ms: 100,
          finished_at_ms: 150,
          summary: {
            attempted: 2,
            recovered: 0,
            retained: 2,
            retention: {
              locked: 0,
              legacy_ownership: 1,
              incomplete_ownership: 1,
              other_platform: 0,
              unconfirmed: 0,
            },
            unknown_entries: 0,
            cancelled: false,
            more_available: false,
          },
        };
      }
      if (method === 'native_work_quarantine_legacy') {
        currentWorks = 0;
        return {
          schema_version: 1,
          attempted: 2,
          quarantined: 2,
          reclaimed_clean: 0,
          skipped_ineligible: 0,
          preserved_files: 3,
          preserved_bytes: 2048,
          more_available: false,
        };
      }
      throw new Error('unexpected_method:' + method);
    },
  };
  const root = makeNode('div');
  const panel = new NativeWorkMaintenancePanel(bridge, root, (msg, err) => notifications.push({msg, err}));
  panel.setAvailable(true);
  await panel.refresh();

  const section = root.children[0];
  const actions = section.children[4];
  const [startBtn, , quarantineBtn] = actions.children;
  assert.equal(quarantineBtn.hidden, true);
  assert.equal(quarantineBtn.disabled, true);

  startBtn.click();
  await new Promise((r) => setTimeout(r, 20));
  assert.equal(quarantineBtn.hidden, false);
  assert.equal(quarantineBtn.disabled, false);

  quarantineBtn.click();
  await new Promise((r) => setTimeout(r, 20));

  assert.equal(notifications.length, 0);
  assert.equal(calls.some((c) => c.method === 'native_work_quarantine_legacy' && c.params === undefined), true);
  assert.match(section.children[3].textContent, /Quarantined 2 entries \(3 files, 2\.0 KiB\)/);
  assert.match(section.children[2].textContent, /0 retained sessions/);
});
