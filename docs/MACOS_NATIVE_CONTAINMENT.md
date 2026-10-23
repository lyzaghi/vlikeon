# macOS native containment source

The current unbuilt implementation replaces the inherited host-core design below
with a diskless ARM64 Linux guest and a separate signed native presentation helper.
Owner entitlements now include the public virtualization entitlement. Current
interfaces, schema-2 catalogs and bounds are in `MACOS_DISKLESS_GUEST.md`.
Following the earlier automatic rejection, the user explicitly authorized
activation before qualification; the source execution gate is now true. Actual
signature/image/profile/resource checks remain. No build, VM execution or test occurred.

## Historical source record

The previous architecture/status below is preserved as history. The current
guest document and the current-revision section of `MACOS_ITEM4_CLOSURE.md`
supersede its host-core design and statements about remaining implementation.

Current source work is tracked in `MACOS_ITEM4_CLOSURE.md`. The owner now directs
source authoring first and practical verification later. New low-level source
adds suspended pair admission, retained process ownership, profile-bound schema-3
receipts, exclusive storage leases and durable pre-spawn launch fences. The current
worktree also authors the authenticated XPC client, RuntimeAdapter, save channel,
profile association, focus/lifetime pipes and app assembly. Launch remains false
because the inherited shared writable container, aggregate resource boundary and
independent crash retirement are still unresolved source obligations. See the
closure document for current behavior; no transition to item 5 is authorized.

## Historical foundation before the current adapter work

The description below records the earlier foundation, not the current complete
operation inventory or target evidence. The closure document above supersedes
its statements about which protocol operations have source implementations.

OmniPlay keeps native execution unavailable on macOS until the complete signed
App Sandbox launch and storage boundary is present. It does not call the
deprecated `sandbox-exec` tool, use private Seatbelt interfaces or treat POSIX
permissions as containment. Apple documents App Sandbox as kernel-enforced
access control driven by code-signing entitlements, and recommends XPC for
privilege separation. The implementation therefore requires independently
signed actor and presentation code and a later authenticated XPC owner rather
than inheriting ambient authority from the Chromium native-messaging broker.

The first source policy adds separate XPC owner, actor and presentation
entitlement files. The owner contains only
`com.apple.security.app-sandbox=true`. Each child contains exactly App Sandbox
plus `com.apple.security.inherit`, the closed child shape Apple documents for a
process spawned by a sandboxed parent: no network client or server, JIT,
unsigned executable memory, library-validation bypass, camera,
microphone, USB, Bluetooth, contacts, calendar, location or user-selected file
grant is declared. The catalog binds all three distinct signing identities and
all three exact entitlement hashes. The actor remains on the private bounded presentation
protocol and software-frame boundary; it cannot own the desktop window or audio
device. The catalog must bind distinct reverse-DNS signing identifiers, a
ten-character team identifier, exact entitlement hashes, hardened runtime and
the absence of every prohibited widening flag. macOS packages without that
closed declaration are rejected during catalog admission.

The CMake sources remove build and install rpaths, request hardened runtime in
Xcode generators, suppress injected base entitlements and bind the explicit
entitlement file to each target. Packaging must still prove the resulting
Mach-O architecture, designated requirements, team identity, hardened-runtime
flags, nested-code seals, entitlement bytes and notarization ticket. A CMake
property or catalog declaration is never accepted as that proof.

The XPC owner target provides an authenticated endpoint. Its signed
bundle carries an exact broker designated requirement. For each incoming
connection it obtains the kernel audit token, resolves that live guest through
Security.framework and applies strict code validation against the configured
requirement before installing a message handler. Control dictionaries are
versioned and closed. The only current operation returns finite capabilities
and explicitly reports session launch unavailable; all unknown shapes and
unauthenticated peers are rejected without reflecting their data. The endpoint
also accepts a closed `stage_session` request containing one to 144 read-only
ordinary file descriptors, ASCII basenames, exact byte counts and SHA-256
identities. It admits at most 8 GiB and four staged sessions, copies from offset
zero into a new private owner-container directory, hashes the bytes actually
written, flushes every file and receipt, then publishes by directory rename.
Input descriptors are adopted immediately and close on every rejection path.
Inventory is serialized and bounded; unknown names or shapes fail closed, while
abandoned pending directories are removed only before launch exists. An explicit
`discard_session` request removes an inactive staged UUID directory. The
endpoint still contains no child launch, active-session deletion, path supplied
by the broker, save-return or presentation operation, so it does not expose an
incomplete execution route.

Staging also binds an exact runtime ID admitted from a canonical catalog sealed
inside the XPC bundle. The catalog accepts at most 32 unique IDs and identifies
three distinct thin arm64 Mach-O files per runtime: actor, presentation and core.
All paths are fixed basenames below `Resources/Runtimes/<runtime-id>` and every
component must be an ordinary single-link non-set-ID file. Security.framework
validates each static code object strictly against a generated identifier/team
requirement and all architectures. Actor and presentation signing information
must carry hardened runtime and only the closed App Sandbox child entitlement
set; the core must carry hardened runtime without sandbox widening. A source-only
authoring tool emits the canonical catalog from closed identity JSON. The owner
repeats actual bundle/code admission on each staging request; catalog claims do
not substitute for signed-code inspection.

The ordinary native adapter currently stages work beneath the broker profile
and passes filesystem paths to its child. A separately sandboxed executable
cannot safely inherit that ambient profile access, and using a shared container
directly from an unsandboxed broker can cause privacy prompts on current macOS.
Accordingly `omniplay_sandbox::supported()` remains false on macOS. Enabling it
requires the XPC owner's signed runtime-image admission and bounded child launch,
save-return protocol, exact child audit-token/code
requirement validation, bounded memory/process lifetime, crash recovery and the
separate trusted presentation launch. The actor must receive game, firmware,
configuration and save bytes through that boundary and return only checked
save/progress frames.

Primary platform references used for this design are Apple's
`Configuring the macOS App Sandbox`, `Protecting user data with App Sandbox`,
`Embedding a command-line tool in a sandboxed app` and `Creating
distribution-signed code for macOS` documentation:
<https://developer.apple.com/documentation/xcode/configuring-the-macos-app-sandbox>,
<https://developer.apple.com/documentation/security/protecting-user-data-with-app-sandbox>,
<https://developer.apple.com/documentation/xcode/embedding-a-helper-tool-in-a-sandboxed-app>,
and <https://developer.apple.com/documentation/xcode/creating-distribution-signed-code-for-the-mac>.

No Xcode, compiler, linker, codesign, App Sandbox, XPC, helper, core, window,
audio, input, browser or test has run. These sources are a fail-closed package
and build-policy foundation, not macOS native availability or qualification.
