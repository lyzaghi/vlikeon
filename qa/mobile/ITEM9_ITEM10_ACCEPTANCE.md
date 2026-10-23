# Mobile acceptance assignment — NOT EXECUTED

Codex owns source review, diagnosis, case design and fix decisions. Antigravity
may execute only a later explicitly released frozen batch with owned synthetic
data/devices and bounded evidence capture. This document is not an execution
request, a passed test or permission to access personal accounts/data.

| Case | Environment | Required observation |
| --- | --- | --- |
| A01 | Actual Android arm64 APK | Open Hub, normal navigation, native hello and game, then change tab/activity/renderer and verify native grant ownership |
| A02 | Android blocked native lane | Expire 16 replies while I/O stays blocked; subsequent ordinary work is refused without growing the queue; release I/O and verify admission returns |
| A06 | Android document lane | Time out picker/export work behind a blocked native operation; the shared 32-work budget remains held until real completion and busy errors retain the caller ID |
| A03 | Android native queue | Queue a mutation, navigate before it starts; obsolete command is cancelled, no late destination picker appears; already executing work is reported separately |
| A04 | Android lifecycle | Repeated hide/show/navigation while I/O blocks; maintenance backlog stays bounded and the final requested cleanup is not lost |
| A05 | Android SAF providers | Known/unknown length, revocation, stalled reads, picker cancel, partial export, low storage and process termination; retained committed saves remain recoverable |
| I01 | Actual iPhone/iPad + simulator | Build development resources only with development flag; unsigned simulator/device build; release rejects development/empty catalog; archive needs explicit team/classification |
| I02 | iOS native queue | Block lane, timeout/recycle renderer slots; native admission stays at 16; document replacement cancels old queued work and the media lane rejects expired work |
| I11 | iOS document lane | Block authorization and selected single-file I/O; retain 16 authorizations/4 single-file I/O slots until actual return, even across document reset; no stale queued import continues after invalidation |
| I03 | iOS Game Hub | `index.html#view` retains authority; query, sibling asset, another port, frame and ordinary browsing do not gain bridge authority |
| I04 | iOS rapid lifecycle | Suspend A, suspend B after A finishes/expires, then deliver A's late callbacks; B's identity/task remains intact; foreground/visibility gates hold |
| I05 | iOS stalled save | OS expiry ends exactly the owned background task even while native I/O blocks; no successful final-save claim; later callbacks cannot end a new task |
| I06 | iOS recovery | Missing assets, failed profile identity/open, committed load failure and repeated process termination produce one retry UI; browsing stays usable on profile failure |
| I07 | iOS website data | Establish normal/private cookies and local game save; cancel deletion, then confirm; normal tabs/data removed, private session and game data preserved |
| I08 | iOS bookmarks/history | Rename/remove, cancel clearing, confirm clearing, restart; malformed persisted URLs rejected, storage errors visible, private pages absent from automatic persistence |
| I09 | iOS WKDownload | Unknown length grows past bound between timer ticks: no oversized final share; timeout, denial and completion all use correct destination lifetime |
| I10 | iOS cancelled download | Stall cancellation callback; admission retained, new transfers cannot bypass 2 slots, no unlink while WebKit still owns destination; retire after callback |
| M01 | Both physical platforms | Audio interruptions/route changes, Bluetooth/USB controllers, touch with rotation, background kill/restore, low memory/storage and protected-device lock |
| M02 | Both physical platforms | Offline local library/play/save, real approved core/media matrix, sync cancellation/recovery, mobile Remote Play with actual network/device evidence |

For every case record frozen revision/file hashes, actual platform/SDK/device,
command, exit/status, raw logs, observable assertions and artifact digests.
Skipped/blocked/unavailable cases remain open; no synthetic passing result.
