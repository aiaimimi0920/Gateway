# Credential-pool automation ownership

Owner: parallel coordinator. State: structural_green. Started: 2026-09-12 UTC.
Accepted: 2026-09-12 20:58:38 UTC.

Scope: src/credential_pool_automation.rs and its private extracted owners. The
baseline entry measured 1,521 effective lines. Separate registry validation,
runtime state, credential inventory, archive persistence, scheduled/provider
actions, refill collection, reconciliation, driver execution and original tests.
Keep shared contracts and public entry paths in the parent. Every resulting owner
must remain at most 500 effective lines.

Preserve configuration defaults, tagged driver JSON, all public methods and
paths, field visibility, management guards, provider-specific locking, request
and prune validation, archive-before-prune/commit ordering, timeout/error text,
resource ownership and all original test bodies. Only family-local visibility
needed by the new ownership boundaries may change. Check callers in runtime,
refill delivery/demand and the management route.

Require paired original automation tests, adjacent refill units, fresh all-targets,
scoped formatter, checker 19/19, ratchet/strict inventories, exact production and
test/source/neighbor proof, UTF-8 without BOM and both repositories' Git checks.
Serialize Cargo; do not edit Rust during a running gate.

Review driver response accumulation and script-input timeout as a separate
hardening boundary after extraction. Archive filesystem races, blocking I/O,
scheduler shutdown and lock/state cardinality need independent evidence; this
structure-only lane does not claim they are solved.

S06 retains its implementation/cursor. GWP-20260912-01, runtime-profile governance
and final freeze/build transfer remain pending. No dependency, checker-policy,
baseline, exception, release or persistent runtime changes belong to this lane.

Evidence: target/effective-line-evidence/20260912-credential-pool-automation-owners/.

The entry decreases from 1,521 to 247 effective lines. All ten Rust owners are
at most 277. Exact comparison preserves 59 production items: 31 free functions,
19 structs/enums, five impl blocks and four constants. Fourteen public entry
paths, thirteen public runtime methods, all ten original test bodies and 286
neighboring inputs remain unchanged. Seventeen helpers and four private methods
gain only family-local visibility; all type fields and serde attributes remain
intact.

Paired automation 10/10 and refill 7/7 pass, including the original real-loopback
HTTP driver and temporary archive/purge tests. Fresh all-targets, scoped formatter,
checker 19/19, ratchet, exact source/encoding proof and both Git checks pass.
All Cargo gates are terminal. Strict scans 1,607 files: 29 hard, 29 mandatory and
40 soft; 58 remain above 700, with clearance 87/145 (60.0%). Global formatting
still reports only the unchanged S06 runtime-mirror files.

The immutable scope binds baseline/candidate sources, paired tests, gate logs,
inventories and independent repository observations. See the
[acceptance report](../../status/2026-09-12-credential-pool-automation-owners.md).
HTTP driver response bounds are the next focused regression; script I/O lifetime
and the other recorded lifecycle risks remain separate.
