# Actual AppContainer storage observations

Codex reviewed Antigravity's exact source 942cda6 test output and unchanged
inventory. Both real ignored OS cases ran (2 passed, 0 failed, 0 ignored), with
actual actor exit code zero before retirement. Each required all seven booleans:
default AC file creation denied; package directory creation, WRITE_DAC and
label lowering denied; private registry key creation and actual value writing
denied; explicit private scratch writable only in the assigned writable case.

The first case creates, writes, flushes and deletes a real scratch file. The
second uses a separate compiled readonly probe and withholds that scratch grant.
Both use the production launch/job/token/registry/filesystem boundaries, with no
weaker retry. Default object creation may return privilege denial 1314 under its
mandatory label; missing paths/arbitrary errors do not count as denial.

The earlier actual batch024 value-write observation was successful (field six
was zero), despite a medium mandatory label. Batch025's DENY ACE and batch026's
extra suspended bootstrap did not fix it. Final source removes mutation bits
from matching package ALLOW grants and protects inheritance in the exact fresh
profile namespace; the extra bootstrap was removed. User/broker/read permissions
remain. Raw failure logs, containing disposable local profile paths, remain in
the local QA workspace; this paragraph is a review, not a replacement raw log.

This qualifies only the exercised storage operations on this Windows host.
It does not prove physical quotas, production core/helper compatibility, USER32
initialization, the package-to-grant selection branch or full item-2 completion.
