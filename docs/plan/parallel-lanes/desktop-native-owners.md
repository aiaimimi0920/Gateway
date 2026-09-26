# Desktop native process and profile ownership

Owner: parallel coordinator. State: structural_green. Started: 2026-09-12 UTC.
Accepted: 2026-09-12 22:22:36 UTC.

Scope: apps/desktop/src-tauri/src/process.rs, profile.rs and their private
extracted owners. The entries measure 866 and 741 effective lines. Keep all eight
Tauri command functions in their original parents and preserve lib.rs registration.
Separate child environment, startup probes, shutdown, profile paths, validation,
dependency checks, preflight assembly, storage and original tests. Every resulting
owner must remain at most 500 effective lines.

Preserve complete production bodies, public and crate-local entry paths, Tauri
attributes, serde contracts/defaults, environment clearing, management-token
headers, preflight/launch/drain ordering, process handles and last-owner Drop
cleanup. Move original tests without changing assertions or module paths. Python
source contracts may read the extracted production owners; their existing
assertions must remain intact.

Require paired native library and three desktop Python contract suites, fresh
native and root all-target checks, scoped formatting, checker 19/19, ratchet/strict
inventories, exact source/test/neighbor proof, UTF-8 without BOM and both Git
checks. Record baseline native formatter debt separately. Serialize every Cargo
operation; do not edit Rust while any gate is running.

This structural lane does not change blocking startup/dependency I/O, shutdown
deadlines, path policy, profile persistence semantics or test-fixture lifecycle.
Newly observed failures require a separate evidence-led decision before changes.

S06 retains its implementation/cursor. GWP-20260912-01, runtime-profile governance
and final freeze/build transfer remain pending. No dependency, checker-policy,
baseline, exception, release or persistent runtime changes belong to this lane.

Evidence: target/effective-line-evidence/20260912-desktop-native-owners/.

Native baseline passes 14/14, including process-tree termination, last-owner
authorized drain and log redaction. The initial Python baseline passes 25/26;
one UI contract still reads the old BrowserConsoleApp owner for extracted account
filtering and pool actions. Its reader now includes the existing production
owners. Native source readers also cover the new child modules, excluding their
test files. Reader-only proof preserves all 239 assertions and 26 test bodies
apart from those source-loading statements; the adapted baseline passes 26/26.
The original failed receipt is retained. Native cargo fmt baseline returned zero;
the runner's predeclared nonzero expectation was incorrect and is not a product
failure. Final explicit scoped rustfmt passes.

The process/profile parents are now 204/186 effective lines; all twelve native
files are at most 324. Exact comparison preserves 67 production definitions,
25 public and three crate-local entry paths, all eight parent-owned Tauri
commands, ten moved tests and two fixtures. Twelve helpers and one constant gain
only family-local visibility; 313 neighboring inputs remain unchanged.

Paired native 14/14 and adapted Python 26/26, fresh native/root all-target checks,
scoped/native formatting, checker 19/19, ratchet, exact source/encoding proof and
both Git checks pass. All gates are terminal; no named fixture processes remain.
Strict scans 1,620 files: 29 hard, 27 mandatory and 40 soft; 56 remain above 700.
Clearance is 89/145 (61.4%). Global formatting retains only the two S06 findings.
See the [acceptance report](../../status/2026-09-12-desktop-native-owners.md).
Automation script I/O lifetime is the next focused review.
