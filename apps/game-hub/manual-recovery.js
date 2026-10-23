import {currentLocale, exportMessage} from './i18n.js';
import {RECOVERY_MAX_BYTES, recoveryRecord} from './manual-recovery-format.js';

const text = (vi, en) => currentLocale() === 'vi' ? vi : en;
const hash = value => typeof value === 'string' && /^[a-f0-9]{64}$/.test(value);
const node = (tag, label) => { const item = document.createElement(tag); if (label !== undefined) item.textContent = label; return item; };
const scopes = {assets: ['Kho game', 'Game content'], legacy_saves: ['Save cũ', 'Legacy save files'],
  repair_files: ['Kho payload', 'Payload files'], web_assets: ['Kho game Web', 'Web game content'], web_saves: ['Kho save Web', 'Web save data']};

export class ManualRecoveryPanel {
  #bridge; #root; #notify; #page = null; #review = null; #busy = false; #closed = false; #abort = new AbortController();
  constructor(bridge, root, notify) {
    this.#bridge = bridge; this.#root = root; this.#notify = notify; root.hidden = true;
    window.addEventListener('pagehide', () => { this.#closed = true; this.#abort.abort(); }, {once: true, signal: this.#abort.signal});
    document.addEventListener('omniplay_locale_changed', () => this.#render(), {signal: this.#abort.signal});
    void bridge.request('hello').then(value => { if (!this.#closed && value.manual_recovery_protocol === 1) { root.hidden = false; this.#render(); } }).catch(() => {});
  }
  #action(label, body, disabled = false) {
    const button = node('button', label); button.type = 'button'; button.className = 'button button-secondary';
    button.disabled = this.#busy || disabled; button.addEventListener('click', () => void this.#run(body)); return button;
  }
  #render() {
    if (this.#closed) return;
    const section = node('section'); section.className = 'panel manual-recovery'; section.setAttribute('aria-busy', String(this.#busy));
    section.append(node('h2', text('Phục hồi thủ công', 'Manual data recovery')),
      node('p', text('Cứu bản sao dữ liệu khi thông tin game hoặc save bị mất. Kiểm tra bytes trước khi xuất gói .oprecovery. Bản gốc được giữ nguyên; checksum xác nhận bytes, không chứng minh game, độ tương thích hay nguồn gốc. Gói này không tự thêm dữ liệu vào thư viện hoặc chọn save để chạy.',
        'Preserve a copy when game or save details are missing. Inspect the bytes before exporting an .oprecovery package. Originals stay intact. Checksums verify bytes, not game identity, compatibility or provenance. This package does not add library entries or select a save to run.')));
    const actions = node('div'); actions.className = 'manual-recovery-actions';
    actions.append(this.#action(text('Xem dữ liệu còn giữ', 'Inspect retained data'), () => this.#load(null)),
      this.#action(text('Trích xuất gói phục hồi', 'Extract a recovery package'), () => this.#extract()));
    section.append(actions);
    if (this.#busy) section.append(node('p', text('Đang đọc và kiểm tra dữ liệu…', 'Reading and checking data…')));
    if (this.#page) {
      section.append(node('p', text(`${this.#page.candidates.length} tệp trong trang; ${this.#page.retained_skipped} mục không đọc được theo định dạng này vẫn được giữ. Có thể gồm dữ liệu đang được sử dụng.`,
        `${this.#page.candidates.length} files on this page; ${this.#page.retained_skipped} unsupported entries remain retained. May include data still in use.`)));
      for (const candidate of this.#page.candidates) {
        const row = node('div'); row.className = 'save-item';
        row.append(node('p', `${text(...scopes[candidate.source_kind])} · ${candidate.display_name} · ${candidate.bytes.toLocaleString()} bytes`),
          this.#action(text('Kiểm tra bản này', 'Inspect this copy'), async () => {
            this.#review = null;
            const value = await this.#bridge.request('manual_recovery_inspect', {candidate_id: candidate.candidate_id}, 120000);
            if (this.#closed) return;
            recoveryRecord(value?.record);
            if (value.candidate_id !== candidate.candidate_id || !hash(value.revision)) throw new Error('Invalid recovery review.');
            this.#review = value;
          }, !candidate.inspectable));
        if (!candidate.inspectable) row.append(node('p', text('Vượt giới hạn 4 GiB của công cụ; tệp được giữ nguyên.', 'Exceeds the tool’s 4 GiB limit; file remains intact.')));
        section.append(row);
      }
      if (this.#page.next_after) section.append(this.#action(text('Trang tiếp theo', 'Next page'), () => this.#load(this.#page.next_after)));
    }
    if (this.#review) {
      const review = this.#review, record = review.record, details = node('div'); details.className = 'save-item manual-recovery-details';
      details.append(node('h3', text('Bản sao chưa xác định danh tính', 'Copy with unverified identity')),
        node('p', `${record.display_name} · ${record.payload_bytes.toLocaleString()} bytes`),
        node('p', `SHA-256: ${record.payload_hash}`), node('p', `${text('32 byte đầu (hex)', 'First 32 bytes (hex)')}: ${record.prefix_hex || '∅'}`),
        this.#action(text('Xuất gói phục hồi', 'Export recovery package'), () => this.#export(review)));
      section.append(details);
    }
    this.#root.replaceChildren(section);
  }
  async #load(after) {
    this.#review = null;
    const value = await this.#bridge.request('manual_recovery_list', {after}, 30000);
    if (this.#closed) return;
    if (!Array.isArray(value?.candidates) || value.candidates.length > 64 || !Number.isSafeInteger(value.retained_skipped) || value.retained_skipped < 0 || value.next_after !== null && !hash(value.next_after)) throw new Error('Invalid recovery inventory.');
    let last = after;
    for (const candidate of value.candidates) {
      if (!hash(candidate.candidate_id) || last !== null && candidate.candidate_id <= last || !Object.hasOwn(scopes, candidate.source_kind) ||
          typeof candidate.display_name !== 'string' || candidate.display_name.length > 240 || !Number.isSafeInteger(candidate.bytes) || candidate.bytes < 0 ||
          typeof candidate.inspectable !== 'boolean' || candidate.inspectable !== (candidate.bytes <= RECOVERY_MAX_BYTES)) throw new Error('Invalid recovery candidate.');
      last = candidate.candidate_id;
    }
    if (value.next_after !== null && (value.candidates.length !== 64 || value.next_after !== last)) throw new Error('Invalid recovery cursor.');
    this.#page = value;
  }
  async #export(review) {
    const grant = await this.#bridge.request('file_select_export', {suggested_name: `recovery-${review.record.payload_hash.slice(0, 24)}.oprecovery`}, 120000);
    try {
      if (this.#closed) return;
      const value = await this.#bridge.request('manual_recovery_export', {candidate_id: review.candidate_id, revision: review.revision, destination_grant_id: grant.grant_id}, 120000);
      if (this.#closed) return;
      recoveryRecord(value?.record);
      if (value.exported !== true || JSON.stringify(value.record) !== JSON.stringify(review.record)) throw new Error('Invalid recovery export response.');
      this.#notify(exportMessage(value));
    } finally { await this.#bridge.request('permission_revoke', {grant_id: grant.grant_id}).catch(() => {}); }
  }
  async #extract() {
    const source = await this.#bridge.request('file_select_import', undefined, 120000);
    let destination;
    try {
      if (this.#closed) return;
      destination = await this.#bridge.request('file_select_export', {suggested_name: 'recovered-unverified.bin'}, 120000);
      if (this.#closed) return;
      const value = await this.#bridge.request('manual_recovery_extract', {grant_id: source.grant_id, destination_grant_id: destination.grant_id}, 120000);
      if (this.#closed) return;
      recoveryRecord(value?.record);
      if (value.exported !== true) throw new Error('Invalid extraction response.');
      this.#notify(exportMessage(value) + ' ' + text('Bản trích xuất cần được xác định game/định dạng trước khi dùng. ', 'Identify the game and format before using the extracted copy. ') + `SHA-256: ${value.record.payload_hash}`);
    } finally {
      for (const grant of [source, destination].filter(Boolean)) await this.#bridge.request('permission_revoke', {grant_id: grant.grant_id}).catch(() => {});
    }
  }
  async #run(body) {
    if (this.#busy || this.#closed) return;
    this.#busy = true; this.#render();
    try { await body(); }
    catch (error) { if (!this.#closed) { this.#review = null; this.#notify(error.message || text('Chưa phục hồi được; bản gốc vẫn giữ nguyên.', 'Recovery did not finish; originals remain intact.'), true); } }
    finally { this.#busy = false; this.#render(); }
  }
}
