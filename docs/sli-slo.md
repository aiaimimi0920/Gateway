# Gateway SLI And SLO Contract

This document defines measurable service indicators for the Rust Gateway. The
source of truth is the current Gateway-owned runtime surfaces, not a dashboard:

- `GET /healthz` for process liveness.
- `GET /readyz` for traffic admission readiness.
- `GET /metrics` for Prometheus counters and gauges.
- `GET /v1/internal/gateway/operations/summary` for an authenticated,
  secret-free point-in-time runtime and dependency summary.

Use `tools/get-gateway-operator-summary.ps1` to collect the management summary.
The command accepts `x-management-token` by default or Bearer authentication
with `-AuthenticationMode Bearer`; neither output mode prints the token.

## Measurement Rules

- The availability window is 30 rolling days unless a row states otherwise.
- Planned drains count as unavailable at `/readyz` and are excluded from the
  readiness SLO only when the drain has an approved change record. They remain
  visible through `gateway_request_drain_rejections_total` and operator summary.
- `gateway_request_errors_total` currently counts every HTTP status at or above
  400. Its ratio is therefore a request-success indicator, not a pure server
  availability indicator; client-invalid traffic can reduce it.
- The current duration series exposes count and sum, not histogram buckets.
  Only a mean-duration objective is executable today. A percentile SLO would be
  fictitious until histogram buckets exist.
- Provider ratios must retain the bounded `provider` and `model` labels. The
  `overflow` series is a cardinality guard and must be investigated separately.

## Service Objectives

| SLI | Executable measurement | Objective | Short-window guardrail |
| --- | --- | --- | --- |
| Process liveness | `avg_over_time(probe_success{job="gateway-liveness"}[30d])` | At least 99.95% over 30 days | `GatewayLivenessFailed`: non-200 for 2 minutes |
| Traffic readiness | `avg_over_time(probe_success{job="gateway-readiness"}[30d])` after removing approved maintenance windows from the SLO report | At least 99.9% over 30 days, excluding approved drains | `GatewayReadinessFailed`: non-200 for 2 minutes |
| Request success | `1 - increase(gateway_request_errors_total[30d]) / clamp_min(increase(gateway_requests_total[30d]), 1)` | At least 99% over 30 days, interpreted with the client-error limitation above | Five-minute error-ratio rules in `operations-alerts.yaml` |
| Mean request duration | `increase(gateway_request_duration_ms_sum[30d]) / clamp_min(increase(gateway_request_duration_ms_count[30d]), 1)` | Thirty-day mean at or below 30,000 ms | Five-minute mean above 30,000 ms for 10 minutes |
| Provider/model success | `1 - sum by (provider, model) (increase(gateway_provider_errors_total{failure_class=""}[30d])) / clamp_min(sum by (provider, model) (increase(gateway_provider_requests_total[30d])), 1)` | At least 95% per enabled provider/model over 30 days | Ten-minute provider/model error ratio below 80% |
| Route availability | `gateway_routes_configured == 1` and `gateway_providers_total >= 1` | 100% while accepting public model traffic | Routes missing for 2 minutes; empty provider inventory for 5 minutes |

The 30-day expressions above evaluate the product objectives. They are distinct
from the 5-minute and 10-minute incident guardrails in `operations-alerts.yaml`.
That file is a native Prometheus rule file and must pass `promtool check rules`
before deployment. Run `tools/validate-gateway-alert-rules.ps1`; use
`-RequirePromtool` in release automation so a missing validator fails closed.

The provider error numerator explicitly selects the aggregate series with
`failure_class=""`. Prometheus treats that matcher as also matching a missing
label. This avoids summing the aggregate series together with its per-class
breakdown and double-counting the same failures.

## Operator Summary Contract

The management summary returns schema version 1 and includes:

- package name/version, target OS/architecture, runtime role, process ID, and
  listening port;
- serving/draining state, drain metadata, shutdown request state, and active
  request count;
