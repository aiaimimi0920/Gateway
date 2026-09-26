# Folder-sync hydrated-account cache bounds, 2026-09-17

This incremental checkpoint is verified. Lazy account hydration from the database-bound
checkpoint remains, while scan-lifetime retention of parsed full account views now has
finite entry and conservative byte budgets. The overall optimization and release goal
remains active.

## Implemented boundary

`HydratedAccountCache` owns a per-import LRU with maximums of 128 entries and 64 MiB of
retained allocation weight. The weight includes the account struct, cache keys/order,
all account string capacities, optional strings, endpoint execution-mode map capacity,
and recursively traversed JSON arrays, objects, keys and strings. Fixed allocation and
container margins intentionally make the accounting conservative. Every addition uses
saturating arithmetic.

The cache returns `Arc<GatewayProviderAccountView>` so a hit does not clone the account
payload. A miss performs the existing `get_provider_account` hydration and inserts the
result after evicting least-recently-used entries until both bounds hold. An account
larger than the total budget is returned to the current caller without being cached.
This preserves normalization and mutation for that file while preventing long-lived
retention. Later material selecting an evicted or oversized account may hydrate it again.

Source-hash skipping still happens before cache lookup. Unselected accounts still do
not hydrate, selected missing payloads still fail closed, and deletion phases retain
their previous database-snapshot and ordering behavior. Cache code emits no account ID,
payload, token, cookie or other credential material.

## Verification

Fresh final-source gates:

    cargo test --offline --locked --lib provider_credential_folder_sync -- --test-threads=1
    cargo test --offline --locked --test provider_credential_folder_sync_database -- --ignored --test-threads=1
    cargo check --offline --locked --all-targets
    rustfmt --edition 2021 <three scoped source files>
    npm run test:effective-lines --prefix scripts
    npm run check:effective-lines --prefix scripts
    git diff --check
    git diff --cached --check

The folder-sync library result is 119 passed, zero failed and 12 ignored. This includes
three new cache tests for LRU recency/entry eviction, byte-budget eviction and oversized
non-retention. The real guarded PostgreSQL/Redis target passes 12/12 on the final source
snapshot. Its two uniquely labelled containers were removed in `finally`, and the exact
ownership label reports zero remaining containers.

All-targets passes with the three pre-existing Gemini/S06 warnings; the library test has
one of those warnings. Checker tests pass 19/19 and the ratchet passes. The strict audit
has the expected exit 1 from inherited debt, with 2,181 files, 11 hard, 20 mandatory and
39 soft entries. Global formatting remains expected-red only for the two reserved S06
runtime-mirror files; scoped formatting passes.

## Size and scope

| File | Effective lines |
| --- | ---: |
| `src/provider_credential_folder_sync.rs` | 60 |
| `src/provider_credential_folder_sync/account_cache.rs` | 224 |
| `src/provider_credential_folder_sync/import.rs` | 148 |

All scoped files are below 500 and UTF-8 without BOM. Gateway HEAD remains
`4a7aece6b6531bb97b71afc99ae9e1ad3c4f2d7d`; Neuro HEAD remains
`bf818f0324024634bc890585efb78cc8e603d11a`. Existing dirty worktrees are preserved.
No dependency, policy, baseline, exception, staging, commit, release or persistent
service state changed.

## Remaining work

The cache bound controls retained hydrated accounts between files. It does not prove a
hard peak for the one full account currently being read and normalized, and remote
object-storage success/read limits remain unverified. The next aggregate-memory boundary
is retained source and observed path bookkeeping.

Native operation deadlines, complete shutdown drain, status ordering, blocking-pool
fairness, concurrent replacement, hard-link/reparse and cross-process races, Unix/macOS
behavior and complete provider/UI/Docker/release acceptance remain open. S06 source,
cursor and final native-build ownership remain reserved. Port 4200 and
`Neuro/release/Gateway` were not touched.

Evidence: `target/effective-line-evidence/20260917-folder-sync-account-cache/`.
Scope SHA-256: `496dc8a9297e9688f35f604dd8c6fa8e1a2f8cc1adc5aae30024d7b00435eea5`.
[Lane](../plan/parallel-lanes/folder-sync-account-cache.md).
