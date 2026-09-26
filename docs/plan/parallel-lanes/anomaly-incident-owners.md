# Anomaly-incident ownership

Owner: parallel coordinator. State: verified. Accepted: 2026-09-13 04:54:44 UTC.

Scope: src/db/anomaly_incidents.rs and private anomaly_incidents/ owners. The
complete 2,933-effective-line source, database reexports, management routes and
remediation sync caller were inspected. Keep all 11 public DTOs and three shared
private types at the entry; their fields remain private. Separate synchronization,
queries, summaries, follow-up actions, alert recording, history, provider/hotspot/
export persistence, normalization and escalation policy. Preserve six tests.

Keep all 50 functions, 14 structs and 21 public paths. Preserve SQL/bind order,
query limits and sorting, null-scope matching, fingerprints, sequential writes and
history append order, reopening/escalation decisions, owner precedence, delivery
success rules, actor validation, timestamps and diagnostics. Only cross-owner
helpers gain pub(super). Atomicity and extreme-value arithmetic changes require
their own behavioral evidence and are outside this extraction.

No test/script implementation reader was found. Paired incident/remediation/
hotspot tests bracket the extraction. Require fresh locked offline all-targets,
scoped formatting, checker 19/19, ratchet/strict inventories, exact definition/
SQL/type/test proof, UTF-8/no-BOM checks, terminal Cargo cleanup and both Git diff
checks. Every new/extracted owner must be at most 500 effective lines. Pure unit
gates do not establish live PostgreSQL, alert delivery or packaged runtime proof.

Evidence: target/effective-line-evidence/20260913-anomaly-incident-owners/.
Original raw/canonical SHA-256:
e73b1de89268c1d337b8888db5db716f680751509f40b77a1e6ac29b7d4898b9.
Serialize Cargo and do not edit Rust during a gate. S06 retains its implementation
and cursor. GWP-20260912-01, runtime-profile governance and final freeze/build
transfer remain pending. No dependency, baseline, exception, checker-policy,
release or persistent deployment change belongs here.

The entry is now 274 effective lines. The twelve child source/test owners range
from 44 to 456. Exact proof preserves all 64 production definitions, 21 public
paths, 18 SQL literals, 14 parent types and six test bodies/attributes. Twenty-five
helpers gain pub(super), including two cfg-guarded test imports. All 443 neighboring
inputs, including seven caller files, are unchanged.

Paired incident/remediation/hotspot tests pass 6/6, 21/21 and 5/5 with unchanged
identities and warning sets. Fresh all-targets, scoped formatting, checker 19/19,
ratchet, source/encoding/cleanup proof and both Git checks pass. The rejected
helper transport and pre-baseline process guard are preserved; neither changed
Rust. A later native observation found zero processes without intervention.

Strict scans 1,689 files: 23 hard, 27 mandatory and 40 soft; 50 remain above 700.
Clearance is 95/145 (65.5%). Global formatting still reports only the S06 pair.
Immutable scope SHA-256:
e7a522d03019b00bb441dc52e8039c3440d4c38aa721050b32e09442e5895440.
Report: ../../status/2026-09-13-anomaly-incident-owners.md. Analysis-export ownership
is next. The overall plan and final release remain open.
