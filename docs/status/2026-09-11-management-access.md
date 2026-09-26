# Credential, access and internal Gateway management extraction

Date: 2026-09-11. Owner: parallel coordinator. State: `structurally_verified`.
Scope: [management access lane](../plan/parallel-lanes/management-access.md).

## Source result

| Entry | Before effective lines | After effective lines |
| --- | ---: | ---: |
| `src/http/routes/internal_provider_credentials.rs` | 1,005 | 34 |
| `src/http/routes/internal_access.rs` | 825 | 34 |
| `src/http/routes/internal_gateway.rs` | 945 | 60 |

All 25 scoped source/test files are at most 302 effective lines. Credentials
delegate to listing, mutation, folder synchronization, quota, route-config
fallback, public views, display metadata and payload normalization owners.
Access delegates to catalog, bundles, access keys and routing diagnostics.
Internal Gateway delegates to project records, project access, user credentials,
credential caching and usage reporting. The central management admission
functions remain in the original Gateway entry. Public paths remain explicit
re-exports; PostgreSQL guards remain in their entries.

The 17 production children range from 78 to 302 effective lines. Existing tests
remain under their respective entries, alongside two new metadata contracts and
the new 262-line management admission integration target. No effective-line
exception or baseline regeneration was needed.

Per-file counts, SHA-256 hashes, exact original snapshot hashes, source proofs and
independent repository states are captured in
[`scope.json`](../../target/effective-line-evidence/20260911-management-access/scope.json).
The three `*.before.rs` sources and `extract-management-access.mjs` preserve the
reproducible comparison inputs. The manifest refuses replacement on rerun.

## Preservation proof and review

The controlled comparison covers 34 credential items, 46 access items and 43
internal Gateway items, plus five retained helpers. Eight existing tests and two
test helpers retain their complete bodies. Expected source permits explicit
imports/re-exports, necessary restricted visibility and official Rust formatting.
The only production-body relocation is the credential quota loader's path from
`super::internal_provider_accounts` to `super::super::internal_provider_accounts`;
it resolves to the same payload-normalization helper.

Review covered every new owner and the three entries:

- Management authentication still precedes storage, cache, database and upstream
  work. Actor headers remain attribution behind that gate. Explicit management
  token precedence and bootstrapped Console authentication are preserved.
- Provider and user credential operations preserve owner/project/tenant scopes,
  SQL arguments, payload aliases, masking, cache TTLs, and mutation/deletion order.
- Cached quota and refresh paths retain their different I/O behavior, credential
  fingerprint rules, route-config fallbacks and display-model precedence.
- Access catalog, bundle/key changes and routing diagnostics retain endpoint-kind
  parsing, ordering, bounded collection and existing failure mappings.
- No task, lock, process, cancellation boundary or resource lifetime was added.
  Allocation patterns and algorithmic bounds remain those of the original code.

## Fresh verification

| Gate | Observed result |
| --- | --- |
| Original management unit baseline, including metadata contracts | 27 passed |
| Original real-router management integration baseline | 8 passed across two targets |
| Extracted management unit tests | 27 passed |
| Extracted real-router management integration tests | 8 passed across two targets |
| `cargo check --offline --locked --all-targets` | Passed |
| Scoped official Rust formatter check | Passed |
| Effective-line checker tests | 19 passed |
| Effective-line ratchet | Passed |
| Gateway and Neuro `git diff --check` | Passed independently |
| Scoped UTF-8 without BOM and trailing-whitespace checks | Passed |

Metadata contracts passed before production extraction. They cover credential
fingerprints ignoring display metadata and JSON field order while distinguishing
credential material, plus display-model alias precedence and ordered,
case-preserving supported-model lists.

The new HTTP target checks 16 management reads, 33 valid mutation requests with
forged actors, three legacy user-credential aliases and eight database-gated
reads with valid management authentication. Positive requests deliberately avoid
live object storage and upstream quota refresh. These are real-router admission
contracts with PostgreSQL unavailable, not live database or provider tests.

Evidence directory: `target/effective-line-evidence/20260911-management-access/`.
Final logs are `final-unit.log`, `final-routes.log`, `final-all-targets.log`,
`scoped-fmt.log`, `global-fmt.log`, `checker-tests.log`, `ratchet.log`,
`gateway-diff-check.log`, `neuro-diff-check.log` and `strict-inventory.json`.

Global `cargo fmt --all -- --check` still reports only the reserved S06 files
`src/upstream/gemini_canvas_runtime_mirror.rs` and
`src/upstream/gemini_canvas_runtime_mirror_tests.rs`. Existing upstream warnings
remain outside this scope; no owned-file warning was reported.

## Overall plan and release boundary

Strict scans 1,392 files: 31 hard, 55 mandatory and 40 soft. There are still 86
files above 700; structural clearance is 59/145 (40.7%). Strict remains red for
that debt. This checkpoint completes the three selected entries only.

No release was built or modified. S06 retains its Rust/Gemini source and original
plan cursor. GWP-20260908-06 still needs an explicit source/docs freeze and shared
release-build transfer receipt. Remaining structural work, integrated release and
runtime/UI/Docker acceptance remain open.
