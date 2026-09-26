# Request-audit ownership

Owner: parallel coordinator. State: verified. Accepted: 2026-09-13 04:27:28 UTC.

Scope: src/db/request_audits.rs and private request_audits/ owners. The complete
2,064-effective-line source, prompt-cache monitoring design, database reexports,
pipeline creation/finalization and management read callers were inspected.
Retain all 23 public DTOs at the entry. Separate persistence, read queries, query
filters, provider summaries, sample analysis, provider-routing analysis, prompt-
cache reads and shared metrics. Move private row/profile/accumulator types with
their consumers so fields and methods remain private. Keep the five existing
tests and single fixture together. Every owner must remain below 500.

Preserve all 47 free functions, four accumulator methods, 28 structs, the impl
block and 36 public paths. SQL text/bind order, identity validation, attribution,
numeric conversions, optional-field coalescing, timestamp validation, query
limits, provider/model window ownership, percentile rules, cache adoption/cost
calculations and diagnostics must remain unchanged. Only required cross-owner
helpers gain pub(super); test-only imports retain existing test identities.
Arithmetic hardening is separate from this structural proof.

Paired gates cover request-audit unit tests, hotspot caller tests and the existing
documentation contract. The prompt-cache design continues to name the retained
public entry, so no contract reader change is needed. Require fresh locked
offline all-targets, scoped formatting, checker 19/19, ratchet/strict inventories,
exact source/SQL/type/test/fixture proof, UTF-8/no-BOM checks, terminal Cargo
cleanup and both Git diff checks. Pure gates do not claim live PostgreSQL or
packaged runtime validation.

Evidence: target/effective-line-evidence/20260913-request-audit-owners/.
Preserve original CRLF bytes separately from canonical LF hashes. Serialize
Cargo and do not edit Rust while a gate runs. S06 keeps its implementation/cursor;
GWP-20260912-01, runtime-profile governance and final freeze/build transfer remain
pending. No dependency, baseline, exception, checker-policy, release or persistent
deployment change belongs here.

The entry is now 407 effective lines. The other owners are persistence 181,
queries 150, filters 122, summary 210, analysis 211, provider_routing 298,
prompt_cache 273, metrics 103 and tests 154. All ten scoped files remain below
500. Exact proof preserves 76 production definitions, 36 public paths, eight SQL
literals, 23 parent DTOs, five private structs, the impl/four methods, five tests
and one fixture. Thirteen helpers gain pub(super), with four test-only parent
imports. All 432 neighboring inputs, including 22 caller files, are unchanged.

Paired unit/hotspot/documentation gates pass 5/5, 5/5 and 4/4 with identical test
identities and warning sets. Fresh all-targets, scoped formatting, checker 19/19,
ratchet, source/encoding/cleanup proof and both Git checks pass. Strict scans
1,677 files: 24 hard, 27 mandatory and 40 soft; 51 remain above 700. Clearance is
94/145 (64.8%). Global formatting still reports only the unchanged S06 pair.

Immutable scope SHA-256:
a7250be7e693108d6145f08f7e18414e71c8446f43fc90d4fd1a4742214824b4.
The acceptance report is ../../status/2026-09-13-request-audit-owners.md. Anomaly-
incident ownership is next; final release and the overall plan remain open.
