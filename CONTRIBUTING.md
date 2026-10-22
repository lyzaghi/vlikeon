# Contributing to Vlikeon Browser

Start with [public status](docs/PUBLIC_STATUS.md),
[architecture](docs/ARCHITECTURE.md) and [roadmap](ROADMAP.md).
The source has not passed consolidated execution or device qualification.

## Discuss a change

Use an issue for a concrete bug, design question or focused proposal once the
repository is public. Include the source path, expected behavior and whether
the finding comes from source review or actual execution. Do not label an
inferred defect as a reproduced failure without evidence.

Discuss substantial changes with the maintainer first. Small documentation
corrections can go directly to a pull request.

## Submit a focused pull request

1. Explain the problem and resulting behavior.
2. Keep ownership, permission and compatibility boundaries explicit.
3. Preserve upstream notices; document dependency and license changes.
4. State exactly what ran, on which platform and what remains unverified.
   If nothing ran, say so. Authored tests are not passing results.
5. Exclude unrelated changes, generated output, upstream checkouts and binaries.

There is no verified clean-clone build guide yet. The maintainer is authoring
source before one consolidated verification pass. Contributors who independently
run a check should report their environment and evidence; that does not establish
whole-product qualification.

## Licensing and private content

Intentional original contributions are offered under the applicable project
license, Apache-2.0 unless a file or component has another license. Submit only
work you are authorized to share. No ownership transfer is requested.
See [LICENSING.md](LICENSING.md).

Do not submit credentials, private keys, personal saves, commercial games or
manufacturer BIOS. Use synthetic fixtures and permission-cleared examples.
For vulnerabilities, follow [SECURITY.md](SECURITY.md).
