# Lane K: provider evidence runner test decomposition

Coordinator-only test scope, no production runner, Rust, Cargo or provider change.
The former825-effective-line suite now has these cohesive owners:

- Original entry159: offline secret-free evidence and explicit live-mode gate.
- Delegation377: tool failure classification, per-target success, environment-only
  credential forwarding and unchanged GET target delegation.
- Route proof284: version/request binding, untrusted shape/status rejection and
  mismatch classification.
- Shared process fixture24: runner path and invocation only, not a TestCase and
  containing no test methods. Each suite remains independently discoverable.

All9 test bodies plus the process invocation helper remain exactly unchanged:
AST source comparison original10/current10/unique10, zero differences, including
embedded PowerShell canary fixtures. No assertions, strings or scenarios were
removed. Files stay in tests/python, preserving the repository-relative root.

Evidence in target/effective-line-evidence/provider-runner-tests:
before.py is the actual pre-edit file; before.log9/9 (110.981s), after.log9/9
(111.594s); checker19/19, ratchet and scoped diff pass. All files UTF8 without BOM.
Focused discovery: python -m unittest discover -s tests/python -p
"test_gateway_provider_evidence_runner*.py". The fake canaries use temporary
scripts and synthetic credentials; SkipCargo remains present in all runner calls.
These are not real provider-live validation or a product release.

Strict1091 scanned:42 hard+75 mandatory=117 above700,38 soft; exit1 remains.
This clears one original debt file, now28/145 (19.3%) structurally cleared.
Production runner and process invocation behavior are untouched; the inherited
test helper still has no subprocess timeout, which is not resolved by this move.
No source/docs freeze acknowledgement for GWP05 has arrived, so this test-only
checkpoint does not initiate a shared build or mutate an immutable release.
