# Folder-sync deletion overlap

GWP-20260916-19 is verified. Delete-missing now receives the existing explicit intent
set and excludes exact or directory-covered normalized source paths using the shared
prefix matcher. Explicit deletion remains first and byte-exact. Observed paths,
archive/manual/pending eligibility, DB/Redis order, audit semantics and genuine failure
propagation remain unchanged. No ID copy, public API or global 404 suppression is added.

Library baseline/candidate: 116/116 passed and 12 ignored, all 128 identities/results exact.
Real PostgreSQL/Redis overlap baseline: 4 passed / 2 failed; candidate 6/6. Public
database suite 7/7 and deletion target 9/9. All-targets/fmt/checker 19/19/ratchet/Git
checks pass. Strict is 2179/11/20/39 with expected exit 1; all 31 above-700 files are
unchanged and clearance remains 114/145 (78.6%). Union 1884, neighbors 1879, assets 22.

Five owners are 136/213/295/131/206 effective lines. Six owned PostgreSQL/Redis
containers and volumes were removed across baseline/candidate/public phases; 45 prior
containers and all persistent services/releases were preserved. The pre-candidate scope
correction, failed external review routing and unrelated Neuro status additions are
recorded. Coordinator frozen-candidate review found no issue.

Next: DB row/account/aggregate-memory/path bounds, native deadlines/full drain/status,
pool fairness, filesystem races, Unix/macOS and full provider/UI/Docker/release
acceptance. S06 stays reserved; overall goal remains active.

Evidence: `target/effective-line-evidence/20260916-folder-sync-deletion-overlap/`.
Evidence key: `folder-sync-deletion-overlap`.
Scope SHA-256: `482401e6e17c6b3bd2f0402844b86d5cc15929a2f30b267b60dc0289ebc533e7`.
[Report](../../status/2026-09-16-folder-sync-deletion-overlap.md).
