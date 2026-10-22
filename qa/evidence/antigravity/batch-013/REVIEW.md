# Actual Windows worker lifecycle and framing

Codex inspected both actual evidence files: all 13 lifecycle and all 19 wire
cases passed. Both harness invocations exited zero; artifact and harness hashes
match batch 012 and remained unchanged. Raw outer stdout/stderr are empty;
scenario observations and exit codes are in the two evidence JSON files.

Scope: direct Windows worker with dedicated qualification DLLs, real pipes and
dummy SDL drivers. Includes unknown core exceptions, checked pause barriers,
teardown/detach witnesses and malformed/truncated/replayed wire input. This does
not qualify production cores, broker/AppContainer launch, physical devices,
whole-machine resource accounting or the full Windows acceptance matrix.
