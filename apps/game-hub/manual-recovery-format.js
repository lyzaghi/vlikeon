import {Sha256, hashBlob} from './runtime/sha256.js';

export const RECOVERY_MAX_BYTES = 4 * 1024 ** 3;
const utf8 = value => new TextEncoder().encode(value);
const MAGIC = utf8('OPRC0001'), DOMAIN = utf8('Vlikeon unverified recovery v1\0');
const HEX = /^[a-f0-9]{64}$/;
const rawHash = hash => Uint8Array.from(hash.match(/../g), pair => parseInt(pair, 16));
const fields = ['schema_version', 'trust', 'source_kind', 'display_name', 'payload_bytes', 'payload_hash', 'prefix_hex'];
function requireValue(ok, message, code = 'corrupt_data') {
  if (!ok) throw Object.assign(new Error(message), {code});
}
export function recoveryRecord(value) {
  requireValue(value && typeof value === 'object' && !Array.isArray(value) && Object.keys(value).length === fields.length && fields.every(key => Object.hasOwn(value, key)), 'Recovery record fields are invalid.');
  requireValue(value.schema_version === 1 && value.trust === 'unverified' &&
    ['assets', 'legacy_saves', 'repair_files', 'web_assets', 'web_saves'].includes(value.source_kind) &&
    typeof value.display_name === 'string' && utf8(value.display_name).length <= 240 && !/[\u0000-\u001f\u007f-\u009f]/u.test(value.display_name) &&
    Number.isSafeInteger(value.payload_bytes) && value.payload_bytes >= 0 && value.payload_bytes <= RECOVERY_MAX_BYTES &&
    typeof value.payload_hash === 'string' && HEX.test(value.payload_hash) && typeof value.prefix_hex === 'string' &&
    value.prefix_hex.length === Math.min(value.payload_bytes, 32) * 2 && /^[a-f0-9]*$/.test(value.prefix_hex), 'Recovery record is invalid.');
  return value;
}
export function recoveryLabel(value) {
  let result = '';
  for (const ch of String(value).replace(/[\u0000-\u001f\u007f-\u009f]/gu, '\ufffd')) {
    if (utf8(result + ch).length > 240) break;
    result += ch;
  }
  return result;
}
export async function inspectRecoveryBlob(blob, source_kind, display_name, options = {}) {
  requireValue(blob instanceof Blob && blob.size <= RECOVERY_MAX_BYTES, 'Recovery file exceeds 4 GiB.', 'resource_limit');
  const payload_hash = await hashBlob(blob, options);
  const prefix = new Uint8Array(await blob.slice(0, 32).arrayBuffer());
  return recoveryRecord({schema_version: 1, trust: 'unverified', source_kind, display_name: recoveryLabel(display_name),
    payload_bytes: blob.size, payload_hash, prefix_hex: Array.from(prefix, byte => byte.toString(16).padStart(2, '0')).join('')});
}
export const recoveryRevision = (id, record) => new Sha256().update(utf8(JSON.stringify([id, recoveryRecord(record)]))).hex();
export async function packRecovery(blob, record, options = {}) {
  recoveryRecord(record);
  const actual = await inspectRecoveryBlob(blob, record.source_kind, record.display_name, options);
  requireValue(actual.payload_bytes === record.payload_bytes && actual.payload_hash === record.payload_hash && actual.prefix_hex === record.prefix_hex, 'Recovery bytes changed after review.', 'conflict');
  const json = utf8(JSON.stringify(record));
  const header = new Uint8Array(12); header.set(MAGIC); new DataView(header.buffer).setUint32(8, json.length, true);
  const body = new Blob([header, json, blob]);
  const hash = await hashBlob(new Blob([DOMAIN, body]), options);
  return new Blob([body, rawHash(hash)], {type: 'application/octet-stream'});
}
export async function unpackRecovery(blob, options = {}) {
  requireValue(blob instanceof Blob && blob.size >= 44 && blob.size <= RECOVERY_MAX_BYTES + 4096 + 44, 'Recovery package size is invalid.');
  const header = new Uint8Array(await blob.slice(0, 12).arrayBuffer());
  requireValue(MAGIC.every((value, index) => header[index] === value), 'Select a Vlikeon .oprecovery package.');
  const length = new DataView(header.buffer).getUint32(8, true);
  requireValue(length > 0 && length <= 4096 && 12 + length <= blob.size - 32, 'Recovery manifest length is invalid.');
  let record;
  try { record = recoveryRecord(JSON.parse(new TextDecoder('utf-8', {fatal: true}).decode(await blob.slice(12, 12 + length).arrayBuffer()))); }
  catch { requireValue(false, 'Recovery manifest is invalid.'); }
  requireValue(12 + length + record.payload_bytes + 32 === blob.size, 'Recovery payload length is invalid.');
  const hash = await hashBlob(new Blob([DOMAIN, blob.slice(0, -32)]), options);
  const trailer = new Uint8Array(await blob.slice(-32).arrayBuffer());
  requireValue(rawHash(hash).every((value, index) => value === trailer[index]), 'Recovery package checksum failed.');
  const payload = blob.slice(12 + length, -32);
  const observed = await inspectRecoveryBlob(payload, record.source_kind, record.display_name, options);
  requireValue(observed.payload_hash === record.payload_hash && observed.prefix_hex === record.prefix_hex, 'Recovery payload checksum failed.');
  return {record, payload};
}
