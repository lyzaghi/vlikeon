# Vlikeon Browser

**Gaming Superbrowser by Vlike**

An experimental Chromium-first browser project bringing a game hub, isolated
native engine adapters, save management and remote-play workflows into one
workspace. Vlikeon is the new public name of the project previously called
OmniPlay.

**Status: source development, not a verified release.** No qualified installer,
compatibility list or measured performance result is available. The source has
not undergone its consolidated build and verification pass.

[Tiếng Việt](README.vi.md) · [Roadmap](ROADMAP.md) ·
[Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) ·
[License scope](LICENSING.md)

## What we are building

- A browser and game hub with a consistent library and session workflow.
- Permission-scoped native runtime integration and explicit resource ownership.
- Save import, recovery and encrypted synchronization workflows.
- Remote-play coordination and desktop, mobile and web product surfaces.
- Reproducible source preparation with pinned dependencies and explicit
  distribution admission for third-party engines.

These describe intended behavior and authored source areas, **not verified
shipping features**. Desktop and Android target Chromium integration; iOS source
targets WebKit. Device, platform, compatibility and release qualification remain open.

## Start here

1. Read the [current status](docs/PUBLIC_STATUS.md) and [roadmap](ROADMAP.md).
2. Explore the source map below and [architecture](docs/ARCHITECTURE.md).
3. Read [CONTRIBUTING.md](CONTRIBUTING.md) before proposing a change.
4. Review [licensing boundaries](LICENSING.md) before distributing any build.

This is a source-review entry point. A clean-clone build has not been verified;
there is no supported end-user installation command yet. Tools and recipes are
authored source, not proof of successful execution. The maintainer is authoring
the implementation before one consolidated verification pass.

## Source map

| Directory | Responsibility |
| --- | --- |
| `browser/` | Chromium pins, reference snapshots, patches and integration |
| `apps/` | Game hub, web, Android and iOS product surfaces |
| `platform/` | Rust contracts, broker, storage and platform/runtime boundaries |
| `engines/` | Native, web and streaming runtime integration |
| `services/` | Account, encrypted synchronization and signaling services |
| `security/` | Dependency provenance, release trust and distribution decisions |
| `packaging/` | Platform packaging source and policies |
| `qa/` | Authored acceptance cases and test source, not executed evidence |
| `tools/` | Source acquisition, adaptation, staging and build recipes |
| `docs/` | Architecture, requirements and implementation records |

Large upstream checkouts, generated `out/` snapshots, local data and secrets
are excluded from Git. References to local snapshots are authoring records,
not downloadable releases.

## Contributing

Early contributions can help review lifecycle and containment boundaries,
document build prerequisites, review dependency provenance and improve focused
acceptance cases. Discuss substantial changes before implementation. Do not
upload commercial game assets, manufacturer BIOS, credentials or private saves.

We do not currently claim community adoption, production use, completed audits
or third-party endorsement.

## License and attribution

Original project material without a separate notice is offered under
[Apache-2.0](LICENSE). Existing MIT/ISC notices, Chromium-derived files and all
third-party components retain their own licenses. The root license does not
relicense upstream code or approve combined binary distribution. See
[LICENSING.md](LICENSING.md), [NOTICE](NOTICE) and
[the dependency inventory](security/dependencies.json).

Selected upstream sources include replacement firmware/fonts and embedded
assets requiring review; this repository does not grant rights to game content
or manufacturer BIOS.

Internal names, crate identifiers, protocol fields and paths still use
`omniplay` where changing them requires a separately reviewed migration.
The public naming change does not imply that migration has been completed.
