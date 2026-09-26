# Folder synchronization root ownership completion, 2026-09-16

The remaining S09 folder-sync root migration is complete. The root decreases from
3066 to 124 effective lines, a reduction of 2942, and retains the public API and
phase coordinator. Five production owners and seven test suites plus shared
fixtures contain the extracted responsibilities. All 50 Rust files in the complete
folder-sync source subtree, including the root, are now at most 500 effective
lines; the maximum is 460. The overall optimization and release plan remains open.

## Ownership and preservation

| File under src/provider_credential_folder_sync | Effective lines |
| --- | ---: |
| Root ../provider_credential_folder_sync.rs | 124 |
| import.rs | 132 |
| export.rs | 98 |
| filesystem.rs | 100 |
| accounts.rs | 277 |
| layout.rs | 206 |
| tests.rs | 75 |
| tests/classification.rs | 211 |
| tests/catalog_text.rs | 357 |
| tests/catalog_media_search.rs | 183 |
| tests/layout.rs | 307 |
| tests/normalization_web.rs | 460 |
| tests/normalization_chatgpt.rs | 275 |
| tests/normalization_canvas.rs | 347 |

The root retains direction, per-run counters, run_folder_sync_once and its explicit
deletion-aware entry, timestamp formatting and existing public exports. Import
and export each own their database/filesystem phase. Filesystem owns discovery,
retrying JSON reads and byte hashes. Accounts owns the three lookup indexes,
ranking and selection. Layout owns material descriptors, naming and export paths.
Shared counters stay with the coordinator; lookup and path types live with their
owners. Existing deletion/watch/status imports and public call paths remain valid.

All 27 original production functions are preserved: 24 move and three remain in
the root. Two types move with narrow parent-subtree visibility. Exact query/write/
deletion order, counters, Chinese error text and error kinds, archive filtering,
five-read/four-200ms-wait policy, account scores and stable label ties, service/
surface/legacy fallback, alias precedence, directory parsing and path construction
remain. No public API, authentication, payload, database or filesystem policy changes.

The 36 original root tests move into seven cohesive contract suites. Their names,
attributes, assertions and fixture values remain; indentation and official rustfmt
reflow account for source formatting changes. The two shared fixtures retain their
definitions and deletion tests still resolve tests::build_test_credential. No test
is deleted, added, ignored or weakened. The first exploratory count of 38 tests
included the two fixtures; the exact attribute census and accepted log establish 36.

Test module paths gain one suite segment. projection.json records an explicit
36-entry old-to-new bijection. The verifier checks all 78 emitted library results
through this mapping: 36 relocated paths and 42 unchanged paths/results, including
all six ignored tests. It does not compare suffixes alone. Whole-source projections
from the captured root verify every formatted parent/child byte; unchanged root
function bodies receive an additional direct comparison.

Combined size of this round's root and 13 new files is 3152, up 86 wiring/formatter
lines from 3066. Every completed owner is below 500 without an exception. The root
is cleared from the recorded structural debt inventory. No latency or allocation
improvement is inferred from this ownership change.

## Fresh verification and audit

Before editing, the accepted normalization scope/publication, source/test/assets,
library receipt/log/snapshot and both repositories' HEAD/status were revalidated.
Its unchanged accepted result supplies the before gate. The candidate freshly runs:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1

Result: 72/72 passed, zero failed, six ignored, 3038 filtered, finished in 0.18
seconds. All 36 mapped and 42 unchanged identities/results pass exact comparison.

Fresh serialized gates pass:

    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <all 14 scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

The exact formatter arguments are in scoped-fmt.json. Checker tests pass 19/19.
Source projection, UTF-8/no-BOM checks, both repositories' staged/unstaged Git
checks and final native-idle guard pass. No successful native gate was replayed.
No source/test fix was needed after freezing the candidate. Existing all-targets
warnings remain: three library warnings and one lib-test warning.

Strict exits 1 for other legacy debt: 2151 scanned / 11 hard / 20 mandatory /
39 soft. Files above 700 decrease from 32 to 31; every remaining oversized file
retains its exact hash/count. Clearance improves to 114/145 (78.6%). The captured
source/test union contains 1856 files, with 1842 unchanged neighbors and 22 unchanged
web/Tauri assets. All 50 folder-sync Rust files are freshly measured in scope.json.
Dependencies, checker, policy, baseline and exceptions remain unchanged.

Three accepted independent reviews cover exact I/O bodies, lookup/layout behavior
and test/projection evidence. An earlier I/O review substituted a historical commit
after failing to locate the requested baseline; it was not accepted. A fresh scout
read the exact absolute-path original and confirmed preservation. Other review
wording corrections (16 service keys, parent visibility and fixture/test counts)
are recorded without changing source. An optional proof import failed in the Node
REPL because process was unavailable; direct rtk proxy node proof succeeded. These
observations and dispositions are preserved in reviews.md.

Evidence: target/effective-line-evidence/20260916-folder-sync-root-ownership/.
scope.json observedAt: 2026-09-15T23:03:39.161Z. Scope SHA-256:
3512e8b7f83d8d5145ceff66489454e1e8404b316449028bc5651c2c3598fc01.
All captured inputs, scripts, projections, reviews and gate receipts are hash-bound.
publication.json validates these five final documents and exact repository deltas.

Gateway retains 194 modified, one unstaged deletion, two staged deletions and 2385
untracked entries, including this lane's 13 source files and two documents. Neuro
retains 10 modified and 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a respectively. No staging, commit, dependency,
profile, persistent service or release changes occurred.

## Remaining optimization and acceptance

Structural closure does not fix existing synchronous/unbounded filesystem reads,
recursive traversal or symlink policy, path composition containment, deletion
TOCTOU, I/O deadlines, shared status priority or backend shutdown. Dedicated
successful PostgreSQL import/export and provider input coverage remain incomplete.
The unchanged normalization gaps for Business, generic fallback and canonical
Codex still apply. These responsibilities are now independently reviewable owners.

The [normalization checkpoint](2026-09-16-folder-sync-normalization.md) remains
accepted at its own frozen snapshot. Earlier Redis runtime 10/10 is historical;
Redis/Docker/PostgreSQL/provider/runtime/UI tests and native release were not rerun
for this structural extraction. Full provider/runtime/UI/Docker/release acceptance
and the other 31 oversized files remain open. S06 source/cursor/final native build
stays reserved under GWP-20260912-01; target 4200 and Neuro/release/Gateway remain
unchanged. The overall optimization goal remains active.
