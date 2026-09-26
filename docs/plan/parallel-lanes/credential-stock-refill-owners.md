# Credential stock and refill ownership

Owner: parallel coordinator. State: structural_green. Started: 2026-09-12 UTC.
Accepted: 2026-09-12 19:47:40 UTC.

Scope: src/credential_stock.rs, src/credential_refill.rs and their new private
owner/test directories. Both complete source files, direct HTTP/runtime callers
and existing tests were read before selecting boundaries. The entries decrease
from 1,514/1,306 to 281/263 effective lines; all twenty Rust owners are at most 281.

Stock separates public/wire records, watermark evaluation, policy persistence,
status evaluation, quota field projection, signal orchestration/storage and input
validation. Refill separates public/private task records, demand projection,
notification scheduling, creation, claims, completion, delivery, Redis storage,
validation and timestamps. Keep state ownership and all existing execution order.

Preserve every public path, field visibility, serde attribute, production item,
SQL/Redis script and all original Rust test names/assertions. Only helpers needed by
siblings or the original tests may gain family-local visibility. New/extracted
owners must remain at most 500 effective lines. No behavioral hardening is folded
into the extraction.

Rust baselines pass stock 15/15 and refill 7/7. The Python wiring baseline fails
because routes already moved out of router.rs; its four literal UI status labels
also no longer exist. A separately reviewed source-contract correction reads the
actual route/creation owners and asserts the current endpoint/manual-refill
component bindings. Its original trigger, endpoint, management-access and stream
secret-exclusion assertions remain unchanged. No UI production code is modified.

Exact normalized comparison preserves 120 production items, 46 public paths,
all 22 original Rust tests/two fixtures and nine raw SQL/Lua blocks. Field
visibility is unchanged; 38 helpers gain family-local visibility. All 262 baseline
neighbors are unchanged, and the two UI inputs are separately hash-bound.

Paired stock 15/15 and refill 7/7, corrected Python contract 1/1, fresh all-targets,
scoped formatter, checker 19/19, ratchet, source/encoding proof and both Git checks
pass. All Cargo gates are terminal. Strict scans 1,597 files: 30 hard, 29 mandatory
and 40 soft; 59 remain above 700. Clearance advances to 86/145 (59.3%). The two
unchanged S06 formatter findings remain outside this scope.

Stock cooldown arithmetic/signal publication bounds and refill lease/delivery/
notification lifecycle remain separate reviews. S06, runtime-profile governance,
GWP-20260912-01 and final freeze/build transfer remain unchanged. This scope does
not perform a release or persistent deployment.

Next bounded regression: stock signal cooldown deadline arithmetic.
Immutable evidence: target/effective-line-evidence/20260912-credential-stock-refill-owners/scope.json.
Report: [stock/refill ownership](../../status/2026-09-12-credential-stock-refill-owners.md).
