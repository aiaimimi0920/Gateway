# Folder synchronization deletion containment, 2026-09-16

The S09 public file-deletion boundary now rejects static path escapes. Native
Windows regression, preservation and focused closing gates pass. Concurrent
filesystem replacement, full root migration and runtime/release acceptance remain open.

## Repair and behavior

The inherited implementation normalized away rooted prefixes and whitespace while
retaining parent components, then joined the result under the root without checking
containment. Real temporary-filesystem baseline tests demonstrated outside-file
removal through parent traversal and Windows directory junctions.

The public delete_synced_credential_file signature and re-export remain unchanged.
Its private paths owner validates raw input before normalization, rejects rooted,
drive/UNC, dot/parent, ADS/colon, device and ambiguous names, canonicalizes the trusted
configured root, checks every descendant including the leaf with symlink_metadata,
rejects symlinks and Windows reparse points, and compares canonical paths using
Path::starts_with. Deletion uses the checked pathname, not a canonicalized link target.
In-root link aliases are deliberately rejected too.

Ordinary slash/backslash, repeated and mixed separators remain supported. Disabled
runtime and empty/all-whitespace inputs remain noops. Missing roots, components or
files return false; unlink NotFound also returns false instead of an exists/remove
race error. Other filesystem errors propagate. This exposes non-NotFound errors
previously suppressed by exists(). No dependency or runtime profile changed.

Invalid paths return structured bad-request errors. Management callers propagate
those errors before DB credential deletion. Legacy unsafe source paths may therefore
block deletion. Account deletion already proceeds incrementally; an error may leave
previous credentials deleted. This lane does not alter that transaction policy.

Only public file deletion, its imports and private module wiring changed in the
existing deletion owner. Exact source reconstruction preserves the hit type and all
DB/runtime-key/counter/audit/watcher logic. The 5975-line parent, 271-line original
deletion tests, 227-line status owner and existing fixtures are byte-identical.

Effective lines: deletion owner 219 -> 209; new paths owner 118; integration tests
167; fixture 84. All completed changed/new owners are below 500. The root remains
legacy debt and no extra cleared file is claimed. Clearance stays 113/145 (77.9%).

## Fresh verification

Both frozen runs used GATEWAY_PREBUILT_WEB_UI=1 with offline, locked Cargo:

    cargo test --offline --locked --test provider_credential_folder_sync_deletion -- --test-threads=1
    cargo test --offline --locked --lib folder_sync -- --test-threads=1

Windows filesystem baseline: 2 passed / 7 failed. The same byte-identical test and
fixture files pass 9/9 after the fix. Seven failing groups cover slash/backslash and
nested parent traversal, external and in-root directory junctions, rooted input and
ambiguous names. Two preservation groups cover valid deletion/separators and
empty/missing/disabled noops. Baseline failure assertions checked real victim files;
no live provider, database or Redis service was used.

Paired original folder_sync: 45/45 with 3038 filtered out; exact test identities,
assertions and fixture bodies remain. The Unix-only live/dangling leaf-symlink test
is checked in but was not executed on this Windows host. Windows junction tests
ran without skips. Fresh temporary fixture residue count is zero.

All-targets cargo check, scoped official rustfmt --check, checker tests 19/19,
ratchet, and both staged/unstaged Gateway and Neuro Git checks pass. All native
phases are terminal and serialized. Warning sets match within each baseline/candidate
family; inherited Gemini unused imports/dead code remain. No failed candidate or
executed-fixture correction occurred. The fixture Config root type was corrected
before the first frozen baseline, and its official formatter ran before execution.
Two later documentation/evidence batches partially applied, then stopped on incomplete
board/publication line context; exact-line recovery completed both. The record is
documentation-recovery.md. Accepted sources, tests and native receipts were unaffected.

Strict remains exit 1 for existing debt: 2103 scanned / 12 hard / 20 mandatory /
39 soft. Above-700 membership and counts remain exactly 32. The parent remains 5975.

## Scope and evidence

Evidence: target/effective-line-evidence/20260915-folder-sync-deletion-containment/.
The evidence directory retains its initiation date; this checkpoint closed after
local midnight on 2026-09-16. Before capture revalidated the preceding deletion
ownership checkpoint, 1805 inputs, 22 assets, five docs and exact Git states.
Final union: 1808 inputs, 1804 unchanged neighbors and 22 unchanged web/Tauri assets.
Source, test, fixture, logs, receipts, inventories, process guards and scripts are
hash-bound; baseline and accepted snapshots are write-once. Source/scripts/docs
are UTF-8 without BOM. Independent review found no introduced candidate defect.

scope.json observedAt: 2026-09-15T16:19:07.849Z.
Scope SHA-256: 89bf5e4fa0e267c63c5b39349d9d7be9ecef88de61bc16ab19aa9a63e3990de7.
Source SHA-256 values:

- deletion.rs: 8a07e039ae6109d6ea071cb8631ebe47deeef0a18e41a7fca2b7c37dd387f12c.
- deletion/paths.rs: 5c5a3b1fe43ac7dde8e1abbc219fcfbbaeea30f202ce22a996e777e0e2416ead.
- integration tests: 473d644e5100198570b8bf5d7347556b47c124361d925e41b762f3cc8cf55bdb.
- fixture: 816a155c0a8e21933a812becbcdef80ff6379c5de000696fabb921c1bcc2cb26.

publication.json verifies final docs, source/evidence integrity, script limits and
exact repository deltas. Final expected Gateway status: 193 modified, 1 unstaged
deletion, 2 staged deletions, 2317 untracked. Neuro stays 10 modified, 182 untracked.
HEADs remain 4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d and
bf818f0324024634bc890585efb78cc8e603d11a. No staging, commit, service change or release.

## Remaining work

Canonical and metadata checks do not make deletion atomic. An adversarial writer
can replace a directory after checks; handle-relative or locked traversal remains
necessary for a race-resistant guarantee. The API remains synchronous. The scoped
fix does not establish scan/import/export link safety or live DB/Redis semantics.

Next S09 work: watcher ownership and remaining 5975-line root migration; separately
reproduce shared-snapshot explicit/missing deletion behavior before changing its
counts or DB error policy. Strict/feature/language/provider/runtime/UI/Docker/release
gates remain open. S06 original source/cursor and final-build coordination remain
reserved under GWP-20260912-01. Persistent service target remains 4200 and the only
release root is Neuro/release/Gateway. Full optimization is still in progress.
