# Windows video worker build/staging and release recipe — item 56

Source authored only. No configure/build, staging, PE parser, package generation,
test, helper launch or media operation was executed. These are recipes for when
the owner resumes build/test, not new runtime evidence or release approval.

`tools/build_stream_video.py` requires an explicit existing Windows CMake path,
Visual Studio generator, x64/ARM64 architecture, SDK version and bounded build
parallelism. It refuses an existing output directory. It snapshots reviewed C++
sources/headers/CMake into a new output tree with before/after hashes, configures
and builds only the Release video-worker target, then stages the binary at
`package/helpers/omniplay-video-worker.exe`. No application/encoder test or worker
execution, download, SDK install, reset, signing or release step is included.

The CMake recipe uses static MSVC runtime, CFG, ASLR/high-entropy ASLR, NX and an
asInvoker/uiAccess=false manifest request. The receipt records source/recipe and
CMake executable hashes, selected generator/architecture/SDK, exact commands and
artifact bytes/hash. It does not pin or establish all compiler/linker/system SDK
inputs, reproducible binary identity, actual mitigation behavior or signatures.
Those remain release/toolchain evidence obligations.

`windows_video_artifact.py` performs bounded structural PE admission without
executing anything: regular nonlinked file up to 128 MiB, DOS/PE headers, matching
x64/ARM64 machine, PE32+ console executable (not DLL), bounded section table/file
offsets and configured CFG/ASLR/NX flags. It returns exact file hash/length.
This is not a full PE verifier, Authenticode validation or proof of safe/runtime
correct code. Seven inert synthetic fixture cases are authored and unexecuted.

The existing OPPK package tool now recognizes the fixed helper entry. If present,
it requires explicit approval in the caller-supplied executable inventory,
matching Windows platform/PE structure and matching hash/length from the same
inventory pass. An absent helper remains optional and cannot enable capture.
Existing full source/provenance/notices attribution and signing requirements
continue to apply; no distribution approval is manufactured by this check.

To assemble a future complete release, combine the staged helper with the
reviewed browser/broker tree, add its exact path to the full executable inventory,
attribute it and its actual dependencies in the reviewed source-offer/notices,
and use the existing package/signing workflow. The recipe's single-entry
`executables.json` is an additive staging aid, not the complete product inventory.
Build receipt stays outside the payload unless explicitly inventoried/attributed.

Remaining: run authorized build and actual dependency/signature/provenance
qualification, produce reviewed installed packages, wire native observer/service/
Stop UI and receiver/transport, and satisfy all original platform/media/fault/
performance requirements. Nothing here enables streaming or marks item 56 done.
