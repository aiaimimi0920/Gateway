# Console persistence and journal ownership

## Starting checkpoint - 2026-09-11

Owner: the parallel coordinator. State: `structural_green`; paired validation
is terminal. Exclusive scope is `src/console/persistence.rs`,
`src/console/persistence/`, `src/console/journal.rs` and `src/console/journal/`.
S06 Rust/Gemini, the existing document/secret owners, and the original plan
cursor remain outside this write scope.

The official scanner records persistence at 2,071 effective lines and journal
at 913. Current strict inventory is 1,323 files, 34 hard, 61 mandatory and 40
soft; 95 files remain above 700. Source snapshots are preserved under
`target/effective-line-evidence/20260911-console-persistence-journal/`.

The structural boundary separates platform file operations, path/metadata
validation, transaction records, immutable archives, YAML replacement/recovery,
and journal framing/order/reconciliation. Public types remain at their current
paths, and state, writer guards, persistence ordering and failure semantics must
remain unchanged. New owners must stay below 500 effective lines without a new
exception; exact source comparison brackets the moves.

The paired gate is the two `console::persistence_lock_contract` unit tests and
the seven existing Console integration targets (134 tests). The lock tests
exercise one guard across preparation/replacement/phase persistence and the
durable-JSON/journal-append failure boundary. Final verification also includes
all-target compilation, scoped formatter, checker tests, ratchet and Git checks.

Scoped Cargo work is serialized after a current process census. This does not
claim a global source/docs freeze or the still-pending GWP-20260908-06 shared
release transfer. Existing immutable releases remain separate checkpoints.

## Verified result - 2026-09-11

Journal is now 305 effective lines; entry validation 128, history validation 166,
bounded storage/tail repair 175 and recovery 176. Its public types, configurable
append trait, limits and mutation orchestration remain at the original paths.

Persistence is now 316 effective lines. Children own physical paths (243),
metadata validation (120), durable file I/O (134), transaction records (325),
native replacement (151), first-save backup (131), immutable archives (352),
YAML replacement/recovery (339) and atomic transaction JSON (69).
Public types and shared state fields remain in the original module; associated
methods keep their signatures and visibility. `read_transaction_json` retains
its crate-level path through a re-export. No new soft exception is required.

Complete-source comparison passes for both roots and every child, with only
imports, module declarations, necessary private visibility and responsibility
comments added around unchanged bodies. The earlier 23 integration-test and 13
document/secret source hashes remain unchanged. State and writer-guard ownership,
same-directory native flags, flush/sync order, backup retention and fail-closed
recovery behavior were reviewed against the captured source.

Paired gates pass the two single-writer/durable-append failure tests and all 134
tests in the seven Console integration targets. Final all-targets compilation
exits 0; the journal-only intermediate library check also exits 0. No new warning
was introduced. The file-symlink test still returns early on Windows error 1314;
the link-rejection assertion is not counted as executed platform proof.

Scoped formatting, checker 19/19, ratchet, UTF-8/no-BOM, Gateway/Neuro whitespace
checks and the common development-standard contract pass. Strict remains red:
1,336 scanned, 33 hard + 60 mandatory = 93 above 700, 40 soft. This batch clears
two original entries, bringing structural progress to 52/145 (35.9%).

This is structural acceptance, not a new resource-hardening or full-release
claim. Independent review is not claimed: the required default-agent model was
already unavailable in the preceding attempts. S06 retains its two formatter
differences and production scope, and the release-window receipt is still pending.
Detailed evidence and commands are in
[the acceptance report](../../status/2026-09-11-console-persistence-journal.md).
