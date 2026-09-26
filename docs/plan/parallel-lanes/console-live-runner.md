# Lane Q: console live E2E runner decomposition

Coordinator owns tools/run-gateway-console-live-e2e.ps1 and the new private
tools/console-live-e2e function files. S06 retains Rust/Cargo; this extraction
does not run the complete console E2E orchestrator or change UI production code.

The original runner is 965 effective lines. Its entry now has 373 lines,
retaining setup, canonical route seed constants, build/browser invocation,
environment snapshots, live orchestration and finally cleanup. Four owners
separate runtime/process/HTTP operations (171), disposable Redis and RESP (196),
revision archive seeding (67), and the deterministic Node upstream fixture (162).
The existing inline fixture source is moved unchanged, not newly encoded to
evade the line checker. All five owners are below 500 and UTF8 without BOM.

All 22 original function extents match exactly after newline normalization,
and the complete orchestration suffix is unchanged. Every source AST parses.
The entry initializes RepoRoot and RedisImage before sourcing private functions;
runtime precedes Redis, revision seeding and upstream to satisfy dependencies.
These remain private invocation-scoped functions, not standalone APIs.

Evidence: target/effective-line-evidence/console-live-runner. Original and
post-extraction static console contracts pass 3/3. The nested package fixture
copies the actual runner and four helpers, checking every byte, support record
and checksum. On Windows it loads the packaged helpers from outside the package
and checks a RESP line reader, valid/invalid revision archive publication, and a
real loopback Node upstream's health/completion/request log. The owned Node
process is stopped, waited and disposed. Package test passes 1/1; a fresh process
census finds zero matching console-fixture Node processes. No Docker, Cargo,
real provider, production Gateway or Playwright browser was started by this test.

Checker 19/19, ratchet and diff pass. Independent review found no extraction
regression. Snapshot: 1158 files, 40 hard, 71 mandatory and 38 soft. Structural
clearance is 34/145 (23.4%), with 111 files still above 700; inherited strict
failure remains. No baseline, exception registry or packager implementation
change was needed: recursive tools packaging already includes the new owners.

Validation does not cover the full TCP RESP writer/parser, disposable Docker
lifecycle, Gateway startup/restart, browser mutations or implicit defaults in
Invoke-LoggedCommand. Those original paths are exact moves, not newly certified
runtime successes. Unbounded subprocess capture, RESP allocation, fixture body/
log growth, raw fixture authorization logging, startup port races and cleanup
error handling remain hardening work. Test credentials are synthetic and the
fixture server binds loopback. The full console/UI scenario still needs its
proper shared Cargo/runtime window. Actual immutable release inclusion is pending
GWP-20260908-06, which has no explicit transfer receipt at this checkpoint.
