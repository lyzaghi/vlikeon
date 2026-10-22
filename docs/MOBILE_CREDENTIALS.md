# Native mobile device credential source

The sync client now accepts an explicit native `CredentialStore` for create,
recover, renew, open, disconnect, account deletion and reconciliation. Existing
desktop entry points retain their operating-system keyring adapter. On mobile,
that default fails closed instead of selecting a mock/plaintext fallback. The
device credential remains the existing versioned device ID/Ed25519 device seed/
save-data key record; the account recovery/root seed is never persisted by this
provider. Inputs and returned credentials have a 4096-byte bound, canonical
32-byte base64url seed/key encoding and zeroizing Rust owners.

The C ABI uses `om_mobile_open_with_credentials_v2` and a native-only version-2
callback table with a noninteractive, profile-bound lock/access check. It uses a
new symbol rather than accepting a smaller older table. Rust consumes context
ownership on success or rejection, binds
the provider once to the opened UUID and checks that binding before every call.
Callbacks receive exactly sixteen profile UUID bytes and synchronously borrowed
buffers; none may be retained or reenter the Rust host. Read returns a bounded
byte count; write succeeds only after provider acknowledgement; remove tolerates
an absent exact record. Missing, locked and failed native operations stay distinct.
An Arc retains the context across potential workers and releases it after the
last owner, including failed/duplicate profile opens. The unsafe native caller
must supply valid function pointers, buffers, thread safety and release behavior;
this is not a renderer-callable registration API.

`LocalHost` retains the explicit provider separately from transient page grants.
Detaching a renderer does not export/remove device credentials; closing the last
native owner releases only the in-process provider. There is no command returning
credential bytes or taking a renderer-selected vault name/path. Mobile hello now
reports the authored sync path only when opened with the explicit provider;
legacy opens keep it unavailable. Account commands and one explicit foreground
worker are connected in source, with separate native lifecycle cancellation.
`MOBILE_SYNC.md` describes this unexecuted integration, its in-flight I/O limits
and the still-required background/device/store/release work. No account,
credential, service or network operation has been run.

The current UI additionally uses a bounded owner-bound account worker with
one-shot/expiring secret results, described in `MOBILE_ACCOUNT_ACTIONS.md`. Its
HTTP operation runs outside the local host mutex; legacy direct commands retain
their existing behavior. No secure-store or worker behavior has been exercised.

On iOS the trusted Swift open path supplies a retained, locked native object.
Generic-password Keychain items use fixed service `org.omniplay.sync.device.v1`
and the canonical native profile UUID as account. They are explicitly
non-synchronizing and `WhenUnlockedThisDeviceOnly`; no access group is added and
no Keychain prompt is requested by the callback. Copy/update/add/delete statuses
are checked. The exact service/account is removed on disconnect; corrupt or
unavailable entries are not silently replaced during read. Swift temporary Data
is reset best-effort after a borrowed copy, but COW/OS/runtime copies are outside
an application's guaranteed wiping boundary. Keychain API acknowledgement is not
device-fault or power-loss evidence.

On Android the trusted Java open path passes a separate native-only provider
object to JNI. The app's AndroidKeyStore creates/retains a per-profile AES-256-GCM
wrapping key. It does not export that wrapping key. The encrypted record under
canonical `getNoBackupFilesDir()/OmniPlaySecureCredentials` has `OPCR0001`, a
provider-generated 12-byte IV and bounded ciphertext/tag; the authenticated data
binds its domain, profile bytes and header. It contains no plaintext credential.
An invalidated/missing key alongside retained ciphertext fails rather than
generating a replacement silently. The implementation does not assert StrongBox,
TEE residence or hardware attestation.

Android checks user/keyguard unlock before access and before returning decrypted
bytes. API 35+ also requests `setUnlockedDeviceRequired(true)` at key generation;
earlier versions use app checks because Android documents credential-loss/unlock
bugs through API 34. Those checks do not become equivalent hardware enforcement,
and a lock race or already-loaded sync key still needs lifecycle qualification.
No per-operation biometric prompt or continuous in-memory protection is claimed.

Android record access has a nonblocking exclusive file lease plus in-object
serialization. Ordinary-file/non-link/bound checks cover the exact base/new/bak
record names. The owned directory and files request 0700/0600. Write synchronizes
the file, finishes AtomicFile publication, reads back the selected ciphertext and
synchronizes the parent; new-directory publication also requests parent sync.
Failure cleanup remains under the same lease. A zero-length per-profile lease
file is retained. Removal deletes only the exact key first, then the three
selected record files; failed cleanup can be retried without making old ciphertext
readable. Removal can delete an oversized ordinary exact record without reading
it; read/write still reject oversized records. There is no traversal, broad keystore alias enumeration or recursive
credential deletion. Same-UID hostile path races, provider/OS/storage behavior
and physical durability remain unverified.

JNI retains one global provider reference, looks up fixed kept method signatures,
attaches native worker threads through JavaVM rather than sharing JNIEnv and
releases local frames/owned thread attachments. Credential byte arrays and native
request vectors are wiped on their owned paths; exceptions become bounded generic
failures without logging secret data. The Java generic native request path also
wipes its owned request/response byte arrays. Immutable JSON strings, GC/crypto/
Keychain internals and a VM already shutting down cannot be promised zeroized or
released on a fixed deadline. R8/ABI/thread/failure qualification is outstanding.

Cargo.lock was refreshed by offline dependency fetch only; no compiler, syntax/
configuration check, app, JNI/Swift/Keystore/Keychain API, device or test has run.
Android/iOS source assembly must carry the new header/provider/kept callbacks.
Native background scheduling and repeated phase access checks are now authored in
`MOBILE_BACKGROUND_SYNC.md`. Complete lock/suspend/recovery/backup/uninstall and
scheduler qualification, store/privacy evidence and all other full-scope
requirements remain required.

The final consolidated pass must cover callback ABI/ownership and duplicate-open
cleanup, wrong-profile/oversize/empty/status failures, concurrent jobs and close,
all secure-store lock/reboot/invalidation states, no plaintext fallback, read/
write/remove faults and publication interruption, corrupt/wrong-AAD ciphertext,
lost native responses, exact deletion retries, cross-process leases and JNI
thread/exception/R8 behavior, iOS device-only/non-sync item properties, backups/
restore/uninstall, recovery root non-retention and real Android/iOS devices.

Primary APIs: [Android app-owned keystore](https://developer.android.com/privacy-and-security/keystore),
[Android unlocked-device policy and older-version limitations](https://developer.android.com/reference/android/security/keystore/KeyGenParameterSpec.Builder#setUnlockedDeviceRequired(boolean)),
[JNI thread/reference rules](https://developer.android.com/ndk/guides/jni-tips),
[Apple device-only unlocked Keychain accessibility](https://developer.apple.com/documentation/security/ksecattraccessiblewhenunlockedthisdeviceonly).
