# Analysis-export text policy

Owner: parallel coordinator. State: verified. Accepted: 2026-09-13 06:23:05 UTC.

Scope: src/db/analysis_exports/export.rs and its private text-mode tests. The
accepted owner is 285 effective lines. The row builder applies text_mode and
max_text_chars to flattened request/response text but publishes the original
request message text. The HTTP export and persisted JSONL share this row builder.

Freeze serialized row/JSONL tests before the fix. Cover none, default/explicit
redacted preview, explicit full mode, zero character budgets, Unicode character
limits and redaction before truncation. Keep message role/name/tool-call metadata,
ordering and the existing JSON shape. Reuse the existing text-policy function;
keep its regexes, defaults, character counting and flattened truncation flags.

Only the row builder may change behavior. Existing export/persistence/filter
bodies, SQL/bind order, database/object-store behavior and the other 476 inputs
remain unchanged. Reconstruct the original source by removing only the reviewed
fix and test wiring. Freeze the test file byte-for-byte between red and green.

Evidence: target/effective-line-evidence/20260913-analysis-export-text-policy/.
Previous accepted scope SHA-256:
efc99d6c686e6df0f3a1de4ede9e0e9ed3370c5d482150331a35c9ee7043a05a.
Require the expected red failures, six green regression cases, paired original
export tests, fresh locked offline all-targets, scoped formatting, checker 19/19,
ratchet/strict inventories, source/encoding proof, Cargo cleanup and both Git
diff checks. Every source/test owner must remain at most 500 effective lines.

Serialize Cargo and do not edit Rust during a gate. S06 retains its implementation
and cursor. GWP-20260912-01, runtime-profile governance and final freeze/build
transfer remain pending. No dependency, baseline, exception, checker-policy,
release or persistent deployment change belongs here. Pure serialization tests
do not establish live PostgreSQL/object-store or packaged runtime acceptance.

The frozen contract advances from one pass/five failures to 6/6. Its test file has
195 effective lines and 38 assertions, unchanged between phases. Original export tests pass
6/6 before/after with unchanged test identities and warning sets. The production
owner is 292 effective lines, including two test-wiring lines. Only the message
text transformation changes; five signatures, two public paths, four unrelated
bodies and 476 neighboring inputs remain unchanged.

Fresh all-targets, scoped formatting, checker 19/19, ratchet, source/encoding/
cleanup proof and both Git checks pass. The stalled observer and cleanup of only
its verified childless RTK helper are recorded; no Cargo process was terminated.
All owned Cargo gates are terminal. Strict scans 1,708 files: 22 hard, 27 mandatory
and 40 soft; 49 remain above 700. Clearance stays 96/145 (66.2%). Global formatting
still reports only the S06 pair.

Immutable scope SHA-256:
488306d5e882292e7c67e9285ef1d28e68dfeaae7d61362d992802007d2f5298.
Report: ../../status/2026-09-13-analysis-export-text-policy.md. Remediation ownership
is next; the overall plan and final release remain open.
