# HTTP route registration ownership

Owner: parallel coordinator. State: `structurally_verified`. Date: 2026-09-11.

Scope is `src/http/router.rs` (1,011 effective lines), its private `router/`
directory and `tests/router_layer_contract.rs`. No handler, middleware, database,
S06 implementation, original plan cursor or release ownership is transferred.
The preceding accepted checkpoint has 86 files above 700 effective lines.

The entry retains UI registration, ordered domain mounting, observability routes,
global body limits, request lifecycle/logging, CORS and final state attachment.
Private registration owners follow inference, management access, configuration,
request audit/archive reads, credential lifecycle and analysis reports. They
accept and return the existing router; all route paths, methods, handlers,
per-family limits and compatibility aliases must remain unchanged and in order.

Before production extraction, record the original source and run the same smoke,
ingress, management admission, readiness and router-layer contracts that will run
afterward. New contracts check both sides of 33 endpoint body-limit boundaries,
the global limit on six route groups, and CORS/drain/body-limit ordering.

All new owners must remain below 500 effective lines. Use a complete ordered
registration/source comparison, serialized Cargo gates, all-targets compilation,
scoped/global formatter checks, checker tests, ratchet, strict inventory and
independent Git/encoding checks. Evidence is under
`target/effective-line-evidence/20260911-http-router/`. Integrated release still
requires the separate GWP-20260908-06 source/docs freeze and build-transfer receipt.

The entry is now 40 effective lines; all eight source/test files are at most 235.
The complete source proof preserves 233 route registrations, 31 locally limited
routers, their ordered merge/registration chain, and outer layer/state wiring.
All 44 tests across the seven selected targets pass before and after extraction.
All-targets compilation, scoped formatter, checker 19/19, ratchet, encoding and
independent Git checks pass. Strict now reports 85 files above 700. Global
formatting still reports only the two reserved S06 runtime-mirror files.

Acceptance: [HTTP router checkpoint](../../status/2026-09-11-http-router.md).
