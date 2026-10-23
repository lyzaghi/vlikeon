Execute exactly once: node E:\V10\work\vlikeon-qa-20261023\run-batch-039.mjs

Exact test-only assignment: dispatch the existing manual GitHub workflow linux-native-qualification.yml on lyzaghi/vlikeon main with expected_sha=2008e169412420db8f3037f71a2b4c898dc7af11 and batch_id=vlikeon-039, locate this exact unique run, wait for its terminal result and collect raw logs/job outcomes. The fixed Node runner performs these commands using the existing authenticated gh CLI. No local source/product process is executed.

Workflow runs on disposable GitHub Ubuntu24.04 runners with read-only repository contents permission. Its fixed steps provision Rust1.89.0 in that disposable runner, download/compile locked Cargo dependencies, check actual Linux sandbox/runtime/broker, run only linux_host_channel::tests, and independently compile the static-pie C bootstrap. No privileged cgroup/systemd/desktop tests are assigned. No package, service, distribution, WSL or permission settings on the user's machine are installed or changed. Source SHA guard and unchanged tracked-source checks are mandatory.

Antigravity executes only this runner. No source/test/workflow/expectation edits, diagnosis, suggestions, fixes, retries, extra dispatches, Git/publishing or completion decisions. Only new runs/batch-039 output and the one exact remote test workflow run are authorized. Stop/report denial, missing prerequisite, uncertain dispatch or failed execution. Codex alone reviews raw evidence and decides fixes. A workflow success is not whole item3/product acceptance.

New workflow revision fixes runner context in job environment. Batch036 dispatch was rejected HTTP422 before any workflow execution. Do not replay036.

Source d07305c fixes compiler-reported borrowed temporary and pins original Linux6.12 UAPI header/licenses with SHA/layout checks, retaining runtimeABI6. Eight socket cases now include bounded-connect flags. Batch037 terminal failed before any socket test. Do not replay prior batches.

Source2008e16 adds sys/wait.h for compiler-reported WEXITED and the workflow explicitly provisions only libdbus-1-dev/pkg-config via apt in its disposable Ubuntu runner, recording installed versions. This is an assigned build prerequisite, not permission for Antigravity to install anything locally or edit dependencies. Batch038 actual sandbox compile+8socket tests passed, broker blocked missing SDK and bootstrap failed missing include. New batch039 qualifies the resulting source; preserve those records.
