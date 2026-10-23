import {checkAbort, requireValue, HASH} from './errors.js';
import {Sha256} from './sha256.js';
import {RECOVERY_MAX_BYTES, recoveryLabel, recoveryRevision, inspectRecoveryBlob, packRecovery} from '../manual-recovery-format.js';

const identity = (kind, key) => new Sha256().update(new TextEncoder().encode(JSON.stringify([kind, key]))).hex();
const namespaces = [['assets', 'blob', 'web_assets'], ['saves', 'payload', 'web_saves']];

export class WebManualRecovery {
  #store;
  constructor(store) { this.#store = store; }
  async #inventory(signal, selected = null) {
    const until = performance.now() + 15000;
    return this.#store.transaction(['assets', 'saves'], 'readonly', async tx => {
      const candidates = []; let visited = 0, skipped = 0, found = null;
      for (const [store, field, source_kind] of namespaces) {
        await new Promise((resolve, reject) => {
          const request = tx.store(store).openCursor();
          request.onerror = () => reject(request.error);
          request.onsuccess = () => {
            try {
              checkAbort(signal);
              const cursor = request.result; if (!cursor) { resolve(); return; }
              requireValue(++visited <= 20000 && performance.now() < until, 'resource_limit', 'Manual recovery inventory exceeds its pass limit.');
              const blob = cursor.value?.[field];
              // Metadata is not needed to preserve the Blob, even when its key
              // or save fields are malformed. Unsupported keys remain retained.
              if (!(blob instanceof Blob) || typeof cursor.key !== 'string' || cursor.key.length > 4096) skipped++;
              else {
                const candidate = {candidate_id: identity(source_kind, cursor.key), source_kind,
                  display_name: recoveryLabel(cursor.key), bytes: blob.size, inspectable: blob.size <= RECOVERY_MAX_BYTES};
                candidates.push(candidate);
                if (candidate.candidate_id === selected) found = {candidate, blob};
              }
              cursor.continue();
            } catch (error) { reject(error); }
          };
        });
      }
      return {candidates, skipped, found};
    }, signal);
  }
  async list(after = null, {signal} = {}) {
    requireValue(after === null || typeof after === 'string' && HASH.test(after), 'invalid_request', 'Recovery cursor is invalid.');
    const inventory = await this.#inventory(signal);
    const candidates = inventory.candidates.filter(value => after === null || value.candidate_id > after).sort((a, b) => a.candidate_id < b.candidate_id ? -1 : 1);
    const more = candidates.length > 64; candidates.length = Math.min(64, candidates.length);
    return {candidates, next_after: more ? candidates.at(-1).candidate_id : null, retained_skipped: inventory.skipped};
  }
  async #inspect(id, signal) {
    requireValue(typeof id === 'string' && HASH.test(id), 'invalid_request', 'Recovery identity is invalid.');
    const {found} = await this.#inventory(signal, id);
    requireValue(found, 'not_found', 'Recovery source is no longer available. Refresh the inventory.');
    requireValue(found.candidate.inspectable, 'resource_limit', 'Recovery file exceeds 4 GiB.');
    const record = await inspectRecoveryBlob(found.blob, found.candidate.source_kind, found.candidate.display_name, {signal});
    checkAbort(signal);
    return {review: {candidate_id: id, revision: recoveryRevision(id, record), record}, blob: found.blob};
  }
  async inspect(id, {signal} = {}) { return (await this.#inspect(id, signal)).review; }
  async export(id, revision, {signal} = {}) {
    const {review, blob} = await this.#inspect(id, signal);
    requireValue(typeof revision === 'string' && HASH.test(revision) && review.revision === revision, 'conflict', 'Recovery source changed after inspection. Inspect it again.');
    // The captured IDB Blob is immutable; later removal/replacement cannot make
    // this copy read a different object. No writes are made to any object store.
    return {record: review.record, blob: await packRecovery(blob, review.record, {signal})};
  }
}
