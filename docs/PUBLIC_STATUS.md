# Public status — 2026-10-23

Vlikeon Browser is the new public identity for the source project called
OmniPlay internally. Its positioning is a gaming superbrowser by Vlike.
Internal names and wire formats have not been migrated.

## GitHub publication

The source repository is public at
[lyzaghi/vlikeon](https://github.com/lyzaghi/vlikeon). English/Vietnamese project
overviews, the roadmap, contribution and security guidance, licensing boundaries
and transparent Open View neon assets are included. The profile README is
published at [lyzaghi/lyzaghi](https://github.com/lyzaghi/lyzaghi).

Private vulnerability reporting is enabled for the project. Commits use the
account's GitHub noreply address. Generated build directories, large upstream
checkouts and local data are excluded from Git. The source publication was
reviewed for secrets across reachable history before the first push.

Logo-asset checks cover PNG alpha/sRGB and ICO/ICNS decoding. Product build,
runtime, device, compatibility, security and release qualification remain pending.
No OSS grant application has been submitted or accepted.

## What exists

Authored browser integration, product surfaces, Rust platform/services, native
and web engine adapters, recipes, packaging source and acceptance cases.
Original source licensing is Apache-2.0 with upstream/per-file exceptions.

Product source development has resumed in the owner's 1–16 sequence, currently
on item 2, Windows runtime lifecycle and containment. Item 1 has
authored integration but remains unverified. Other source areas require
completion review; source presence does not close the roadmap.

## What has not been established

- Successful consolidated build, test run or clean-clone build.
- Browser, native core, game, device or end-to-end qualification.
- Measured performance, production security or installer/update qualification.
- Complete license audit or permission to distribute all selected combinations.
- Public release, established adoption, download metrics or maintainer community.

No evidence-based completion percentage exists. Historical estimates are not
live readiness claims.

## Implementation records

[`IMPLEMENTATION_STATUS.md`](IMPLEMENTATION_STATUS.md) records source authoring.
References to `out/` describe local ignored snapshots, not release downloads.
Materialization and hashing do not execute or qualify the product. The local
`out/presentation/item2-v40-checked-guest-pause-source` snapshot records checked
guest-pause/retirement and Windows actor termination source, following the
v38 audio sink-commit/device-control/presentation admission work. It is a
source-only copy, not a compiled or executed product.

[`COMPLETION_SEQUENCE.md`](COMPLETION_SEQUENCE.md) records current source work
and the remaining verification requirements for every numbered item.

Use evidence for feature, benchmark, user, audit and compatibility claims. Do
not use unearned passing badges, fabricate activity, backdate commits or imply
sponsorship. OpenAI program acceptance has not been established.
