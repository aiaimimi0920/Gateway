# Folder-sync hydrated-account cache bounds

Verified on 2026-09-17. Folder import retains hydrated provider accounts in a dedicated
LRU cache with two independent limits: at most 128 accounts and at most 64 MiB of
conservatively estimated retained heap allocation. The estimator recursively charges
JSON value storage, string capacities, object and array allocation margins, endpoint
execution maps, account fields and cache bookkeeping. Arithmetic is saturating, so an
unrepresentable estimate is treated as over budget.

The import order remains classify, read material, select account, compare source hash,
then hydrate. Cache hits return a shared account view without copying the payload and
refresh LRU order. Insertion evicts the oldest accounts until both limits hold. An
account whose estimate exceeds the entire byte budget is returned for the current
normalization and mutation but is not retained, so unusual data remains functional and
cannot pin the scan-level cache. Repeated files for an evicted or individually oversized
account may perform another database/object-storage read; this is the explicit bounded
memory tradeoff.

Three focused tests prove hit ordering, entry-limit LRU eviction, retained-byte eviction
and oversized-account non-retention. The complete folder-sync unit group passes 119/119
with 12 external-service tests ignored. The final source snapshot passes the 12/12 real
PostgreSQL/Redis database contract suite, including same-hash no-hydration, unselected
object account behavior, selected missing payload, exact database limits and deletion
preservation. All-targets, scoped formatting, checker 19/19, ratchet and Git diff checks
pass. Existing warnings and global formatting differences remain confined to the
reserved S06 scope.

The new cache owner is 224 effective lines; import is 148 and the coordinator root is
60. Ratchet scans 2,181 files and passes. Strict inventory remains expected-red with
11 hard, 20 mandatory and 39 soft entries. All three scoped source files are UTF-8
without BOM.

The cache limit controls scan-lifetime retained account views; it does not certify the
peak required to hydrate and normalize one selected account. Selected remote object
storage success and its single-object read bound remain open. Retained source/observed
path memory, native deadlines/full drain/status ordering, blocking fairness, filesystem
races, Unix/macOS behavior and complete release acceptance also remain open. S06 stays
reserved and the overall goal remains active.

Evidence: `target/effective-line-evidence/20260917-folder-sync-account-cache/`.
Scope SHA-256: `496dc8a9297e9688f35f604dd8c6fa8e1a2f8cc1adc5aae30024d7b00435eea5`.
[Report](../../status/2026-09-17-folder-sync-account-cache.md).
