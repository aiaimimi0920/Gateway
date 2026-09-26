# Browser-pool context ownership

Owner: resumed coordinator. Accepted at 2026-09-14 01:57:17.665 UTC.
Entry 10,074 -> 9,783; owner 305, fixture 67, new context/capacity tests 94/71,
shared fixture 47 and package contract 160 effective lines. Paired Node suites
pass 70/70; paired package tests pass 1/1. Source/syntax/checker/ratchet and both
Git checks pass. All gates are terminal; storage/CDP fixture census is zero.

Move inspectContextEntryForReuse, createContextEntry, ensureContext, closeContext,
evictIdleContexts and reserveContextCapacity into one adjacent context owner.
Keep all function bodies with indentation-only changes. The factory owns both
context registries and captures existing root logger/predicate/constants/profile
callbacks once. Keep root public inspector export, connected-client ownership,
health count, eviction timer and caller busy/lease/lastUsedAt protocol unchanged.
No startup I/O, new cleanup policy, callback scheduling or browser process is added.

Pre-extraction tests protect creation deduplication, adoption, stale replacement,
late close-event identity, removal-before-await, failing/idempotent disposal and
capacity/idle behavior. The separate real CDP gate validates failed creation and
successful creation/reuse/disposal while the original owned browser/page stays
usable. Test-created browser handles/user-data are cleaned; no external browser
or profile is used. Package inclusion adds one support-file path only.

Evidence: target/effective-line-evidence/20260914-browser-pool-context-owners/scope.json.
SHA-256: 5341b8c4caa7be4bcccc0ab51edf5eea4cb6d080f93da55a635381837ab29d42.
Predecessor correctness scope: 2d62f639510281f90d1d6e7cd3a6b5b299549f28a086b99eafbc76cbe2b6653a.
Union 708; unchanged neighbors 705. Strict: 1,865 scanned; 16 hard, 24 mandatory,
40 soft; 40 above 700. Clearance stays 105/145 (72.4%). S18/release remain open.
[Checkpoint report](../../status/2026-09-14-browser-pool-context-owners.md).

Next proposed scope is app page preparation/auth/consent. Its definitions and
authoritative measurements must be read by main before edits. S06 and final
build transfer stay reserved under GWP-20260912-01.
