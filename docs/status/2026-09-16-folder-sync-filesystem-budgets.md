# Folder synchronization filesystem budgets, 2026-09-16

The S09 filesystem resource-budget checkpoint is verified. Discovery, encoded
material reads and export serialization now have enforced finite budgets. The
staged baseline reproduced 13 failures; the identical candidate tests pass. Static
containment and normal synchronization contracts remain. The overall optimization
and release goal remains active.

## Enforced policy and behavior

| Resource | Production limit | Boundary |
| --- | ---: | --- |
| Encoded credential material | 32 MiB | Import read, export serialization/write and existing-export read |
| Relative source path | 512 Unicode characters | Reject before path use or folder-derived DB persistence |
| Descendant directory depth | 32 | Check before opening the next child directory |
| Entries per scan | 100,000 | Includes directories and non-JSON files; excludes the configured root itself |
| JSON files per scan | 10,000 | Check before retaining an additional result |

The byte ceiling matches ordinary credential/state object storage. The path limit
matches the existing DB source_path character capacity, avoiding silent truncation
of folder-derived identities. Directory/count limits are new explicit operational
policy, chosen to permit large credential trees while bounding retained paths and
open iterators. There is no new configuration/schema/dependency or bypass option.
The DB management-input truncation policy itself was not changed.

Discovery retains depth-first read_dir order and prior link/reparse protection.
It holds at most the root plus 32 descendant ReadDir iterators. Every entry counts
before descendant inspection; an over-budget tree returns an error, never a partial
successful list. Import reconciliation and stale deletion therefore cannot treat
omitted paths as missing credentials/files. Tests verify local stale files and
deleted_count remain untouched when discovery fails.

File metadata enables early oversized rejection before allocation/read. Actual
stream bytes are still counted, so a stale short length cannot bypass the ceiling.
The reader consumes at most one sentinel byte beyond its limit. The retained byte
buffer never exceeds the limit; geometric fallible capacity growth is capped by
the same budget. Existing import hashes continue to cover the exact raw bytes.

Export uses a bounded Write implementation with serde_json::to_writer_pretty.
Every serialization fragment is checked before buffer growth; ordinary output is
byte-identical to to_vec_pretty, including escaped strings and nested arrays.
Already-serialized data is checked again before directory creation or file access.
An existing export file is also read with the same byte limit; a size/allocation
failure is returned without replacing it. Ordinary non-capacity read-error fallback
retains the prior behavior and remains a separate reliability consideration.

Size and allocation failures leave the retry loop immediately. Ordinary transient
read/JSON failures retain five attempts and four 200 ms waits. New errors expose
only budget context, without payload content: provider_credential_folder_sync_scan_limit,
provider_credential_folder_sync_material_too_large and
provider_credential_folder_sync_material_allocation_failed. Path overflow uses the
existing provider_credential_folder_sync_path_invalid code. Serialization failures
now use a material-level message without the credential ID.

Public APIs, configured-root trust, raw-path checks, descendant checks, archived
filtering, expected-path handling, byte hashes, write/skip/delete counters, sync
metadata fields and full-list-before-deletion ordering remain. Production callers
always use the defaults; private lower-budget parameters support isolated tests.

## Regression and fresh verification

The accepted containment scope/publication, current source/test/assets, previous
library receipt/log/snapshot and both repositories' HEAD/status sets were checked
before edits. The predecessor's removed deletion/paths.rs was correctly excluded
from the new before snapshot.

The staged extraction baseline retains unlimited behavior behind private owners:
84 passed / 13 failed / 6 ignored, 103 total results, in 1.09 seconds. The candidate
passes **97/97**, zero failed, six ignored and 3038 filtered, in 1.07 seconds. All
90 previous test identities/results remain exact. All 13 new budget tests and their
fixtures are frozen between baseline and candidate; the existing filesystem test
source differs from its predecessor only by the new mod limits declaration.

The 13 new tests verify exact/excess JSON count, all-entry counting, directory
depth, fail-closed stale cleanup, exact/excess stream bytes, oversized metadata
rejection before Read, stale length hints, actual file-size enforcement, no mkdir
on oversized export, preservation of oversized existing files, exact pretty bytes,
512-character Unicode paths and limit-plus-one actual byte consumption. Filesystem
cases use real isolated files/directories; stream tests include a no-read witness
and a consumed-byte counter. No production credentials or external service data
are involved.

Fresh serialized gates pass:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <9 scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

Existing management-deletion integration passes **9/9**, in 0.31 seconds. Checker
tests pass 19/19. Both repositories' staged/unstaged git diff --check, source/asset
freezes, exact result comparison and terminal native-idle guard pass. No candidate
correction or successful native gate replay was needed. Existing all-targets
warnings remain: three library warnings and one lib-test warning, plus the
prebuilt-web-assets build notice. Final fixture inspection found no leaked
gateway-folder-containment-* temporary roots.

## Size and evidence audit

| Source under provider_credential_folder_sync | Before | After |
| --- | ---: | ---: |
| Root ../provider_credential_folder_sync.rs | 125 | 126 |
| filesystem.rs | 118 | 138 |
| filesystem/export.rs | 56 | 82 |
| export.rs | 65 | 61 |
| filesystem/material.rs | new | 114 |
| paths.rs | 159 | 167 |
| limits.rs | new | 42 |
| tests/filesystem.rs | 221 | 222 |
| tests/filesystem/limits.rs | new | 192 |

All 56 folder-sync Rust files remain at most 500 effective lines, maximum 460.
No exception is needed. Strict retains expected exit 1 for other legacy debt:
2157 scanned / 11 hard / 20 mandatory / 39 soft. All 31 files above 700 retain
their exact hashes/counts; clearance remains 114/145 (78.6%). Current source/test
union is 1862, unchanged neighbors 1853, unchanged web/Tauri assets 22.

Evidence: target/effective-line-evidence/20260916-folder-sync-filesystem-budgets/.
scope.json observedAt: 2026-09-16T00:34:29.160Z. Scope SHA-256:
01ebbd92ccf6919cde077a370d38072fb33f7f3dcffe6c173cc75809dd8cf146.
Baseline/candidate snapshots, test logs, gate receipts, scripts, design and review
dispositions are hash-bound. publication.json checks all five checkpoint documents
and exact repository deltas. Independent baseline, resource-boundary and wiring
reviews found no in-scope candidate defect; their limits and wording corrections
are retained in reviews.md.

After publication, Gateway retains 194 modified, one unstaged deletion, two staged
deletions and 2395 untracked entries, including three new source files and two new
documents. Neuro retains 10 modified and 182 untracked. HEADs remain
4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit, dependency, checker,
policy, baseline, exceptions, persistent-service or release changes occurred.

## Remaining optimization

These budgets bound the encoded filesystem material and individual discovery.
They do not establish a whole-process RSS ceiling: parsed JSON expansion, DB
credential listing/hydration and overlapping manual/watcher runs remain. The
existing async task still executes blocking filesystem operations; run-wide
admission, blocking-worker ownership, cancellation, deadlines, total multi-file
duration and backend shutdown need further work.

Static pathname checks still do not prevent concurrent directory replacement or
hard-link aliasing. Successful PostgreSQL import/export and provider input coverage,
Unix execution, Redis/runtime/UI/Docker and native-release acceptance were not
completed by this lane. Other 31 oversized files and shared status priority remain.
S06 source/cursor/final native build stays reserved under GWP-20260912-01; target
4200 and Neuro/release/Gateway remain unchanged. The full goal stays active.
