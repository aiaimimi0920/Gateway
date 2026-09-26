# Provider quota ownership

Owner: parallel coordinator. State: structural verification complete; hardening follows.
Started: 2026-09-12 UTC.

The coordinator owns src/provider_quota.rs and its new private child modules.
The source was read completely before projection. The entry originally measured
1,390 effective lines and combined public views, cache storage, refresh scheduling,
locking, provider HTTP probes, payload lookup, aggregation and seven unit tests.

Retain all public and crate-local entry paths, wire records and field visibility.
Separate cache, refresh orchestration, refresh lock, refresh timing, aggregation,
Codex, Accio response, Accio HTTP, generic balance and shared HTTP/payload leaves.
Move the original test module without changing its seven tests or two fixtures.
Every projected owner must be at most 500 effective lines.

Preserve request URLs/headers, Accio endpoint fallback and primary-error precedence,
quota classification, reset/refresh timing, cache scope/TTL, lock behavior and
aggregation ordering in this structural phase. Capture current source and callers,
run the same tests before/after, compare every production item and public path,
then run fresh all-targets, formatter, checker 19/19, ratchet, strict inventory,
UTF-8/no-BOM/source proof and both Git checks.

The existing GET/compare/DEL refresh lock, request diagnostics containing provider
URLs, unbounded response reads and permissive numeric conversion are recorded
for separate evidence-led hardening after structural acceptance. This phase does
not change their behavior. S06 ownership, GWP-20260912-01 and final freeze/build
coordination remain unchanged. No release or persistent deployment is authorized
by this structural checkpoint.

Evidence: target/effective-line-evidence/20260912-provider-quota-owners/.

Accepted 2026-09-12 17:28:52 UTC. Entry: 1,390 -> 104 effective lines; all thirteen
owners are at most 223. Exact normalized comparison preserves 39 functions, seven
structs, one constant, eleven public paths, the crate-local cached-refresh path,
seven original tests/two fixtures and 240 neighboring inputs. Eighteen helpers and
the refresh-lock type gain only family-local visibility; fields are unchanged.

Paired quota tests 7/7, fresh all-targets, scoped formatter, checker 19/19, ratchet,
source/encoding proof and both Git checks pass. All Cargo gates are terminal.
Strict: 1,577 scanned, 31 hard, 30 mandatory, 40 soft; 61 above 700. Clearance is
84/145 (57.9%). The two unchanged S06 formatter findings and freeze/build transfer
remain open. URL-bearing transport diagnostics are the next fixed regression.
[Acceptance report](../../status/2026-09-12-provider-quota-owners.md).
