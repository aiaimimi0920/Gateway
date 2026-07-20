# TypeScript To Rust Mapping Archive

## Status

The TypeScript gateway migration is complete.

- Rust `gateway/` is the only active gateway runtime.
- The old TypeScript gateway has been removed from the repository.
- This file remains only as a concise archive of concept-level migration results.

## High-Level Mapping

| Legacy responsibility | Rust owner |
| --- | --- |
| Public `baseURL + api_key` ingress | `gateway/src/http/router.rs` and pipeline stages |
| Project / special API key auth | `gateway/src/auth/project_api_key.rs` |
| User credential auth | `gateway/src/auth/user_credential.rs` |
| Provider credential CRUD | `gateway/src/http/routes/internal_provider_credentials.rs` |
| Provider account CRUD | `gateway/src/http/routes/internal_provider_accounts.rs` |
| Route policy / model alias management | `gateway/src/http/routes/internal_routing.rs` |
| Request audit / traces / artifacts | `gateway/src/db/request_audits.rs` and `gateway/src/http/routes/internal_requests.rs` |
| Response cache / quota / finalize settlement | `gateway/src/pipeline/stage_filter.rs`, `gateway/src/pipeline/stage_finalize.rs`, `gateway/src/redis/usage_tracking.rs` |
| Browser executor service and runtime plane | `gateway/src/http/routes/internal_browser_executor.rs` and `gateway/src/browser_executor_runtime.rs` |
| Keepalive / session ensure | `gateway/src/keepalive.rs` and `/v1/credentials/ensure` |
| Operator analysis / anomaly / remediation | `gateway/src/db/anomaly_incidents.rs`, `gateway/src/db/remediation.rs`, `gateway/src/db/analysis_exports.rs` |

## Current Rule

- Do not recreate a second gateway runtime in TypeScript.
- Do not restore the deleted legacy gateway as a helper, compat profile, or preview dependency.
- Any remaining website or operator capability must integrate with Rust `gateway` APIs, not a revived relay layer.
