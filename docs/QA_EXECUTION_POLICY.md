# Test execution and decision ownership

The owner directs Codex to complete the full application and delegates test
execution to Antigravity. The owner additionally specifies that Antigravity
has test authority only; review and fix decisions belong to the owner and Codex.
This policy does not mark any product item complete.

## Current owner direction — 2026-10-23

The owner now directs completing source authoring first and deferring practical
verification. This supersedes the earlier concurrent-test timing below: do not
dispatch another build, test, workflow or Antigravity execution batch during
this source-first phase. Historical results remain evidence only for their exact
revisions and scope. Antigravity retains test-only authority when the owner
later resumes verification; Codex retains source, diagnosis and fix decisions.
Source inspection and necessary implementation documentation lookup may continue.

## Codex and owner

- Own requirements, implementation, test selection, expected behavior and the
  exact frozen source/artifact identity assigned to each test batch.
- Define commands, prerequisites, isolated disposable test data, runtime limits
  and the evidence destinations before releasing an execution batch.
- Review raw evidence, diagnose issues, decide fixes and edit product/test code.
- Accept or reject results and decide roadmap completion and release readiness.

## Antigravity

- Execute only the exact commands in a released test batch, against its named
  source/artifact snapshot and disposable test workspace.
- Report assertion outcomes, exit codes, stdout/stderr, elapsed time, artifact
  identity and evidence paths. Distinguish executed, failed, blocked, skipped
  and not run; absence of a result is never a passing test.
- Write only the logs/reports/artifacts explicitly allowed by the test batch.
- Stop and report missing prerequisites, denied actions and unexpected changes.

Antigravity must not perform source review or diagnosis, propose/implement
fixes, change tests/expectations/golden files, edit product source, install
dependencies, weaken permissions, broaden command scope, commit/push/publish,
declare a roadmap item finished or approve a release. A new execution batch
requires an explicit Codex assignment within the owner's authorized scope.
An exact released Cargo test batch may explicitly authorize normal locked
dependency download/compilation into its isolated cache/target directory. This
does not authorize ad hoc installs, version changes, global environment edits
or changes to source, tests or lockfiles.

## Timing and evidence

The owner subsequently explicitly requested concurrent work. Codex continues
source authoring through the original roadmap while Antigravity may execute
specific released test batches against frozen copies. This supersedes the
former deferral for those explicitly released batches. Full platform/device/
fault/release qualification remains item 16. Codex prepares each exact batch;
no autonomous execution, scope expansion or retesting is enabled.

The initial Antigravity handoff against source commit
`264d4d0860d7fec80b43a67ee4466143700bc390` encountered a denied directory-inventory
command and produced no deliverables. No test or code change resulted. Its CLI
SUCCESS marker is not accepted as successful delivery. The previous review/
planning assignment is withdrawn following the owner's scope correction.
No permission settings were changed and the denied action is not retried.

The first concurrent batch selects only the existing Node browser context and
document-ownership tests at that frozen commit. Its fixed runner enforces file
hashes, 60-second subprocess deadlines and bounded logs. Antigravity attempted
the exact assigned command; its separate RunCommand permission gate denied it
before execution. No test artifacts or passing result exist. That command is
not retried, rephrased or executed through another transport. Further execution
requires an actual supported permission-state change; source work continues.

Subsequent owner-confirmed permission changes allowed the exact bridge batches
to execute through the normal CLI. Codex did not alter permission settings or
bypass the gate. Recorded build/test evidence begins with the assigned batches;
the historical denied attempt remains unexecuted. Tool availability does not
expand Antigravity's test-only decision authority defined above.

Later reports must bind the exact batch/source/artifact, environment, command,
exit status and raw evidence. A source copy, an agent's assertion or a CLI
success marker alone cannot establish application correctness or 100% readiness.

## First executed batch

After the owner reported Antigravity permission changes, the normal command
gate allowed execution. The first invocation failed in PowerShell parsing;
Codex corrected the missing call operator and Antigravity executed the fixed
batch. Codex changed no permission settings. Batch 001 then returned exit 0
for both Node files: 7 ownership tests and 6 context tests, zero failures/skips,
empty stderr and unchanged source hashes. Codex reviewed the raw TAP logs.
Evidence is in `qa/evidence/antigravity/batch-001/` at source commit 264d4d0.
This resolves the initial execution blocker for that batch. It establishes only
those unit cases, not actual Chromium, native Windows or whole-product behavior.

Batch 002 attempted the native Rust control-worker filter on a8b5c4d. Cargo
returned 101 because the committed lockfile lacked workspace dependency edges;
no test executed and the second filter was not attempted. Codex repaired only
the missing lock edges with `cargo update --workspace` (zero package updates).
The retry must keep --locked and use a new frozen source/batch, not edit the
failed snapshot or replay its evidence directory.
