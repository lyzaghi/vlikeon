# Diskless macOS guest source

This is an unbuilt implementation for Apple Silicon and macOS 13 or newer.
Following the user's explicit approval to activate before qualification,
`kExecutionBoundaryComplete` is **true in source**. The earlier authorization
block is resolved; no binary was rebuilt, installed or launched. Signed package,
image, profile and resource checks still gate each actual launch. This document
describes source, not measured containment or a distributable runtime.

## Source composition

The XPC owner instantiates a Linux ARM64 `VZVirtualMachine` on its own serial
dispatch queue. The untrusted worker/core runs inside that guest, never as a
native macOS child. A separate signed macOS helper owns window/audio/input and
uses the bounded OMPRES02 wire. The guest has no network, writable host disk,
GPU/audio/HID/USB device or Rosetta share. Its only host directory share is the
current staged input directory, configured read-only at the VZ device boundary.
Neither the owner container nor broker profile is a guest-writable mount.

Each guest declares 256–8192 MiB RAM, 1–4 vCPUs and 16–512 MiB scratch tmpfs.
Scratch must fit half guest RAM; initramfs is at most 256 MiB and half guest RAM.
PID 1 mounts a read-only virtiofs lower directory and a tmpfs-backed overlay at
`/work`. The root initramfs, any additional guest-created RAM files and malicious
guest processes consume the same fixed guest memory. No paging disk is attached.
These limits describe the guest allocation; they do not bound all macOS/VZ/SDL
driver overhead or turn helper RLIMIT_AS into a physical-memory quota.

Four session slots and the sum of guest RAM plus helper declarations are reserved
under the owner lock before allocation. The aggregate declared budget is at most
12 GiB and half physical host RAM. A slot remains charged until VZ has acknowledged
termination/released its machine and the owned helper has been reaped. Failed or
uncertain observations retain the slot and launch fence. No persisted PID can
authorize a signal or refund. Stop retries are asynchronous on the VZ queue;
callbacks do not block the XPC connection queue waiting for guest shutdown.

The VM is an owner-process resource; core descendants remain in its guest.
The helper's private lifetime-pipe reader is in a separate trusted process, not
in the core address space. Actual owner-crash, startup-cancellation, suspended
helper, callback-failure and OS resource behavior still need Mac qualification.

## Boot and artifact authoring

1. Select exact Linux ARM64 core/dependency artifacts and their source/license
   identities. The per-core implementations and artifacts remain item 5; the
   existing Windows-only native package tool is not a macOS build recipe.
2. Build `engines/native` with an explicit Linux ARM64 toolchain,
   `OMNIPLAY_GUEST_ACTOR=ON`, `OMNIPLAY_DEVICE_FREE_ACTOR=ON`, software video and
   `OMNIPLAY_BUILD_PRESENTATION_HOST=OFF`. Core filename is an adjacent basename.
   Build `engines/macos-guest` with a static ARM64 C toolchain for PID 1.
3. Supply an explicitly pinned uncompressed ARM64 Linux Image with the required
   built-in features in `engines/macos-guest/kernel.config`. Supply its actual
   source/config hashes, source URL, revision and corresponding-source/license
   materials. A declared receipt alone does not prove a reproducible kernel.
4. `tools/author_macos_guest.py` consumes the declared package worker/core/assets,
   explicit `/lib` dependencies, PID 1 and kernel. It checks actual input hashes,
   ELF architecture/type and static init, and writes deterministic uncompressed
   newc bytes, `guest-kernel` and a provenance receipt. It does not execute code.
   A provisional input catalog may declare the not-yet-created boot images;
   those excluded entries must be replaced with actual output hashes/sizes in
   the final authenticated catalog. Never publish a provisional catalog.
5. Author the final native catalog with the new closed `MacPackage` fields:
   `virtualization=true`, `guest_protocol=1`, `guest_kernel`, `guest_initrd`,
   `guest_cpu_count`, `scratch_memory_mb`, owner/helper signing identities and
   exact entitlement hashes. Guest worker/core are ELF, not Apple-signed Mach-O.
6. Author owner catalog schema 2 with `author_macos_runtime_catalog.py`. Each entry
   has ID/team, helper basename/signing ID/memory, kernel and initrd basenames/
   hashes/byte lengths, guest RAM/vCPUs and scratch. No schema-1 host actor fallback
   exists. Broker admission scans newc without extraction, rejects links/special
   files/duplicates, and hashes embedded worker/core/assets against outer files.
7. Build the macOS owner/helper, populate `Resources/Runtimes/<id>` before signing,
   sign nested code with the closed entitlements, and assemble the named broker
   app using `assemble_macos_broker.py`. Owner requests App Sandbox plus the public
   virtualization entitlement. Helper requests App Sandbox plus inheritance.
   Final app signing/notarization and package inventory use actual final bytes.

These are future operator steps, not commands run during the source-only phase.
No fabricated binary/hash, license approval or signed release is supplied here.

## Updates, failures and recovery

The authenticated outer release, native catalog, sealed owner catalog, exact
helper identity and mirrored boot image hashes must agree. A missing image,
changed embedded core, unsupported OS, denied entitlement or invalid VZ config
leaves native execution unavailable; the library/web route remains available.
There is no native host-core fallback or development-root bypass.

Save-state compatibility includes guest protocol and both boot image hashes;
portable in-game formats retain their existing rules. Mutable game/save work
inside the guest is discarded only after normal save export or explicit session
termination. The broker retains save history outside the guest.

Protected broker journals and previous-boot recovery remain as described in
`MACOS_BOOT_RECOVERY.md`. Same-boot uncertain work is retained until an owned
termination receipt or a later supported boot proof exists. Unknown legacy,
lease-less or schema-1 data is deliberately unsupported: retain it, preserve the
profile/container and diagnose before any operator-authorized repair. An update
must not manufacture provenance, adopt stale PIDs, silently delete legacy data
or refund its quota. This is a supported fail-closed outcome, not automatic repair.

Source activation was explicitly authorized by the user. Read-only source/diff
review does not replace compilation, VM boot, real game/save/media behavior,
owner-loss/resource-abuse tests, signing/notarization or install/upgrade tests.

## Primary API references

Apple documents the serial queue requirement for
[VM construction](https://developer.apple.com/documentation/virtualization/vzvirtualmachine/init(configuration:queue:)),
the [read-only share](https://developer.apple.com/documentation/virtualization/vzshareddirectory),
the [Linux boot loader](https://developer.apple.com/documentation/virtualization/vzlinuxbootloader),
[multiport consoles](https://developer.apple.com/documentation/virtualization/vzvirtioconsoledeviceconfiguration)
and [termination callbacks](https://developer.apple.com/documentation/virtualization/vzvirtualmachinedelegate).
These references informed source authoring; they are not test evidence.
