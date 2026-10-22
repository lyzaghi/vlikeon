export class ControllerNavigation extends EventTarget {
  #frame = 0;
  #lastAction = 0;
  #previousButtons = new Map();
  #visible = true;
  #gameplayActive = false;

  setGameplayActive(value) { this.#gameplayActive = Boolean(value); }

  start() {
    const update = () => this.dispatchEvent(new CustomEvent('controllers_changed', { detail: this.connected() }));
    window.addEventListener('gamepadconnected', update);
    window.addEventListener('gamepaddisconnected', update);
    document.addEventListener('visibilitychange', () => {
      this.#visible = !document.hidden;
      if (this.#visible && !this.#frame) this.#frame = requestAnimationFrame(time => this.#tick(time));
    });
    this.#frame = requestAnimationFrame(time => this.#tick(time));
    update();
  }

  connected() { return Array.from(navigator.getGamepads?.() || []).filter(Boolean).map(pad => ({ id: pad.id, index: pad.index, mapping: pad.mapping, buttons: pad.buttons.length, axes: pad.axes.length })); }

  #tick(time) {
    this.#frame = 0;
    if (!this.#visible) return;
    for (const pad of Array.from(navigator.getGamepads?.() || []).filter(Boolean)) {
      const previous = this.#previousButtons.get(pad.index) || [];
      const pressed = pad.buttons.map(button => button.pressed);
      const dialog = document.querySelector('dialog[open]');
      const scope = dialog || document;
      if (this.#gameplayActive) { this.#previousButtons.set(pad.index,pressed); continue; }
      if (pressed[0] && !previous[0]) {
        if (scope.contains(document.activeElement) && document.activeElement?.matches('button,a,input[type=checkbox]')) document.activeElement.click();
      }
      if (pressed[1] && !previous[1] && dialog) {
        if(typeof dialog.requestClose==='function')dialog.requestClose();
        else if(dialog.dispatchEvent(new Event('cancel',{cancelable:true})))dialog.close();
      }
      if (time - this.#lastAction > 190) {
        const x = (pressed[15] || pad.axes[0] > .6) ? 1 : (pressed[14] || pad.axes[0] < -.6) ? -1 : 0;
        const y = (pressed[13] || pad.axes[1] > .6) ? 1 : (pressed[12] || pad.axes[1] < -.6) ? -1 : 0;
        if (x || y) { this.#move(scope,x,y); this.#lastAction = time; }
      }
      this.#previousButtons.set(pad.index,pressed);
    }
    this.#frame = requestAnimationFrame(next => this.#tick(next));
  }

  #move(scope,x,y) {
    const candidates = Array.from(scope.querySelectorAll('button:not(:disabled),a[href],input:not(:disabled),select:not(:disabled),[tabindex="0"]')).filter(node => node.getClientRects().length && !node.closest('[hidden]'));
    if (!candidates.length) return;
    const active = document.activeElement;
    if (!candidates.includes(active)) { candidates[0].focus(); return; }
    const box = active.getBoundingClientRect();
    const center = [box.x + box.width/2,box.y + box.height/2];
    const ranked = candidates.filter(node => node !== active).map(node => {
      const rect = node.getBoundingClientRect();
      const dx = rect.x+rect.width/2-center[0], dy=rect.y+rect.height/2-center[1];
      const forward = x ? dx*x : dy*y;
      const perpendicular = x ? Math.abs(dy) : Math.abs(dx);
      return { node, score: forward > 4 ? forward + perpendicular*3 : Infinity };
    }).sort((a,b)=>a.score-b.score);
    if (Number.isFinite(ranked[0]?.score)) ranked[0].node.focus({preventScroll:false});
  }
}

export async function probeDevice() {
  const report = { webassembly: typeof WebAssembly==='object', threads: globalThis.crossOriginIsolated===true && typeof SharedArrayBuffer==='function', webgpu: false, webgl2: false, hardwareConcurrency: navigator.hardwareConcurrency ?? null, gamepad: typeof navigator.getGamepads==='function' };
  try { const adapter = await navigator.gpu?.requestAdapter({powerPreference:'low-power'}); report.webgpu=Boolean(adapter); } catch {}
  const canvas = document.createElement('canvas');
  const context = canvas.getContext('webgl2');
  report.webgl2=Boolean(context);
  context?.getExtension('WEBGL_lose_context')?.loseContext();
  return report;
}
