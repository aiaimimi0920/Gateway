# Analysis-export ownership

Accepted: 2026-09-13 05:43:56 UTC. Gateway-only structural checkpoint.

The analysis-export entry decreases from 3,228 effective lines to 488. All 29
public DTOs, eight constants and three shared private structs retain concrete
ownership at the entry. Their fields remain private. The private anomaly context
moves with its only consumer. Seventeen implementation owners separate persistence,
reports, projections and text/dataset preparation; the six original tests remain
together. Every scoped file is at most 488 effective lines.

| Source under src/db/analysis_exports/ | Effective lines |
| --- | ---: |
| Parent src/db/analysis_exports.rs | 488 |
| queries.rs | 167 |
| metadata.rs | 118 |
| cleanup.rs | 125 |
| reports.rs | 151 |
| anomalies.rs | 318 |
| diff.rs | 184 |
| export.rs | 285 |
| manifest.rs | 158 |
| record_views.rs | 83 |
| filters.rs | 114 |
| inventory.rs | 88 |
| trends.rs | 255 |
| thresholds.rs | 196 |
| normalization.rs | 113 |
| text.rs | 95 |
| messages.rs | 131 |
| dataset.rs | 46 |
| tests.rs | 223 |

Exact formatted-source proof preserves 122 production definitions: 81 functions,
33 structs and eight constants. All 41 public paths, nine SQL literals and six
test bodies/attributes plus one fixture are unchanged. Only 36 private helpers
gain pub(super); children import helpers from their owners, and seven original
test helpers have cfg-guarded entry imports. All 458 neighboring inputs, including
nine caller files and the desktop schema/contracts, are unchanged. Scoped sources
are UTF-8 without BOM; diagnostics and other string contents are preserved.

SQL/binds, fallback manifests, filter limits/order, nullable metadata updates,
pinned/dry-run cleanup, object write/delete order, manifest/hash construction,
report arithmetic and regex behavior remain unchanged. This extraction adds no
task, blocking path, resource lifetime or state. Source review identified a
possible text-policy bypass in requestMessages.text; its reproduction and fix
remain a separate behavioral batch.

Paired baseline/final gates pass export 6/6, incident 6/6 and remediation 21/21
with identical test identities and warning sets. Fresh locked offline all-targets,
scoped formatting, checker 19/19, ratchet, source/encoding/cleanup proof and both
Git diff checks pass. These unit gates do not prove live PostgreSQL, object-store
or packaged runtime behavior.

The pre-baseline and pre-format process-guard rejections are retained. Neither
rejected attempt started a Cargo gate. Read-only native observations recorded
existing Cargo/rustc processes and their later absence; none were terminated.
All owned Cargo gates are terminal.

Strict scans 1,707 files: 22 hard, 27 mandatory and 40 soft; 49 remain above 700.
Structural clearance is 96/145 (66.2%). Global formatting still reports only
src/upstream/gemini_canvas_runtime_mirror.rs and its unchanged S06 test companion.
This checkpoint does not complete the whole optimization plan.

Immutable evidence: target/effective-line-evidence/20260913-analysis-export-owners/scope.json.
SHA-256: efc99d6c686e6df0f3a1de4ede9e0e9ed3370c5d482150331a35c9ee7043a05a.
At source acceptance, Gateway HEAD remains 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d
with 1,901 status entries; Neuro HEAD remains bf818f0324024634bc890585efb78cc8e603d11a
with 192. Existing staged deletions and unrelated work are preserved.

The next bounded regression checks serialized row/JSONL text modes and character
limits. S06 retains its implementation and cursor. GWP-20260912-01, runtime-profile
governance and final freeze/build transfer remain pending. No dependency, baseline,
exception, checker-policy, release or persistent deployment change was made.
