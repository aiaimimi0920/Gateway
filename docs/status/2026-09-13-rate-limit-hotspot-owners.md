# Rate-limit-hotspot ownership

Accepted: 2026-09-13 03:35:59 UTC. Gateway-only structural checkpoint.

The hotspot entry decreases from 1,812 effective lines to 337. It retains all
21 concrete public DTOs and their derives/serde/defaults. Eight private owners
separate audit queries, aggregation, anomaly reports, normal/anomaly snapshot
operations, snapshot aggregation, bucket metrics and shared snapshot storage.
The five original tests and three fixtures remain together. All ten scoped
source/test files are at most 397 effective lines.

| Source under src/db/rate_limit_hotspots/ | Effective lines |
| --- | ---: |
| Parent src/db/rate_limit_hotspots.rs | 337 |
| reports.rs | 96 |
| aggregation.rs | 179 |
| anomalies.rs | 236 |
| snapshots.rs | 158 |
| anomaly_snapshots.rs | 120 |
| snapshot_aggregation.rs | 108 |
| metrics.rs | 70 |
| snapshot_storage.rs | 167 |
| tests.rs | 397 |

Exact formatted-source proof preserves all 68 production definitions: 47
functions and 21 structs. All 32 public paths and seven crate-visible builder
paths remain available at the original entry. Only 18 private cross-owner
helpers gain pub(super). All five test bodies/attributes and three fixtures are
unchanged, as are 411 neighboring inputs including six caller files. Every
scoped source file is valid UTF-8 without BOM; existing Chinese diagnostics are
preserved exactly.

Query limits and filters, rate-limit classification, bucket sorting/tie-breaks,
time-window boundaries, threshold/profile precedence, concentration and delta
arithmetic, timestamps/UUIDs, snapshot object keys, object-store calls, malformed
object skipping and error text remain unchanged. This extraction adds no state,
allocation strategy, blocking work, background task or cleanup requirement.

| Gate | Baseline | Final |
| --- | ---: | ---: |
| Hotspot unit tests | 5/5 | 5/5 |
| Anomaly-incident caller tests | 6/6 | 6/6 |
| Remediation caller tests | 21/21 | 21/21 |

All paired test identities and warning sets match. Fresh locked offline
all-targets, scoped rustfmt, checker 19/19, ratchet, exact source/encoding proof,
terminal Cargo cleanup and both Git diff checks pass. These unit gates do not
claim live PostgreSQL/object-store or packaged runtime validation.

The first patch transport was rejected before apply because its guard required
a final newline after the patch terminator. The existing generator omits that
newline. patch-transport-attempt1.json preserves this tooling correction; the
reviewed patch, projection and source scope were unchanged when it was applied.

Global rustfmt still reports only the two unchanged S06 runtime-mirror files.
Strict scans 1,668 files: 25 hard, 27 mandatory and 40 soft. There are 52 files
above 700, down from 53; clearance is 93/145 (64.1%).

Immutable evidence:
target/effective-line-evidence/20260913-rate-limit-hotspot-owners/scope.json,
SHA-256 a3b29ee08425412ff7125d470cf3e55ede650a2e1341847d82e814e1b851ccc4.

At source acceptance, Gateway HEAD is
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,854 status entries: 178 modified,
one unstaged deletion, two staged deletions and 1,673 untracked. Neuro HEAD is
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and 182
untracked. Inherited deletions and sibling work were preserved. Documentation
publication follows this immutable source snapshot.

Request-audit ownership is next. S06 retains its implementation/cursor;
GWP-20260912-01, runtime-profile governance and final freeze/build transfer remain
pending. No release, persistent deployment, dependency, baseline, exception or
checker-policy change was made. This checkpoint does not close the overall plan.
