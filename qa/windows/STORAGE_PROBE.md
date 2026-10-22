# Final-pass Windows default-storage fixture

Authored fixture; no actual OS pass is claimed. Run only in an explicitly assigned
Antigravity batch or as part of item 16 using the Windows x64/MSVC/SDK environment and
the complete current Cargo workspace. It is a test actor, never a production
runtime/package/catalog entry. Record toolchain, source and executable hashes.

Create an ordinary fresh `out/qualification/windows-storage` output directory.
In a qualified MSVC developer shell, compile the copied fixture with static CRT,
CFG, ASLR, high-entropy VA and NX, retaining the exact command and build output:

```text
cl /nologo /std:c++20 /EHsc /W4 /WX /MT /O2 /guard:cf /DUNICODE /D_UNICODE /D_WIN32_WINNT=0x0A00 /DWINVER=0x0A00 qa/windows/storage-probe.cpp /Fo:out/qualification/windows-storage/storage-probe.obj /Fe:out/qualification/windows-storage/storage-probe.exe /link /DYNAMICBASE /HIGHENTROPYVA /NXCOMPAT /GUARD:CF advapi32.lib userenv.lib windowsapp.lib
```

Set `OMNIPLAY_WINDOWS_STORAGE_PROBE` to that executable's absolute path, then
explicitly select the ignored real-OS case in the final consolidated pass:

```text
cargo test --frozen -p omniplay-sandbox --target x86_64-pc-windows-msvc windows::storage_tests::actual_actor_cannot_write_its_default_storage_but_can_use_private_scratch -- --ignored --exact --nocapture
```

The test copies the exact fixture into a new isolated temporary runtime,
grants readonly runtime/game access and a separate writable scratch directory,
creates a fresh AppContainer and launches through the actual contained-process
API. Product creation/job/token/default-storage policy is exercised directly;
the fixture receives no alternative privilege or launch bypass.

Expected stdout is exactly `storage-probe-v2 1 1 1 1 1 1 1`. The seven real operations
are default `AC` file-write denial, package-root subdirectory-create denial,
package-root `WRITE_DAC` handle denial, attempted label-lowering denial,
private registry subkey-creation denial, private value-write route denial and
successful private scratch create/write/flush/delete. Registry readonly lookup
must succeed before either registry result can pass. RegCreateKeyEx uses the
key's descriptor check even though its parent handle is readonly; this is an
actual mutation attempt rather than a missing handle-rights assertion.
For the value check, the fixture first requests KEY_SET_VALUE. If that succeeds,
it attempts RegSetValueEx on the returned handle; either step must reject access.
The exact host scratch identity is carried in OMNIPLAY_PRIVATE_SCRATCH: Windows
may rewrite TEMP/TMP during AppContainer construction. Known access denial and
privilege denial are accepted for attempted creation; positive writable scratch
is required in the writable case. Missing paths, unsupported
APIs, arbitrary I/O errors or an unexecuted fixture are not successful denial
evidence. A label-lowering failure is tested before any scratch success claim.
Only the test's new OS profile is affected if the negative operation unexpectedly
succeeds; the case must fail and retain diagnostic evidence.

The harness checks that its own caller thread has no impersonation token both
before and after launch. It caps captured output, uses a finite response wait, terminates the
contained probe and requires confirmed process exit before reporting success.
Missing environment/artifact input fails this explicitly selected case. An
ordinary unit run which skips this ignored case proves no Windows behavior.

For the companion readonly-scratch case, compile the same source into a separate
fresh executable with `/DOMNIPLAY_EXPECT_SCRATCH_DENIED`, set
`OMNIPLAY_WINDOWS_READONLY_STORAGE_PROBE` to its absolute path and select
`windows::storage_tests::actual_actor_without_scratch_grant_cannot_write_private_scratch`
with `--ignored --exact --nocapture`. Expected stdout is exactly
`storage-probe-readonly-v1 1 1 1 1 1 1 1`. The scratch directory must exist and be
readable, and its actual file-creation attempt must return access/privilege denial.
No writable scratch grant is installed by this case. It tests the sandbox grant
boundary; product package selection and legacy core compatibility remain case 28.

The harness deletes its temporary tree only after confirmed process exit,
expected observations and successful profile retirement. An uncertain or failed
run retains the tree and prints its location. Reader joins require actual held
Windows thread completion, including TLS teardown.

The fixture links CoTaskMemFree through WindowsApp.lib's API-set import, avoiding
an unnecessary static OLE32 desktop dependency in this headless actor. The same
seven OS operations and expected results remain required. Microsoft lists the
allocator in its [WindowsApp API set](https://learn.microsoft.com/en-us/uwp/win32-and-com/win32-apis).
Batch 020's separate kernel32-only staged probe reached entry and loaded ADVAPI32
but reported OLE32 initialization error 1114; it did not pass the storage cases.
Full worker/helper/core dependency compatibility remains a separate requirement.

Also run matrix cases 15 and 16 for default-folder layout variants, existing child ACLs,
alternate streams, replace/delete/rename attempts, token-policy mismatches,
partial sealing and real production core/helper/driver compatibility. A passing
fixture does not prove hard disk quota, general registry isolation, whole-profile
physical accounting or general sandbox completeness.
