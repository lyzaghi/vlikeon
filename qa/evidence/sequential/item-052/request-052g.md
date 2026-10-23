You are test executor only for item 52. Codex alone owns fixes and acceptance. Do not edit source, scripts, settings or permissions. Do not attach/detach/format disks or respond to UAC.

Prerequisite: the owner-confirmed preparation must have produced full-disk-052/volume.json. If missing, stop without running.

Run exactly once:
node E:\V10\work\vlikeon-items-50-109-01a16780\run-052g.cjs

The runner builds the isolated native QA endpoint from a frozen copy, then validates the receipt and exact 256-MiB VHDX identity, NTFS label, non-system/non-boot disk, and mount. It creates only synthetic profile data and bounded owned filler on this small volume, checks mount capacity before every write, requires real ENOSPC, releases its own 1-MiB reserve, attempts an 8-MiB save that cannot fit, verifies original data, releases only its filler, and reopens/retries. Logs remain outside VHDX. It never fills a host drive, deletes user data, resizes or formats a disk. Await terminal status. Read runs/052g/result.json, physical-fault-result.json and relevant logs. Report exact failures; do not fix, retry, change policies or substitute another disk. This test does not establish power-loss or all-platform durability.

