# Analysis-export ownership

Owner: parallel coordinator. State: verified. Accepted: 2026-09-13 05:43:56 UTC.

Scope: src/db/analysis_exports.rs and private analysis_exports/ owners. The
complete 3,228-effective-line source, database reexports, management routes,
remediation consumer and desktop schema/contracts were inspected. Retain all
29 public DTOs, eight constants and three shared private structs at the entry;
move the private anomaly context with its consumer. Private fields stay private.

Separate query, metadata, cleanup, report, anomaly, diff, export, manifest,
record-view, filter, inventory, trend, threshold, normalization, text, message
and dataset ownership. Children import private helpers from their actual owner;
only original test helpers receive cfg-guarded entry bridges. Preserve all 81
functions, 33 structs, eight constants, 41 public paths and six original tests.

SQL/binds, fallback manifests, filter limits/order, nullable metadata semantics,
pinned/dry-run cleanup, object write/delete order, manifest hashes, report
arithmetic, regexes, diagnostics and resource lifetime remain unchanged.
The suspected raw request-message text bypass requires a separate behavioral
regression after structural acceptance; it is not part of this extraction.

Evidence: target/effective-line-evidence/20260913-analysis-export-owners/.
Original raw/canonical SHA-256:
3a1cad877f65dfe13b3db8c750c4e48054ec343247dff0f1aba151fe22a6fb67.
Paired export/incident/remediation tests bracket the extraction. Require fresh
locked offline all-targets, scoped formatting, checker 19/19, ratchet/strict
inventories, exact definition/SQL/type/test proof, UTF-8/no-BOM checks, terminal
Cargo cleanup and both Git diff checks. Every new/extracted owner is at most
500 effective lines. Unit gates do not establish live PostgreSQL, object-store
or packaged runtime acceptance.

Serialize Cargo and do not edit Rust during a gate. S06 retains its implementation
and cursor. GWP-20260912-01, runtime-profile governance and final freeze/build
transfer remain pending. No dependency, baseline, exception, checker-policy,
release or persistent deployment change belongs here.

The entry is now 488 effective lines; the eighteen child source/test files range
from 46 to 318. Exact proof preserves 122 production definitions, 41 public paths,
nine SQL literals, all 33 structs/eight constants and six tests/one fixture.
Thirty-six helpers gain pub(super), with seven cfg-guarded test imports. All 458
neighboring inputs, including nine caller files, are unchanged.

Paired export/incident/remediation tests pass 6/6, 6/6 and 21/21 with unchanged
identities and warnings. Fresh all-targets, scoped formatting, checker 19/19,
ratchet, source/encoding/cleanup proof and both Git checks pass. Rejected process
guards and the subsequent idle observation are preserved; none terminated an
external process. Cargo gates are terminal.

Strict scans 1,707 files: 22 hard, 27 mandatory and 40 soft; 49 remain above 700.
Clearance is 96/145 (66.2%). Global formatting still reports only the S06 pair.
Immutable scope SHA-256:
efc99d6c686e6df0f3a1de4ede9e0e9ed3370c5d482150331a35c9ee7043a05a.
Report: ../../status/2026-09-13-analysis-export-owners.md. Serialized export text
policy is the next bounded regression. The overall plan and final release remain
open.
