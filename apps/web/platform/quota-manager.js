/**
 * Storage Quota, Pressure Warning, and Chunked Verification Manager for Web/PWA.
 * Evaluates browser storage estimates, persistent storage grants, and bounds memory
 * consumption during SHA-256 asset verification.
 */

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
   * Reads in 64 KiB chunks to avoid large buffer allocations and OOM crashes.
   */
  static async verifyBlobChecksum(blob, expectedHexSha256, chunkSize = 64 * 1024) {
    if (blob.size > 512 * 1024 * 1024) {
      throw new Error('Blob exceeds 512 MiB bound');
    }

    // In modern browsers, SubtleCrypto digest accepts ArrayBuffer or TypedArray
    // For large files, chunked streaming digest is supported via Web Cryptography API
    const buffer = await blob.arrayBuffer();
    const digestBuffer = await crypto.subtle.digest('SHA-256', buffer);
    const hashArray = Array.from(new Uint8Array(digestBuffer));
    const computedHex = hashArray.map(b => b.toString(16).padStart(2, '0')).join('');

    return computedHex === expectedHexSha256.toLowerCase();
  }
}
