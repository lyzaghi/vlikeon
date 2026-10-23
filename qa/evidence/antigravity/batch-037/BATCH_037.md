Execute exactly once: node E:\V10\work\vlikeon-qa-20261023\run-batch-037.mjs

Exact test-only assignment: dispatch the existing manual GitHub workflow linux-native-qualification.yml on lyzaghi/vlikeon main with expected_sha=b1f7439ba1cabd9d025e701c9fbf99cade3b3d5b and batch_id=vlikeon-037, locate this exact unique run, wait for its terminal result and collect raw logs/job outcomes. The fixed Node runner performs these commands using the existing authenticated gh CLI. No local source/product process is executed.

Workflow runs on disposable GitHub Ubuntu24.04 runners with read-only repository contents permission. Its fixed steps provision Rust1.89.0 in that disposable runner, download/compile locked Cargo dependencies, check actual Linux sandbox/runtime/broker, run only linux_host_channel::tests, and independently compile the static-pie C bootstrap. No privileged cgroup/systemd/desktop tests are assigned. No package, service, distribution, WSL or permission settings on the user's machine are installed or changed. Source SHA guard and unchanged tracked-source checks are mandatory.

Antigravity executes only this runner. No source/test/workflow/expectation edits, diagnosis, suggestions, fixes, retries, extra dispatches, Git/publishing or completion decisions. Only new runs/batch-037 output and the one exact remote test workflow run are authorized. Stop/report denial, missing prerequisite, uncertain dispatch or failed execution. Codex alone reviews raw evidence and decides fixes. A workflow success is not whole item3/product acceptance.

New workflow revision fixes runner context in job environment. Batch036 dispatch was rejected HTTP422 before any workflow execution. Do not replay036.
