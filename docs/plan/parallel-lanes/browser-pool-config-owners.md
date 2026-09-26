# Browser-pool executable and TLS ownership

Owner: resumed coordinator. State: accepted at 2026-09-14 00:28:52.651 UTC.
Entry 10,424 -> 10,333; executable owner 45, TLS owner 53, fixture 57, new
contracts 87 and package contract 157 effective lines. Paired Node suites pass
45/45 and package contracts 1/1. Source/syntax/checker/ratchet and both Git checks
pass. All gates are terminal; configuration fixture census is zero. The 696-input
union preserves 692 neighbors and web assets. Strict: 1,853 scanned, 16 hard,
24 mandatory, 40 soft; 40 above 700, clearance remains 105/145 (72.4%).

Evidence: target/effective-line-evidence/20260914-browser-pool-config-owners/scope.json.
SHA-256: d61f43870ef049079978bc396c7ff70af2a294dc998d548f272560ba7bb816c5.
[Checkpoint report](../../status/2026-09-14-browser-pool-config-owners.md).

Accepted preparation/design:
Prior scope: target/effective-line-evidence/20260914-browser-pool-input-owners/scope.json,
SHA-256 735aa41474724457612dfdfbb1d0bad4b2d20f8e43e5508153dc53e8d484ee30.

Move platform browser path lists, parseBoolean and resolveExecutablePath intact
to gemini-canvas-browser-pool-executable.mjs. Move getTlsAssetRoot,
normalizeTlsAssetSlug and loadOrCreateTlsCertificate intact to
gemini-canvas-browser-pool-tls.mjs with their fs/path/selfsigned imports. Entry
retains launch options, context ownership, server creation and shutdown.

The TLS module owns certificate file creation/reuse; it does not start a server.
Keep synchronous behavior, filenames, SANs, algorithm, lifetime, environment/cwd
resolution, error propagation and persistence ordering. Do not change TLS policy
or use this refactor to modify existing trust/permission semantics. Keep executable
override precedence and platform fallback order; do not add dependency injection.

Add private fixture exports and pre-extraction tests for booleans, real temporary
executable overrides, certificate generation/key matching/SANs, persisted reuse,
partial-pair replacement, relative TLS root and filesystem errors. Fixtures restore
environment values and remove only their canonically contained temporary roots.
Do not read or expose credentials or launch a browser/provider/live service.

Run paired browser-pool suites and nested package contracts, then syntax,
source/encoding, checker, ratchet, strict and both Git checks. Add both modules to
existing package byte/manifest/checksum coverage. No production release is made.
Root remains S18 debt; each extracted owner must be at most 500 effective lines.
S06 and final build coordination remain reserved under GWP-20260912-01.
