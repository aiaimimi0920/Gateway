# Gateway Productization Risk Assessment

## P0 Risks

| Risk | Evidence | Impact | Verification |
| --- | --- | --- | --- |
| Package contents do not match the Gateway packaging contract | `INTEGRATION_CONTRACT.md` requires routes/manifests/scripts, while the current root Gateway release spec has no support files | A desktop package can open but cannot run the configured provider runtime on another machine | Package manifest and extracted-archive contract test |
| Desktop defaults depend on compile-time paths | `apps/desktop/src-tauri/src/process.rs` uses `env!("CARGO_MANIFEST_DIR")` fallback | Release copied to another machine may launch the wrong/nonexistent working directory | Stage package in a path outside the checkout and assert resolved paths |
| Desktop drain does not currently pass management credentials | `try_graceful_shutdown` posts drain without the configured token | Stop falls back to forced process termination even when Gateway is healthy | Test authorized drain and observe graceful child exit |
| Runtime dependency state is implicit in onboarding | Profile templates default to local Redis but do not perform a dependency preflight before start | Users receive a generic startup failure instead of an actionable setup result | Profile validation and isolated Redis/no-Redis tests |

## P1 Risks

| Risk | Evidence | Impact | Verification |
| --- | --- | --- | --- |
| Provider completion is represented in several independent sources | Manifests, Cargo features, routes, docs, operator catalog, and live artifacts evolve separately | A provider can appear implemented while a capability or operator path is absent | Generated inventory consistency test |
| Live evidence is not part of the default gate | Release candidate intentionally skips real upstream calls | CI can prove compilation but not current account, quota, challenge, or regional behavior | Explicit canary records and classified results |
| Browser-backed execution has multiple fallback modes | Remote executor, local fallback, session material, and policy flags interact | A line may silently use a different execution mode than its declared contract | Mode-specific provider canary and runtime diagnostics |
| Runtime and provider code are concentrated in very large files | `upstream/client.rs`, Gemini Canvas, DB remediation, and desktop state are large | Changes are difficult to review and regressions are hard to isolate | Focused module tests before extraction |

## P2 Risks

| Risk | Evidence | Impact | Verification |
| --- | --- | --- | --- |
| Enterprise observability is fragmented | Existing metrics, audit, and logs are present but not one operator contract | Incident diagnosis requires source-level knowledge | Trace/metric/audit correlation smoke |
| Dependency failure semantics vary by surface | Redis, PostgreSQL, browser executor, and upstream errors have separate paths | Retry, drain, quota settlement, and readiness can disagree | Fault-injection matrix |
| Cross-subproject release state can mark Gateway dirty | The monorepo contains concurrent changes outside Gateway | A valid Gateway package may be rejected by a global clean-worktree gate | Gateway-scoped release cleanliness test |

## Invariants

- Do not remove or rewrite development-time sensitive values.
- Do not introduce a provider-specific bypass around the canonical pipeline.
- Do not make real live calls part of ordinary CI.
- Do not let the desktop shell become a second provider runtime.
- Do not claim a phase complete without package/runtime/test evidence.
