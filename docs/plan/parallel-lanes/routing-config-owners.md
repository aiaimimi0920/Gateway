# Routing configuration ownership

Owner: resumed parallel coordinator. State: accepted at 2026-09-13 19:13:39.649 UTC.

Second final/closing gates and immutable acceptance pass. Paired tests: 68/6/13;
all-targets, scoped formatter, checker 19/19, ratchet and source/encoding/diff
proof pass. Accepted union 623; 599 neighbors preserved. Strict: 1,807 scanned,
18 hard, 25 mandatory, 40 soft; 43 above 700, clearance 102/145 (70.3%).
Evidence: target/effective-line-evidence/20260914-routing-config-owners/scope.json.
SHA-256: 741762daf7875bf036d48aea40481a7ecfc0ac638bd21227959d8f4ab6453665.
See [acceptance report](../../status/2026-09-14-routing-config-owners.md).

Before snapshot pins build-UI acceptance and 600 inputs. Original source SHA-256:
4df50e24c76f70e2be24b1f68226f5c7e4141378d08a963f4cb4a17bbfe7a9f6.
Current projection: entry 166, all 24 source/test owners at most 323 effective lines.
Exact proof preserves 75 production items, 68 tests/three fixtures, 50 raw YAML
literals and original public/crate paths. Independent review found no concrete
extraction regression. Baseline config/admission/hot-reload contracts pass
68/6/13. The exploratory token_refresh:: filter selects zero tests and is not
reported as coverage; its receipt remains recorded and it will not be repeated.

First final tests and all-targets passed, but acceptance rejected new unused
reexports for normalize_model_name and safe_refresh_deadline. Both unchanged
short functions now stay at the original parent; only the parent and their two
former owners changed. Candidate/projection two and separate attempt2 receipts
preserve the first attempt. Acceptance uses the second final/closing gate set.

Exclusive source scope: src/routing/config.rs and new src/routing/config/ owners
and nested tests. The plan assigns this boundary to S16 at
2026-09-03-gateway-effective-line-refactor.md:327. It does not transfer S06.

The coordinator read all 4,654 physical lines, including 68 tests and three
fixtures. The current authoritative inventory is 3,984 effective lines. Preserve
all original public/crate root paths, private state fields, refresh generation
and retirement, ArcSwap publication, alias precedence, constrained round-robin,
account inventory caching, probe selection and raw YAML test bytes.

Keep shared private state at the entry. Extract schema, substitution, provider
payload/compilation, document fingerprints, alias/candidate resolution, account
inventory/constraints, immutable snapshot queries/publication, model listing,
refresh lifecycle and store operations into native private modules. Every result
must be at most 500 effective lines. Tests split by behavior while retaining
all fixture/test bodies and recording nested test-path changes.

Before edits, pin build-UI acceptance and the union of neighboring inputs, then
run paired route-config unit, token-refresh, route-credential admission and console
snapshot contracts as applicable. Exact gates and counts will be recorded from
the current harness. Serialize Cargo/formatter operations with process guards;
do not mutate Rust while a compiler runs. Keep GATEWAY_PREBUILT_WEB_UI=1 local
to Cargo commands and preserve published assets. Final all-targets and scoped
closing gates remain required. No release, runtime profile, policy/baseline,
exception, dependency or S06-owned source change is authorized by this lane.
