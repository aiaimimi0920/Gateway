# Desktop native process and profile ownership

Accepted: 2026-09-12 22:22:36 UTC. Gateway-only structural checkpoint.

The process entry decreases from 866 to 204 effective lines. Its environment,
startup, shutdown and original-test owners measure 85, 137, 141 and 324. The
profile entry decreases from 741 to 186; its path, validation, preflight,
dependency, storage and original-test owners measure 114, 94, 166, 102, 29 and
80. All twelve native files remain below 500 effective lines.

Exact formatted-item comparison preserves 67 production definitions, 25 public
entry paths, three crate-local paths, all eight Tauri command bodies/attributes,
serde fields/defaults and the original ten moved tests plus two fixtures. Twelve
private helpers and one constant gain only family-local visibility. Tauri command
functions stay in their parents; lib.rs, state.rs and all neighboring runtime
callers remain byte-identical. Child environment clearing, authorized drain,
launch/cleanup ordering, process handles and last-owner Drop behavior are retained.

Native library tests pass 14/14 before and after extraction, including Windows
process-tree termination, authorized last-owner drain, path resolution and log
redaction. The first Python run passes 25/26: an older UI contract reads only
BrowserConsoleApp.tsx for account filtering and pool actions that were already
extracted. Its source reader now includes the actual production owners. The
native readers similarly include child production modules and exclude tests.rs.
All 239 assertions remain unchanged; the adapted pre-extraction baseline and
post-extraction suites both pass 26/26. The original failure is retained in evidence.

Fresh serialized native and root cargo check --offline --locked --all-targets
pass. Scoped rustfmt, native cargo fmt, checker 19/19, adoption ratchet, exact
source/UTF-8/no-BOM proof and both repositories' git diff --check pass. The native
formatter baseline also returned zero; the runner had incorrectly expected a
nonzero observation, and that original receipt is preserved. Root compilation
retains its existing three upstream warnings. Global formatting still reports
only the two unchanged S06 runtime-mirror files. The final process observation
finds no named native fixture processes or Cargo/compiler/formatter processes.

The proof preserves 313 neighboring inputs, including the six frontend owners
read by the corrected UI contract. Immutable evidence is
target/effective-line-evidence/20260912-desktop-native-owners/scope.json, SHA-256
3e21ffab7022c863296ff3f60168e8b67089b5d0237123844645ea3db838ee29.
It binds the original and adapted snapshots, unchanged native/Python test cases,
compiler/checker/cleanup logs, inventories and independent Git observations.
Strict scans 1,620 files: 29 hard, 27 mandatory and 40 soft; 56 remain above 700.
Clearance is 89/145 (61.4%).

At capture, Gateway has 1,793 dirty entries: 175 modified, one unstaged deletion,
two staged deletions and 1,615 untracked. Neuro has 192: ten modified and 182
untracked. Both HEADs and inherited deletions remain unchanged. Historical
accepted scopes have not been regenerated.

Automation script I/O lifetime is the next focused review. Native blocking DNS,
startup/shutdown deadlines, profile storage races and cross-platform process-tree
semantics remain separate hardening work. S06 ownership, GWP-20260912-01,
runtime-profile governance and final freeze/build transfer remain pending. No
immutable release, packaged runtime/UI acceptance or persistent deployment occurred.