- Redis, optional PostgreSQL, and object-storage readiness without URLs,
  connection strings, access keys, tokens, cookies, or session material;
- route configuration state, provider count, published route/model count, and
  credential memory-cache entry count. `routeCount` is the number of model IDs
  currently published by `RouteConfigStore::list_models()`, not the number of
  internal policy rows;
- the existing process request-metric snapshot and PostgreSQL-backed provider
  readiness counts when PostgreSQL is configured and reachable.

The summary is diagnostic evidence, not a historical SLO store. Prometheus and
the external HTTP probe retain the time series used for objective evaluation.
Its `readiness.ok` mirrors the existing internal enterprise contract, including
object storage, API-key secret, public base URL, and draining gates. `/readyz`
remains the narrower traffic-admission probe for Redis, configured PostgreSQL,
and draining state.

## Dependency Failure Matrix

| Condition | Observable evidence | Readiness and request behavior | Operator action |
| --- | --- | --- | --- |
| Redis unavailable | `/readyz` is 503; operator summary has `readiness.dependencies.redis.ready=false` | Redis is required and blocks traffic readiness. Redis-backed request paths may fail until connectivity recovers. | Stop rollout activity, verify only the explicitly configured Redis target, retain request IDs/logs, and confirm `/readyz` recovery before restoring traffic. |
| PostgreSQL not configured | Operator summary has `postgresql.configured=false`, `required=false`, and `ready=true` | This is the supported DB-less runtime mode and does not block `/readyz` or internal readiness. PostgreSQL-owned management/provider statistics remain unavailable or zero. | No incident action. Confirm the deployment intentionally uses DB-less mode. |
| PostgreSQL configured but unavailable | `/readyz` is 503; summary has `postgresql.configured=true` and `ready=false` | A configured database is treated as an active dependency and blocks readiness until its probe recovers. | Stop writes and rollout activity, preserve database logs, validate failover/recovery separately, then confirm both readiness endpoints. |
| Browser executor unavailable | `GET /v1/internal/gateway/browser-executor/health` or `GET /v1/internal/browser-executor/health` reports unhealthy/degraded state; browser-backed requests fail | It does not globally block `/readyz`; impact is limited to implementation lines that require browser execution unless that line records an explicit fallback. | Isolate affected browser capacity, inspect leases/session evidence, and validate a replacement executor before re-enabling the affected line. |
| Upstream timeout | Request audit/error kind records timeout or HTTP 504; provider metrics increment `gateway_provider_errors_total` under the existing `provider_transient` classification | The canonical pipeline applies its existing retry/fallback policy; the summary does not claim a separate timeout counter. | Correlate request ID, provider/model labels, latency, and audit details. Reduce or isolate only the affected provider/model when evidence is scoped. |
| Retry exhaustion | The final request audit/error code records the concrete provider-specific exhausted condition; provider error/request ratio increases | No generic `retry_exhausted` Prometheus label exists. The final error remains visible through request audit and the current provider failure taxonomy. | Preserve the last error and attempt evidence, stop repeated retries, then repair or remove the failing route/credential before replay. |

## Executable Checks

```powershell
Invoke-RestMethod -Method Get -Uri "$baseUrl/healthz"
Invoke-RestMethod -Method Get -Uri "$baseUrl/readyz"
Invoke-WebRequest -UseBasicParsing -Uri "$baseUrl/metrics"
.\tools\get-gateway-operator-summary.ps1 `
    -BaseUrl $baseUrl `
    -ManagementToken $env:GATEWAY_MANAGEMENT_TOKEN
```

The focused contracts are `tests/operator_summary_contract.rs`,
`tests/observability_contract.rs`, and
`tests/python/test_gateway_operator_summary_contract.py`. The packaged runtime
smoke also exercises `/healthz`, `/readyz`, and `/metrics`. These checks validate
the current surfaces; they do not substitute for a 30-day SLO time series.
