/**
 * Web Gamepad API Bridge for OmniPlay PWA.
 * Intercepts navigator.getGamepads() in browsers, handles standard gamepad mapping,
 * produces 64-byte bounded StreamGamepadReport structures with monotonic sequence numbers,
 * and maintains 250ms gap checks and neutral report fences on disconnect.
 */

export class WebGamepadManager {
  constructor(reportSink) {
    this.reportSink = reportSink;
    this.active = false;
    this.sequence = 1;
    this.lastReportTime = 0;
    this.pollTimer = null;
    this.connectedGamepadIndex = null;

    this.onGamepadConnected = this.onGamepadConnected.bind(this);
    this.onGamepadDisconnected = this.onGamepadDisconnected.bind(this);
    this.poll = this.poll.bind(this);
  }

  start() {
    if (this.active) return;
    this.active = true;
    this.sequence = 1;

    window.addEventListener('gamepadconnected', this.onGamepadConnected);
    window.addEventListener('gamepaddisconnected', this.onGamepadDisconnected);

    this.sendNeutralReport();
    this.pollTimer = requestAnimationFrame(this.poll);
  }

  stop() {
    if (!this.active) return;
    this.active = false;

    window.removeEventListener('gamepadconnected', this.onGamepadConnected);
    window.removeEventListener('gamepaddisconnected', this.onGamepadDisconnected);

    if (this.pollTimer) {
      cancelAnimationFrame(this.pollTimer);
      this.pollTimer = null;
    }

    this.sendNeutralReport();
    this.connectedGamepadIndex = null;
  }

  onGamepadConnected(event) {
    if (this.connectedGamepadIndex === null) {
      this.connectedGamepadIndex = event.gamepad.index;
    }
  }

  onGamepadDisconnected(event) {
    if (this.connectedGamepadIndex === event.gamepad.index) {
      this.connectedGamepadIndex = null;
      this.sendNeutralReport();
    }
  }

  poll() {
    if (!this.active) return;

    const gamepads = navigator.getGamepads ? navigator.getGamepads() : [];
    let pad = null;

    if (this.connectedGamepadIndex !== null && gamepads[this.connectedGamepadIndex]) {
      pad = gamepads[this.connectedGamepadIndex];
    } else {
      for (const p of gamepads) {
        if (p && p.connected) {
          pad = p;
          this.connectedGamepadIndex = p.index;
          break;
        }
      }
    }

    const now = performance.now();
    if (pad) {
      this.dispatchGamepadState(pad, now);
    } else if (now - this.lastReportTime >= 250) {
      // 250ms gap check heartbeat
      this.sendNeutralReport();
    }

    this.pollTimer = requestAnimationFrame(this.poll);
  }

  dispatchGamepadState(pad, now) {
    let buttons = 0;
    // Standard Gamepad Mapping (W3C standard mapping)
    if (pad.buttons[0]?.pressed) buttons |= (1 << 0); // A
    if (pad.buttons[1]?.pressed) buttons |= (1 << 1); // B
    if (pad.buttons[2]?.pressed) buttons |= (1 << 2); // X
    if (pad.buttons[3]?.pressed) buttons |= (1 << 3); // Y
    if (pad.buttons[4]?.pressed) buttons |= (1 << 4); // L1
    if (pad.buttons[5]?.pressed) buttons |= (1 << 5); // R1
    if (pad.buttons[8]?.pressed) buttons |= (1 << 6); // Select / Back
    if (pad.buttons[9]?.pressed) buttons |= (1 << 7); // Start
    if (pad.buttons[12]?.pressed) buttons |= (1 << 8); // Dpad Up
    if (pad.buttons[13]?.pressed) buttons |= (1 << 9); // Dpad Down
    if (pad.buttons[14]?.pressed) buttons |= (1 << 10); // Dpad Left
    if (pad.buttons[15]?.pressed) buttons |= (1 << 11); // Dpad Right
    if (pad.buttons[10]?.pressed) buttons |= (1 << 12); // L3
    if (pad.buttons[11]?.pressed) buttons |= (1 << 13); // R3

    const lx = Math.max(-32767, Math.min(32767, Math.round((pad.axes[0] || 0) * 32767)));
    const ly = Math.max(-32767, Math.min(32767, Math.round((pad.axes[1] || 0) * 32767)));
    const rx = Math.max(-32767, Math.min(32767, Math.round((pad.axes[2] || 0) * 32767)));
    const ry = Math.max(-32767, Math.min(32767, Math.round((pad.axes[3] || 0) * 32767)));

    const ltVal = pad.buttons[6]?.value !== undefined ? pad.buttons[6].value : (pad.buttons[6]?.pressed ? 1.0 : 0.0);
    const rtVal = pad.buttons[7]?.value !== undefined ? pad.buttons[7].value : (pad.buttons[7]?.pressed ? 1.0 : 0.0);
    const lt = Math.max(0, Math.min(255, Math.round(ltVal * 255)));
    const rt = Math.max(0, Math.min(255, Math.round(rtVal * 255)));

    this.sendReport(buttons, lx, ly, rx, ry, lt, rt);
    this.lastReportTime = now;
  }

  static formatRemotePadJson(seq, buttons, lx, ly, rx, ry, lt, rt) {
    return {
      sequence: seq,
      buttons: buttons >>> 0,
      left_x: Math.max(-32767, Math.min(32767, lx | 0)),
      left_y: Math.max(-32767, Math.min(32767, ly | 0)),
      right_x: Math.max(-32767, Math.min(32767, rx | 0)),
      right_y: Math.max(-32767, Math.min(32767, ry | 0)),
      left_trigger: Math.max(0, Math.min(255, lt | 0)),
      right_trigger: Math.max(0, Math.min(255, rt | 0))
    };
  }

  sendReport(buttons, lx, ly, rx, ry, lt, rt) {
    if (!this.reportSink) return;
    if (this.sequence > 10000000) {
      // Sequence exhausted; fence session instead of wrapping sequence numbers
      this.stop();
      return;
    }

    const seq = this.sequence++;
    const clampedLx = Math.max(-32767, Math.min(32767, lx | 0));
    const clampedLy = Math.max(-32767, Math.min(32767, ly | 0));
    const clampedRx = Math.max(-32767, Math.min(32767, rx | 0));
    const clampedRy = Math.max(-32767, Math.min(32767, ry | 0));

    const buffer = new ArrayBuffer(64);
    const view = new DataView(buffer);

    view.setUint8(0, 0x56); // 'V'
    view.setUint8(1, 1);    // version 1
    view.setUint16(2, 0, true);
    view.setUint32(4, seq, true);
    view.setUint32(8, buttons, true);
    view.setInt16(12, clampedLx, true);
    view.setInt16(14, clampedLy, true);
    view.setInt16(16, clampedRx, true);
    view.setInt16(18, clampedRy, true);
    view.setUint8(20, lt);
    view.setUint8(21, rt);
    view.setUint16(22, 0, true);

    const micros = Math.round(performance.now() * 1000);
    view.setBigUint64(24, BigInt(micros), true);

    this.reportSink(buffer);
  }

  sendNeutralReport() {
    this.sendReport(0, 0, 0, 0, 0, 0, 0);
  }
}
