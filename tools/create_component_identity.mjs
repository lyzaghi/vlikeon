import { generateKeyPairSync, createHash } from 'node:crypto';
import { mkdirSync, existsSync, writeFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { resolve, dirname } from 'node:path';

const root = resolve(dirname(fileURLToPath(import.meta.url)), '..');
const destination = resolve(root, 'security/component_identity.json');
if (existsSync(destination)) {
  process.stdout.write('Component identity already exists; retaining the published identity.\n');
  process.exit(0);
}
const { publicKey } = generateKeyPairSync('rsa', { modulusLength: 2048 });
// Only the public key is retained. Chromium component extensions use it for
// identity; this key is not used to sign releases or approve runtime binaries.
const encoded = publicKey.export({ type: 'spki', format: 'der' });
const digest = createHash('sha256').update(encoded).digest().subarray(0, 16);
const alphabet = 'abcdefghijklmnop';
const id = Array.from(digest, n => alphabet[n >> 4] + alphabet[n & 15]).join('');
const identity = { schema_version: 1, extension_id: id, origin: `chrome-extension://${id}/`, public_key_base64: encoded.toString('base64') };
mkdirSync(dirname(destination), { recursive: true });
mkdirSync(resolve(root, 'apps/game-hub'), { recursive: true });
writeFileSync(destination, JSON.stringify(identity, null, 2) + '\n', { flag: 'wx' });
writeFileSync(resolve(root, 'security/game_hub_origin.txt'), identity.origin + '\n');
writeFileSync(resolve(root, 'apps/game-hub/manifest.json'), JSON.stringify({
  manifest_version: 3,
  incognito: 'not_allowed',
  name: 'OmniPlay Game Hub',
  version: '0.1.0',
  description: 'Your local game library, saved progress and gaming devices.',
  key: identity.public_key_base64,
  permissions: ['nativeMessaging', 'storage'],
  background: { service_worker: 'background.js', type: 'module' },
  chrome_url_overrides: { newtab: 'index.html' },
  action: { default_title: 'Open OmniPlay' },
  content_security_policy: { extension_pages: "script-src 'self' 'wasm-unsafe-eval'; object-src 'none'; base-uri 'none'; frame-ancestors 'none'" },
  cross_origin_embedder_policy: { value: 'require-corp' },
  cross_origin_opener_policy: { value: 'same-origin' }
}, null, 2) + '\n');
process.stdout.write('Created component public identity and extension manifest.\n');
