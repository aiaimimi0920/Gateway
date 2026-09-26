# Console telemetry ownership

Date: 2026-09-21. State: structural_green.

Scope: telemetry.ts (578 effective), new telemetryRollups.ts and
telemetryPresentation.ts, their three direct consumers, and focused telemetry
contracts. Keep snapshot/input/output types and normalization in telemetry.ts;
move aggregation and display policies into cohesive owners. The shared pure
laterTimestamp function is exported from the snapshot owner for rollups. Keep
dependency direction rollups -> snapshot; no cycle or compatibility barrel.

Add five behavioral contracts before moving source: account deduplication,
absent-vs-observed metrics, model concurrency attribution, poll presence and
display semantics. Run them against baseline and split implementation. Preserve
every production function body, including null handling and bounded quota
traversal. Rewire only existing import declarations in consumers.

Evidence: `target/effective-line-evidence/20260921-telemetry-owners/`.
Run paired focused tests, desktop typecheck, exact source projections, checker
tests/ratchet/strict, UTF-8/no-BOM and separate Git checks. All modified owners
must stay <=500 effective lines. Full-goal/release acceptance remains open.

Result: production owners 362/145/73; five paired contracts, desktop typecheck,
web build, exact projection and independent review pass. See
`docs/status/2026-09-21-session-telemetry-owners.md`.
