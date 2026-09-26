# Routing candidate endpoint policy ownership

Date: 2026-09-21. State: structural_green.

Scope: `src/routing/candidate.rs` and its new private `endpoint_policy` child.
The current parent is 541 effective lines. Extract the complete payload policy
impl and its two private helpers. Keep payload/candidate types, serde contracts,
public canonicalization functions and all original tests at their existing
paths. Move no provider runtime or reserved S06 file.

Preserve endpoint overrides before producer-video behavior before explicit
account/default execution mode; preserve OpenAI path bridging, search support,
canonical adapter aliases and endpoint key strings exactly. The child only
depends on parent types/constants and canonical EndpointKind; no I/O or state
ownership moves. Both resulting files must be <=500 effective lines.

Evidence: `target/effective-line-evidence/20260921-routing-candidate-policy/`.
Run paired candidate tests, all-target compile, exact formatted projection,
checker tests/ratchet/strict, scoped/global format, encoding and independent
Gateway/Neuro Git checks. Record inherited failures separately. The full plan
and release goal remain active beyond this structural checkpoint.

Result: 429/119 effective lines, paired 15/15 tests and all-targets pass.
See `docs/status/2026-09-21-routing-credential-test-owners.md` for full evidence.
