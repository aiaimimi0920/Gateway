# Lane P: provider line evidence runner decomposition

Coordinator owns the PowerShell runner, tools/line-evidence private functions,
and the directly affected package exclusion contract. Rust and Cargo remain
with S06; all evidence-runner tests use fixture canaries or explicit SkipCargo.

The entry moves from 1003 to 448 effective lines. Extracted owners are process
and path operations 131, manifest selection 109, safe evidence projection 258,
and offline verification 83. All five owners are below 500, UTF8 without BOM.
The 25 original helper function bodies and the offline loop match the saved
baseline exactly. The offline function receives its eight phase inputs explicitly;
the caller captures its records with @(...). Private helpers remain dot-sourced
and intentionally retain the runner's established invocation-scope dependencies.
Live orchestration, route-proof binding, classification and final output remain
in the entry. The repository-relative line verifier path is unchanged.

The release packager excludes the private line-evidence directory alongside the
already excluded development runner. A real staging contract places a private
helper in its source fixture and proves the whole directory absent from output.
The redundant explicit empty root-exclusion argument on recursive copies was
removed: the function parameter already defaults to an empty array per call.
The packager remains at its inherited 701 effective lines without net growth;
its unrelated structural debt is not claimed as cleared by this lane.

Verification evidence: target/effective-line-evidence/line-evidence-runner.
Before and after provider-evidence suites pass 9/9 (142.622s and 162.651s).
All six touched PowerShell source files parse. The extraction verifier confirms
25 exact helper bodies and the unchanged offline loop. Package layout passes
1/1 after the final packaging change; checker 19/19, ratchet and diff pass.
Independent read-only review found no scope, array or live-flow regression.
The full Node 22.22.2 suite passes 409 with one POSIX-only skip, including the
latest Producer tool-summary control-field regression. Full offline Python passes
283 tests in 665.214s with four explicit skips and zero failures. This fresh gate
includes lanes O/P, packaged inventory imports and the latest package exclusions.

Current ratchet snapshot: 1152 files, 40 hard, 72 mandatory and 38 soft files.
The inherited strict gate remains nonzero. Structural clearance is 33/145,
with 112 files above 700. No baseline or exception registry was changed.

Resource and security review preserves environment-only canary credential
forwarding, temporary target-file cleanup, secret-free evidence checks and
request-bound route proofs. Existing unbounded subprocess capture, JSON input,
dynamic helper scope, non-atomic evidence writes and cleanup-error suppression
remain hardening work. This extraction does not claim those behaviors safe.
No provider calls, Cargo, production runtime or immutable release was changed.
GWP-20260908-06 still requires an explicit source/docs freeze and build transfer.
