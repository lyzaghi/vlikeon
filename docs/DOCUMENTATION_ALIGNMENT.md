# Sequential task 2 — documentation alignment

Reviewed on 2026-10-23 against source integration commit `35f3271`, its published
evidence and live GitHub demo metadata. This task changes documentation only.

| Requirement | Updated entry points | Evidence used |
| --- | --- | --- |
| Active development status | README.md, README.vi.md, ROADMAP.md, PUBLIC_STATUS.md | Current 1–109 owner sequence and committed source |
| Executed checks and explicit limits | Both READMEs, PUBLIC_STATUS.md, IMPLEMENTATION_STATUS.md, ITEM16_VERIFICATION.md | `../qa/evidence/sequential/item-001/acceptance.json`, batch exits and input equivalence |
| Separate demo from full browser | Both READMEs, ROADMAP.md, PUBLIC_STATUS.md, PLAYABLE_DEMO.md | Existing demo qualification record and GitHub prerelease metadata |
| Stable numbering and history | ROADMAP.md, COMPLETION_SEQUENCE.md, IMPLEMENTATION_STATUS.md | `OUTSTANDING_WORK.md`; old 17 product-area identifiers remain historical |
| Contribution instructions | CONTRIBUTING.md | Selected execution exists; full-product clean-clone/device qualification is still open |
| Retained outstanding work | OUTSTANDING_WORK.md and roadmap | Task 1 complete; task 2 closes documentation alignment; task 3 is next |

GitHub CLI read-only release query confirmed tag `v0.1.0-demo.1`,
`isPrerelease=true`, `isDraft=false`, published `2026-10-23T04:28:13Z`.
ZIP asset `VlikeonDemo-0.1.0-windows-x64.zip` has digest
`387a4cad310485df1948c31818621f34ef0de7f0f10acf56a38438260fc3e13a`, matching
`PLAYABLE_DEMO.md`. No demo was rebuilt or republished in this task.

Codex reviewed the documentation diff and matched current counts to the accepted
source evidence: 53 Rust, 13 JavaScript, six patch Python plus 13 release Python,
four ignored Windows cases, and 435 repeatable generated files. Repeated earlier
cases are not additional coverage. Historical journals retain failures and
unexecuted snapshot records, with explicit scope notices instead of rewriting
old observations as current passes. No new runtime tests are necessary for this
documentation-only change; no application qualification is inferred from it.
