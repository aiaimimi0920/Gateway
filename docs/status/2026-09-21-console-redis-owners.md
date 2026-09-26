# Console Redis storage extraction

Date: 2026-09-21. Status: structural_green; full-goal work remains active.

## Structure and preservation

`src/console/redis_store.rs` decreases from **688 to 440 effective lines**.
The parent keeps the Redis store, activation Lua, transaction phases, active
revision loading/validation, Cluster gate, outcome parsing and existing tests.
Two cohesive nested owners are introduced:

| Path under `src/console/redis_store/` | Effective lines | Responsibility |
| --- | ---: | --- |
| `types.rs` | 159 | Validated revision/key/error/outcome contracts and canonical JSON serialization |
| `immutable_json.rs` | 104 | Immutable Redis JSON storage and compare-and-set normalization |

All three files are below 500 effective lines. Private type fields stay private;
only internal factories/serialization/storage access becomes `pub(super)`.
Existing public paths are re-exported through the same parent. The dependency
direction is store -> immutable JSON -> types; types do not depend on the I/O
owner. No blanket helper module, new dependency or exception is introduced.

Every original function body and test body is preserved by exact formatted
projection. Both raw Lua strings are byte-identical to the baseline. The
independent read-only review found no extraction regression. Activation still
validates transaction phases/identity before CAS and publishes only on an
Activated result. Revision parsing still validates metadata and both document
digests. Immutable writes preserve GET/SET NX/re-read, JSON equivalence,
conditional normalization and conflict behavior.

## Measurements and verification

Evidence directory: `target/effective-line-evidence/20260921-console-redis-owners/`.
Saved baseline SHA-256:
`7c673d1a32da0da0537218e6f457f23218c548e95f426101eab29ed0558220a8`.
`extraction-proof.json` records source projection, Lua identity, encoding and
before/after counts.

Strict scans cover 2220 -> 2222 files. Hard/mandatory counts remain 10/18;
soft entries decrease **39 -> 38**. Strict still exits 1 with 28 files above
700. The only changed debt entry is `src/console/redis_store.rs`; no baseline,
policy, checker or exception file changed.

| Check | Result |
| --- | --- |
| Before: `cargo test --locked --lib console::redis_store::tests -- --test-threads=1` | 7 passed |
| After: same command | 7 passed |
| Before: `cargo test --locked --test console_transaction_contract -- --test-threads=1` | 4 passed |
| After: same command | 4 passed |
| `cargo check --locked --all-targets` | Passed; three inherited Gemini warnings |
| `npm run test:effective-lines --prefix scripts` | 19 passed |
| `npm run check:effective-lines --prefix scripts` | Passed |
| `rustfmt --edition 2021 --check src/console/redis_store.rs` | Passed, including new children |
| Exact formatted source projection and both raw Lua identities | Passed |
| UTF-8 without BOM | Passed |
| Separate Gateway and Neuro `git diff --check` | Passed |

Global formatting remains red on the unchanged folder-sync `paths.rs` and
two reserved Gemini runtime-mirror files; `global-format.log` records the
current failure. Scoped formatting passes. No global-format success is claimed.

The existing tests cover revision roundtrip/canonical ordering, immutable JSON
equivalence, digest/active-document mismatch, Cluster detection, activation
outcomes and externally visible namespace/key contracts. They do not establish
live Redis race or failover acceptance. No live Redis service was required or
changed by this extraction.

## Safety and lifecycle review

- Types retain namespace validation, metadata validation, digest matching,
  deny-unknown-fields serde behavior and error codes. Private credential-bearing
  documents remain behind the same getters; no new logging is introduced.
- Canonical serialization preserves the existing JSON value conversion and
  allocations; this move does not add a second serialization pass.
- Immutable I/O preserves the Lua compare-and-set and race re-read behavior.
  It does not add retries, tasks, locks or queues. Connections retain their
  caller-owned lifetime across the same awaits.
- Parent activation preserves key/argument order, legacy mirror handling,
  prepared/activated transaction comparison and publish timing. Cluster refusal
  is unchanged and must not be described as Cluster support.
- Existing payload-size/operation-deadline bounds, multi-read snapshot races
  and post-CAS publish failure semantics require separate hardening evidence;
  this source move neither repairs nor worsens those contracts.

## Whole-goal continuation

The previous goal turn made verified keepalive structural progress; this turn
clears one additional unapproved soft-limit source owner. The full objective
remains active under `2026-09-21-refactor-completion-audit.md`.

Next unreserved candidates are Console authentication (617), request-header
tests (518 parent) and rate-limit rule/key construction (516 parent). Scouts
identified cohesive boundaries, but exact source and tests must be re-read
before editing. S06 source/build transfer and runtime-artifact governance remain
unresolved; none is inferred from this checkpoint.

No release package, live deployment, profile edit, staging, commit or push was
performed. Inherited Gateway changes and the Neuro Gateway submodule state are
preserved; no sibling-project source was edited. The requested release root
remains `C:\Users\Public\nas_home\AI\GameEditor\Neuro\release\Gateway`.
