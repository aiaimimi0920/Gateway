# Request-audit ownership

Accepted: 2026-09-13 04:27:28 UTC. Gateway-only structural checkpoint.

The request-audit entry decreases from 2,064 effective lines to 407. All 23
concrete public DTOs remain at the entry. Eight private owners separate writes,
read queries, filters, provider summaries, sample analysis, provider-routing
analysis, prompt-cache reads and shared metrics. Private SQL/profile/accumulator
types move with their consumers, retaining private fields and method visibility.
The five original tests and their fixture remain together.

| Source under src/db/request_audits/ | Effective lines |
| --- | ---: |
| Parent src/db/request_audits.rs | 407 |
| persistence.rs | 181 |
| queries.rs | 150 |
| filters.rs | 122 |
| summary.rs | 210 |
| analysis.rs | 211 |
| provider_routing.rs | 298 |
| prompt_cache.rs | 273 |
| metrics.rs | 103 |
| tests.rs | 154 |

Exact formatted-source proof preserves 76 production definitions: 47 functions,
28 structs and one impl containing four methods. All 36 public paths and eight
raw SQL literals remain unchanged. Only 13 private cross-owner helpers gain
pub(super); four test-only imports are guarded by cfg(test). All five test
bodies/attributes and the fixture are unchanged, as are 432 neighboring inputs
including 22 caller files. Original CRLF bytes are retained in the before snapshot;
every scoped source is now valid UTF-8 without BOM with canonical LF hashes.

SQL text and bind order, identity validation, attribution, coalesce semantics,
saturating conversions, inclusive timestamp bounds, row limits, provider/model
window ownership, percentile rules, rounding and cost calculations are unchanged.
Prompt-cache bucket normalization retains its hour/day allowlist beside the SQL
construction. This extraction adds no state, task, blocking path or resource
lifetime. Extreme-value arithmetic hardening is outside this structural proof.

| Gate | Baseline | Final |
| --- | ---: | ---: |
| Request-audit unit tests | 5/5 | 5/5 |
| Hotspot caller tests | 5/5 | 5/5 |
| Documentation contract | 4/4 | 4/4 |

Paired test identities and warning sets match. Fresh locked offline all-targets,
scoped rustfmt, checker 19/19, ratchet, exact source/encoding proof, terminal Cargo
cleanup and both Git diff checks pass. These gates do not establish live
PostgreSQL or packaged runtime acceptance.

Global rustfmt still reports only the two unchanged S06 runtime-mirror files.
Strict scans 1,677 files: 24 hard, 27 mandatory and 40 soft. There are 51 files
above 700, down from 52; clearance is 94/145 (64.8%).

Immutable evidence:
target/effective-line-evidence/20260913-request-audit-owners/scope.json,
SHA-256 a7250be7e693108d6145f08f7e18414e71c8446f43fc90d4fd1a4742214824b4.

At source acceptance, Gateway HEAD is
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,865 status entries: 178 modified,
one unstaged deletion, two staged deletions and 1,684 untracked. Neuro HEAD is
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and 182
untracked. Inherited deletions and sibling work were preserved. Documentation
publication follows this immutable source snapshot.

Anomaly-incident ownership is next. S06 retains its implementation/cursor;
GWP-20260912-01, runtime-profile governance and final freeze/build transfer remain
pending. No release, persistent deployment, dependency, baseline, exception or
checker-policy change was made. This checkpoint does not close the overall plan.
