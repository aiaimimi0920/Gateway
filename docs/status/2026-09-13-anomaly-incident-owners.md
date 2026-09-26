# Anomaly-incident ownership

Accepted: 2026-09-13 04:54:44 UTC. Gateway-only structural checkpoint.

The anomaly-incident entry decreases from 2,933 effective lines to 274. All 11
public DTOs and three shared private types retain concrete ownership at the entry.
Private fields remain private. Eleven implementation owners separate sync flows,
query/inventory reads, follow-up actions, alert recording, history, each anomaly
family's persistence, normalization and escalation policy. Six original tests
remain together; every scoped file is at most 456 effective lines.

| Source under src/db/anomaly_incidents/ | Effective lines |
| --- | ---: |
| Parent src/db/anomaly_incidents.rs | 274 |
| synchronization.rs | 348 |
| queries.rs | 183 |
| summary.rs | 76 |
| follow_up.rs | 257 |
| alert_dispatch.rs | 93 |
| history.rs | 82 |
| provider_persistence.rs | 422 |
| hotspot_persistence.rs | 437 |
| export_persistence.rs | 456 |
| normalization.rs | 44 |
| escalation.rs | 125 |
| tests.rs | 194 |

Exact formatted-source proof preserves 64 production definitions: 50 functions
and 14 structs. All 21 public paths, 18 SQL literals and six test bodies/attributes
are unchanged. Only 25 private cross-owner helpers gain pub(super), with two
test-only parent imports. All 443 neighboring inputs, including seven caller
files, are unchanged. Every scoped source is UTF-8 without BOM; Chinese diagnostics
and other strings are preserved.

Query limits/sorting, optional versus null-scope filters, fingerprints, sequential
write/history order, reopening rules, escalation decisions, owner precedence,
alert-delivery success rules, actor validation and timestamps are unchanged.
The extraction adds no task, blocking path, state or resource lifetime. Transaction
atomicity and extreme-value arithmetic remain outside this structural proof.

| Gate | Baseline | Final |
| --- | ---: | ---: |
| Anomaly-incident unit tests | 6/6 | 6/6 |
| Remediation caller tests | 21/21 | 21/21 |
| Hotspot dependency tests | 5/5 | 5/5 |

Paired test identities and warning sets match. Fresh locked offline all-targets,
scoped rustfmt, checker 19/19, ratchet, exact source/encoding proof, terminal Cargo
cleanup and both Git diff checks pass. Pure unit gates do not establish live
PostgreSQL, alert delivery or packaged runtime acceptance.

The first helper transport failed on nested newline escaping before apply. The
first baseline preflight was rejected by the Cargo/compiler/formatter idle guard
before any gate started. Both attempts are retained; a subsequent native process
observation found no matching processes without coordinator intervention. The
unchanged snapshot then passed all paired gates.

Global rustfmt still reports only the two unchanged S06 runtime-mirror files.
Strict scans 1,689 files: 23 hard, 27 mandatory and 40 soft. There are 50 files
above 700, down from 51; clearance is 95/145 (65.5%).

Immutable evidence:
target/effective-line-evidence/20260913-anomaly-incident-owners/scope.json,
SHA-256 e7a522d03019b00bb441dc52e8039c3440d4c38aa721050b32e09442e5895440.

At source acceptance, Gateway HEAD is
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d with 1,880 status entries: 179 modified,
one unstaged deletion, two staged deletions and 1,698 untracked. Neuro HEAD is
bf818f0324024634bc890585efb78cc8e603d11a with 192 entries: ten modified and 182
untracked. Inherited deletions and sibling work were preserved. Documentation
publication follows this immutable source snapshot.

Analysis-export ownership is next. S06 retains its implementation/cursor;
GWP-20260912-01, runtime-profile governance and final freeze/build transfer remain
pending. No release, persistent deployment, dependency, baseline, exception or
checker-policy change was made. This checkpoint does not close the overall plan.
