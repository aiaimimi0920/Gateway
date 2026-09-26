# Analysis-export text policy

Accepted: 2026-09-13 06:23:05 UTC. Gateway-only behavioral checkpoint.

The export row builder now applies the existing text policy to each structured
request message before returning the row. Previously only flattened requestText
and responseText were protected; requestMessages.text retained the original
content. Both API-row serialization and persisted JSONL consume this builder.

The fixed, synthetic-data contract reproduced five failures before the change:
none exposed message text, default preview exposed sensitive content, full and
preview bypassed message character limits, zero budgets retained message text,
and short previews skipped redaction/truncation of structured messages. The
explicit full-mode preservation case passed before and after the change.

| Gate | Before | After |
| --- | --- | --- |
| Frozen serialized row/JSONL contract | 1 passed, 5 failed | 6 passed, 0 failed |
| Original analysis-export tests | 6 passed | 6 passed |

The six regression cases cover none, default and explicit redacted preview,
unknown-mode fallback, explicit full, zero budgets, Unicode character limits,
redaction before truncation, message order and role/name/tool-call metadata.
The 195-effective-line test file is byte-identical between both phases; its
38 assertions inspect the API-row and JSONL representations. The original six
test identities and both warning sets are unchanged.

The production owner increases from 285 to 292 effective lines, including two
test-wiring lines. The fix adds one linear pass over the existing message vector
and reuses apply_text_mode; there is no new policy implementation, task, handle,
network call or resource lifetime. Each message keeps its schema and metadata.
None emits an empty message text, redacted preview sanitizes before truncation,
and full retains text subject to the existing character limit. Flattened text
construction and its truncation flags remain unchanged.

Exact reconstruction removes only the reviewed loop, mutable binding and test
wiring to recover the original owner. Five signatures, two public paths, four
unrelated bodies and 476 neighboring inputs are preserved. SQL/binds, manifest
construction, database/object-store ordering and other text/normalization helpers
are unchanged. Both scoped files remain UTF-8 without BOM and below 500 effective
lines.

Fresh locked offline all-targets, scoped formatting, checker 19/19, ratchet,
source/encoding proof, Cargo cleanup and both Git diff checks pass. The stalled
pre-fix process observer is recorded: only its identity-verified, childless RTK
helper was stopped. No Cargo process was terminated. Bounded native observation
confirmed an idle window before the fix; all owned Cargo gates are terminal.

Strict scans 1,708 files: 22 hard, 27 mandatory and 40 soft; 49 remain above 700.
Structural clearance remains 96/145 (66.2%). Global formatting still reports only
the unchanged S06 runtime-mirror source/test pair. No live PostgreSQL/object-store,
packaged runtime, full-provider or final product acceptance is claimed here.

Immutable evidence: target/effective-line-evidence/20260913-analysis-export-text-policy/scope.json.
SHA-256: 488306d5e882292e7c67e9285ef1d28e68dfeaae7d61362d992802007d2f5298.
At source acceptance, Gateway HEAD remains 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d
with 1,904 status entries; Neuro HEAD remains bf818f0324024634bc890585efb78cc8e603d11a
with 192. Existing dirty files and staged deletions are preserved.

Remediation ownership is next. Its initial inventory is 6,932 effective lines;
no remediation source has changed. S06 retains its implementation and cursor.
GWP-20260912-01, runtime-profile governance and final freeze/build transfer remain
pending. No dependency, baseline, exception, checker-policy, release or persistent
deployment change was made. The overall optimization plan remains open.
