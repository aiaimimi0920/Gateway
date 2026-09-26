# Folder synchronization filesystem containment, 2026-09-16

The S09 static filesystem containment checkpoint is verified. Import discovery and
reads, export writes, stale cleanup and management file deletion now share one
relative-path and descendant-validation owner. The staged baseline reproduced nine
filesystem failures; the identical candidate tests pass. The overall optimization
and release plan remains open.

## Implemented boundary

Database source_path is validated before normalization. Rooted paths, traversal,
Windows device names, ADS/drive syntax, control characters, ambiguous whitespace
and trailing dots are rejected before directory creation or file access. Filesystem
names receive lossless UTF-8 validation instead of silently trimming or dropping
components. Ordinary nested paths and Windows separators remain supported.

The configured root is canonicalized and trusted, including a configured root
symlink/junction. Every existing descendant is checked with symlink_metadata;
symlinks, Windows reparse points and non-file/non-directory material are rejected.
The canonical existing ancestor must remain inside the trusted root. New export
paths may have missing components, but their existing ancestors must pass first.

Discovery uses an explicit depth-first iterator stack, preserving read_dir order
without recursive call-stack growth. A missing root or failed/redirected subtree
returns an error. Import never applies missing-credential reconciliation to an
incomplete listing. Stale cleanup obtains a complete successful listing before
deleting anything, then checks each target again through the shared deletion owner.
Disappeared stale files are treated idempotently and do not increment deleted_count.

JSON reads recheck the path before every attempt. Ordinary partial-write/JSON read
retries retain five attempts and four 200 ms waits; path-safety errors fail immediately.
Export checks before mkdir, before reading existing bytes and before writing.
Unchanged bytes remain skipped; exported hash, normalized path and metadata wiring
remain consistent. Raw-byte import hashes and pretty-serialized export hashes remain.

Intentional compatibility changes: invalid paths now fail closed, including
separator-only source_path values that previously fell back to a default name.
Non-UTF-8 or ambiguous filesystem names abort discovery. Path error codes remain
provider_credential_folder_sync_path_invalid / provider_credential_folder_sync_path_escape;
the escape message is now operation-neutral. Inspection errors identify inspection
rather than deletion. Payload serialization now precedes export directory creation.
Public management/run APIs, enabled admission, archived filtering and DB field
contracts remain; no dependency or new unsafe code was introduced.

## Real regression proof and fresh gates

Before editing, the accepted root-ownership scope/publication, all source/test and
asset hashes, library receipt/log/snapshot, both HEADs and exact Git status sets
were revalidated. Export filesystem operations were exposed as internal owners so
the tests exercise the same real I/O used by database export. The extraction
baseline intentionally retained the defects.

Baseline: 75 passed / 9 failed / 6 ignored, 90 total results. Candidate: **84/84**
passed / 0 failed / 6 ignored, 3038 filtered, finished in 1.10 seconds. All 78
previous identities/results remain exact; the 12 new Windows tests cover roundtrip,
unchanged export, traversal, raw-path ambiguity, external/internal directory links,
outside overwrite/mkdir, stale cleanup, ancestor replacement between discovery and
read, trusted root links, missing root and expected/non-JSON preservation. All new
test and fixture bytes remain identical between baseline and candidate.

Fresh serialized commands pass:

    cargo test --offline --locked --lib folder_sync -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 --config skip_children=true --check <11 scoped files>
    node --test scripts/tests/effective-code-lines.test.mjs
    node scripts/effective-code-lines.mjs --mode ratchet

Existing deletion integration contracts pass **9/9**, in 0.46 seconds. Checker
tests pass 19/19. Both repositories' staged and unstaged git diff --check pass.
Source/asset freezes, exact test-result comparison and terminal native-idle guard
pass. Successful native gates ran once; no candidate correction or gate replay was
needed. Existing all-targets warnings remain: three library warnings and one
lib-test warning, plus the prebuilt-web-assets build notice. Test fixtures left no
gateway-folder-containment-* temporary roots at final inspection.

Windows junction cases executed. The additional Unix leaf/dangling-link test is
compiled/run only on Unix and was not executed here. The tests use isolated random
temporary roots and fabricated JSON, without a database, Redis or real credentials.
The baseline safety failures never accessed production material.

## Size, evidence and repository audit

| Source under provider_credential_folder_sync | Before | After |
| --- | ---: | ---: |
| Root ../provider_credential_folder_sync.rs | 124 | 125 |
| filesystem.rs | 100 | 118 |
| filesystem/export.rs | new | 56 |
| export.rs | 98 | 65 |
| import.rs | 132 | 133 |
| layout.rs | 206 | 186 |
| paths.rs, moved from deletion/paths.rs | 118 | 159 |
| deletion.rs | 209 | 209 |
| tests.rs | 75 | 76 |
| tests/filesystem.rs | new | 221 |
| tests/filesystem/fixture.rs | new | 68 |

All 53 folder-sync Rust files remain at most 500 effective lines, maximum 460.
No exception is required. Strict retains the expected legacy-debt exit 1:
2154 scanned / 11 hard / 20 mandatory / 39 soft. All 31 files above 700 retain
their exact previous hashes/counts; clearance remains 114/145 (78.6%). Current
source/test union is 1859, with 1848 unchanged neighbors and 22 unchanged assets.
The moved deletion path owner is absent at its former location; future snapshot
unions must honor scope.json's absent list rather than resurrecting that old path.

Evidence: target/effective-line-evidence/20260916-folder-sync-filesystem-containment/.
scope.json observedAt: 2026-09-15T23:52:36.712Z. Scope SHA-256:
1fa27bf68cc39b6fcb786e879ef48373f9c48706e2b950598e042b3f7cbf848c.
Source snapshots, baseline/candidate logs, gate receipts, scripts, design and review
dispositions are hash-bound. publication.json validates all five checkpoint docs
and exact repository status deltas.

Independent baseline and candidate reviews covered real I/O, cleanup, static
containment and contract wiring. A scout used the wrong base for the previous log;
its result-comparison claim was not accepted. The coordinator compared the actual
78 previous identities/results using the exact evidence path. Extra direct-read,
dangling-directory and special-file tests remain coverage opportunities, not claims
of executed proof. Review limitations and wording corrections are in reviews.md.

After publication, Gateway retains 194 modified, one unstaged deletion, two staged
deletions and 2390 untracked entries. The source move/new files add three net paths,
and this checkpoint adds two documents. Neuro retains 10 modified and 182 untracked.
HEADs remain 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit, dependency, checker,
policy, baseline, exceptions, persistent-service or release changes occurred.

## Remaining work

These are static pathname checks. Concurrent directory replacement and hard-link
aliasing remain unresolved; no handle-relative/race-free containment is claimed.
The explicit traversal stack still has unbounded directory depth and open iterators;
file counts, payload sizes, synchronous I/O, deadlines and cancellation need further
work. PostgreSQL import/export success and provider-input coverage remain incomplete.

The earlier root-ownership checkpoint remains accepted at its sealed snapshot.
Redis/Docker/PostgreSQL/provider/runtime/UI validation and a native release were not
rerun here. Status priority, backend shutdown, the other 31 oversized files and full
product acceptance remain open. S06 source/cursor/final native build stays reserved
under GWP-20260912-01; target 4200 and Neuro/release/Gateway remain unchanged.
The overall optimization goal remains active.
