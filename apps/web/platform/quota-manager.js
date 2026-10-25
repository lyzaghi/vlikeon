/**
 * Storage Quota, Pressure Warning, and Chunked Verification Manager for Web/PWA.
 * Evaluates browser storage estimates, persistent storage grants, and bounds memory
 * consumption during SHA-256 asset verification.
 */

const SHA256_K = new Uint32Array([
  0x428a2f98, 0x71374491, 0xb5c0fbcf, 0xe9b5dba5, 0x3956c25b, 0x59f111f1, 0x923f82a4, 0xab1c5ed5,
  0xd807aa98, 0x12835b01, 0x243185be, 0x550c7dc3, 0x72be5d74, 0x80deb1fe, 0x9bdc06a7, 0xc19bf174,
  0xe49b69c1, 0xefbe4786, 0x0fc19dc6, 0x240ca1cc, 0x2de92c6f, 0x4a7484aa, 0x5cb0a9dc, 0x76f988da,
  0x983e5152, 0xa831c66d, 0xb00327c8, 0xbf597fc7, 0xc6e00bf3, 0xd5a79147, 0x06ca6351, 0x14292967,
  0x27b70a85, 0x2e1b2138, 0x4d2c6dfc, 0x53380d13, 0x650a7354, 0x766a0abb, 0x81c2c92e, 0x92722c85,
  0xa2bfe8a1, 0xa81a664b, 0xc24b8b70, 0xc76c51a3, 0xd192e819, 0xd6990624, 0xf40e3585, 0x106aa070,
  0x19a4c116, 0x1e376c08, 0x2748774c, 0x34b0bcb5, 0x391c0cb3, 0x4ed8aa4a, 0x5b9cca4f, 0x682e6ff3,
  0x748f82ee, 0x78a5636f, 0x84c87814, 0x8cc70208, 0x90befffa, 0xa4506ceb, 0xbef9a3f7, 0xc67178f2
]);

const rotr = (value, count) => (value >>> count) | (value << (32 - count));

class IncrementalSha256 {
  constructor() {
    this.state = new Uint32Array([
      0x6a09e667, 0xbb67ae85, 0x3c6ef372, 0xa54ff53a,
      0x510e527f, 0x9b05688c, 0x1f83d9ab, 0x5be0cd19
    ]);
    this.buffer = new Uint8Array(64);
    this.words = new Uint32Array(64);
    this.used = 0;
    this.length = 0;
  }

  update(bytes) {
    this.length += bytes.byteLength;
    let at = 0;
    if (this.used > 0) {
      const n = Math.min(64 - this.used, bytes.length);
      this.buffer.set(bytes.subarray(0, n), this.used);
      this.used += n;
      at = n;
      if (this.used === 64) {
        this.processBlock(this.buffer, 0);
        this.used = 0;
      }
    }
    while (at + 64 <= bytes.length) {
      this.processBlock(bytes, at);
      at += 64;
    }
    if (at < bytes.length) {
      this.buffer.set(bytes.subarray(at), 0);
      this.used = bytes.length - at;
    }
  }

  processBlock(bytes, offset) {
    const w = this.words;
    for (let i = 0; i < 16; i++) {
      const at = offset + i * 4;
      w[i] = ((bytes[at] << 24) | (bytes[at + 1] << 16) | (bytes[at + 2] << 8) | bytes[at + 3]) >>> 0;
    }
    for (let i = 16; i < 64; i++) {
      const a = w[i - 15];
      const b = w[i - 2];
      w[i] = (w[i - 16] + (rotr(a, 7) ^ rotr(a, 18) ^ (a >>> 3)) + w[i - 7] + (rotr(b, 17) ^ rotr(b, 19) ^ (b >>> 10))) >>> 0;
    }
    let [a, b, c, d, e, f, g, h] = this.state;
    for (let i = 0; i < 64; i++) {
      const t1 = (h + (rotr(e, 6) ^ rotr(e, 11) ^ rotr(e, 25)) + ((e & f) ^ (~e & g)) + SHA256_K[i] + w[i]) >>> 0;
      const t2 = ((rotr(a, 2) ^ rotr(a, 13) ^ rotr(a, 22)) + ((a & b) ^ (a & c) ^ (b & c))) >>> 0;
      h = g;
      g = f;
      f = e;
      e = (d + t1) >>> 0;
      d = c;
      c = b;
      b = a;
      a = (t1 + t2) >>> 0;
    }
    const next = [a, b, c, d, e, f, g, h];
    for (let i = 0; i < 8; i++) {
      this.state[i] = (this.state[i] + next[i]) >>> 0;
    }
  }

  hex() {
    this.buffer[this.used++] = 0x80;
    if (this.used > 56) {
      this.buffer.fill(0, this.used);
      this.processBlock(this.buffer, 0);
      this.used = 0;
    }
    this.buffer.fill(0, this.used, 56);
    const view = new DataView(this.buffer.buffer);
    view.setUint32(56, Math.floor(this.length / 0x20000000), false);
    view.setUint32(60, (this.length * 8) >>> 0, false);
    this.processBlock(this.buffer, 0);
    return Array.from(this.state, v => v.toString(16).padStart(8, '0')).join('');
  }
}

export class QuotaManager {
  static kMinAvailableBytes = 100 * 1024 * 1024; // 100 MiB safety reserve
  static kUsageWarningRatio = 0.80; // 80% storage threshold

  /**
   * Retrieves current storage estimate and evaluates low-storage pressure.
   */
  static async getStorageEstimate() {
    if (!navigator.storage || !navigator.storage.estimate) {
      return { supported: false, usage: 0, quota: 0, percent: 0, isLowStorage: false };
    }

    try {
      const estimate = await navigator.storage.estimate();
      const usage = estimate.usage || 0;
      const quota = estimate.quota || 0;
      const available = quota > usage ? quota - usage : 0;
      const ratio = quota > 0 ? usage / quota : 0;
      const isLowStorage = (available < this.kMinAvailableBytes) || (ratio >= this.kUsageWarningRatio);

      return {
        supported: true,
        usage,
        quota,
        available,
        percent: Math.round(ratio * 100),
        isLowStorage
      };
    } catch {
      return { supported: false, usage: 0, quota: 0, percent: 0, isLowStorage: false };
    }
  }

  /**
   * Requests persistent storage grant from the browser to prevent eviction under disk pressure.
   */
  static async ensurePersistentStorage() {
    if (!navigator.storage || !navigator.storage.persist) {
      return false;
    }
    try {
      const isPersisted = await navigator.storage.persisted();
      if (isPersisted) return true;
      return await navigator.storage.persist();
    } catch {
      return false;
    }
  }

  /**
   * Performs memory-bounded SHA-256 checksum verification on a Blob or File.
   * Streams in 64 KiB slices to avoid full-file ArrayBuffer allocations and OOM crashes.
   */
  static async verifyBlobChecksum(blob, expectedHexSha256, chunkSize = 64 * 1024) {
    if (!blob || typeof blob.size !== 'number') {
      throw new Error('Invalid Blob input');
    }
    if (blob.size > 512 * 1024 * 1024) {
      throw new Error('Blob exceeds 512 MiB bound');
    }
    const step = Math.max(4096, chunkSize | 0);
    const hasher = new IncrementalSha256();
    let offset = 0;
    while (offset < blob.size) {
      const end = Math.min(blob.size, offset + step);
      const sliceBuffer = await blob.slice(offset, end).arrayBuffer();
      hasher.update(new Uint8Array(sliceBuffer));
      offset = end;
    }
    const computedHex = hasher.hex();
    return computedHex === String(expectedHexSha256).toLowerCase();
  }
}
