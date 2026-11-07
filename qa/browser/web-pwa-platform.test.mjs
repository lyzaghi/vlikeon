import assert from 'node:assert/strict';
import crypto from 'node:crypto';
import test from 'node:test';

import { AudioResampler } from '../../apps/web/platform/audio-resampler.js';
import { WebGamepadManager } from '../../apps/web/platform/web-gamepad.js';
import { QuotaManager } from '../../apps/web/platform/quota-manager.js';
import { PwaLifecycleManager } from '../../apps/web/platform/pwa-install.js';
import { PresentationTimingCollector, DropReason } from '../../apps/web/platform/presentation-timing.js';

test('AudioResampler resamples interleaved stereo across chunk boundaries without phase drift', () => {
  const resampler = new AudioResampler(44100, 48000, 2);
  const chunk1 = new Float32Array([0.0, 0.0, 0.5, -0.5, 1.0, -1.0, 0.25, -0.25]);
  const out1 = resampler.resampleInterleaved(chunk1);
  assert.ok(out1.length > 0);
  assert.equal(out1.length % 2, 0);

  const chunk2 = new Float32Array([0.1, -0.1, -0.2, 0.2, -0.8, 0.8, 0.0, 0.0]);
  const out2 = resampler.resampleInterleaved(chunk2);
  assert.ok(out2.length > 0);
  assert.equal(out2.length % 2, 0);

  resampler.reset();
  assert.equal(resampler.phase, 0);
  assert.equal(resampler.lastFrame, null);
});

test('WebGamepadManager emits 64-byte binary reports, monotonic sequences, and neutral fence on stop', () => {
  const reports = [];
  const listeners = new Map();
  globalThis.window = {
    addEventListener(type, fn) {
      listeners.set(type, fn);
    },
    removeEventListener(type) {
      listeners.delete(type);
    },
  };
  globalThis.requestAnimationFrame = () => 1;
  globalThis.cancelAnimationFrame = () => {};

  const mgr = new WebGamepadManager((buf) => reports.push(buf));
  mgr.start();
  assert.equal(reports.length, 1, 'start() must emit an initial neutral report');

  mgr.dispatchGamepadState(
    {
      index: 0,
      connected: true,
      buttons: [
        { pressed: true },  // A (bit 0)
        { pressed: false }, // B
        { pressed: false },
        { pressed: false },
        { pressed: true },  // L1 (bit 4)
        { pressed: false },
        { pressed: true, value: 0.5 }, // LT
        { pressed: true, value: 1.0 }, // RT
      ],
      axes: [0.5, -0.5, 0.25, -0.25],
    },
    100.0
  );
  assert.equal(reports.length, 2);
  const view = new DataView(reports[1]);
  assert.equal(reports[1].byteLength, 64);
  assert.equal(view.getUint8(0), 0x56);
  assert.equal(view.getUint8(1), 1);
  assert.equal(view.getUint32(4, true), 2);
  assert.equal(view.getUint32(8, true), (1 << 0) | (1 << 4));
  assert.equal(view.getInt16(12, true), 16384);
  assert.equal(view.getInt16(14, true), -16383);
  assert.equal(view.getUint8(20), 128);
  assert.equal(view.getUint8(21), 255);

  mgr.stop();
  assert.equal(reports.length, 3, 'stop() must emit a neutral release fence report');
  const neutralView = new DataView(reports[2]);
  assert.equal(neutralView.getUint32(8, true), 0);
  assert.equal(neutralView.getInt16(12, true), 0);
});

test('QuotaManager evaluates low storage thresholds and verifies chunked Blob SHA-256 checksums', async () => {
  const payload = new Uint8Array(150 * 1024);
  for (let i = 0; i < payload.length; i++) {
    payload[i] = (i * 31 + 7) & 0xff;
  }
  const expectedHex = crypto.createHash('sha256').update(payload).digest('hex');
  const blob = new Blob([payload]);

  const ok = await QuotaManager.verifyBlobChecksum(blob, expectedHex, 16 * 1024);
  assert.equal(ok, true);

  const mismatch = await QuotaManager.verifyBlobChecksum(blob, '0'.repeat(64), 16 * 1024);
  assert.equal(mismatch, false);

  const origStorage = globalThis.navigator?.storage;
  Object.defineProperty(globalThis.navigator, 'storage', {
    configurable: true,
    value: {
      async estimate() {
        return { usage: 850 * 1024 * 1024, quota: 1000 * 1024 * 1024 };
      },
      async persisted() {
        return false;
      },
      async persist() {
        return true;
      },
    },
  });
  try {
    const est = await QuotaManager.getStorageEstimate();
    assert.equal(est.supported, true);
    assert.equal(est.percent, 85);
    assert.equal(est.isLowStorage, true);
    const persisted = await QuotaManager.ensurePersistentStorage();
    assert.equal(persisted, true);
  } finally {
    Object.defineProperty(globalThis.navigator, 'storage', {
      configurable: true,
      value: origStorage,
    });
  }
});

test('PwaLifecycleManager defers update reload while gameSessionActive is true', async () => {
  let reloaded = false;
  globalThis.window = {
    addEventListener() {},
    removeEventListener() {},
    location: {
      reload() {
        reloaded = true;
      },
    },
  };
  const pwa = new PwaLifecycleManager();
  pwa.updateWaiting = true;
  pwa.waitingWorker = {};
  pwa.setGameSessionActive(true);
  pwa.applyPendingUpdate();
  assert.equal(reloaded, false, 'Must not reload while gameplay session is active');

  pwa.setGameSessionActive(false);
  pwa.applyPendingUpdate();
  assert.equal(reloaded, true, 'Should reload once gameplay session is quiescent');
});

test('PresentationTimingCollector enforces maxQueueDepth=2 and computes render-to-present latency/jitter', () => {
  const collector = new PresentationTimingCollector('webgl_canvas', 60);
  assert.equal(collector.recordFrameDispatch(1, 10_000_000), true);
  assert.equal(collector.recordFrameDispatch(2, 26_000_000), true);
  assert.equal(collector.queueDepth, 2);
  // 3rd dispatch before present must drop with QUEUE_OVERFLOW
  assert.equal(collector.recordFrameDispatch(3, 42_000_000), false);
  assert.equal(collector.queueDepth, 2);

  collector.recordFramePresented(1, 10_000_000, 14_000_000, 16_000_000);
  collector.recordFramePresented(2, 26_000_000, 31_000_000, 33_000_000);
  assert.equal(collector.queueDepth, 0);

  const report = collector.getMetricsReport('session-web-1');
  assert.equal(report.session_id, 'session-web-1');
  assert.equal(report.frames_emulated, 3);
  assert.equal(report.frames_presented, 2);
  assert.equal(report.frames_dropped, 1);
  assert.ok(report.render_to_present_p50_ms >= 4.0);
});
