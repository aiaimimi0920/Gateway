# Prompt Cache Monitoring and Billing Design

## Implementation Status

Current Rust gateway status as of 2026-06-09:

- Anthropic runtime parsing now captures `cache_creation_input_tokens` and `cache_read_input_tokens` in the Rust token-usage structures
- Anthropic responses rebuilt by the Rust protocol adapter now preserve those cache usage fields when they are present
- `gateway_request_audits` now has dedicated cache-token columns through the shared migration/schema path
- Rust finalize now writes cache metrics into request audits and usage reports
- Rust `/metrics` now exports baseline prompt-cache aggregate counters sourced from `gateway_request_audits`
- Rust internal analysis now exposes prompt-cache summary and trend reads, including estimated cost savings based on cached read tokens
- Rust request audits now also persist `client_has_cache_control` and `auto_cache_applied`, so prompt-cache monitoring can distinguish client-managed caching from Rust auto-cache behavior
- Rust `/metrics` and prompt-cache summary/trend reads now expose adoption counters for `client marked` vs `auto applied` request paths
- Web operator console now exposes `/ops/gateway/prompt-cache`, and the current `account-api -> ai-gateway-domain` transitional layer now mirrors the Rust prompt-cache summary/trend contract so operators get a first-class adoption/savings dashboard instead of raw audit inspection
- User-facing AI benefit `service_proxy` flows now also expose rolling prompt-cache summary + trend reads through `/v1/me/benefits/services/:serviceId/prompt-cache-summary` and `/v1/me/benefits/services/:serviceId/prompt-cache-trend-report`, and the benefit-center overlay surfaces the same personal hit-rate / savings summary plus recent bucketed trend without operator privileges
- The remaining work in this document is now focused on higher-level reporting:
  - wider and longer-horizon dashboard/reporting surfaces on top of the new Rust read models
  - richer Prometheus/Grafana breakdowns beyond the new aggregate counters
  - final billing/quote integration decisions for cached tokens

## Current Code Anchors

- `Gateway/src/protocol/anthropic.rs`: captures Anthropic cache-token usage and request-side prompt-cache telemetry.
- `Gateway/src/redis/usage_tracking.rs`: parses Anthropic and OpenAI-style cached-token usage fields for usage reports.
- `Gateway/src/pipeline/stage_finalize.rs`: persists cache-token usage plus `client_has_cache_control` and `auto_cache_applied` into request audit and usage-report paths.
- `Gateway/src/db/request_audits.rs`: stores cache-token fields, exposes prompt-cache summary/trend reads, and estimates cost savings from cached read tokens.
- `Gateway/src/http/routes/metrics.rs`: exports aggregate counters including `gateway_prompt_cache_hit_requests_total` and `gateway_prompt_cache_auto_applied_requests_total`.
- `Gateway/src/http/routes/internal_requests.rs`: exposes internal analysis prompt-cache summary and trend report endpoints.
- `Gateway/src/http/routes/internal_gateway.rs`: exposes project-scoped prompt-cache summary and trend report endpoints for benefit-service flows.

## Purpose

This document defines how the Rust gateway should monitor, track, and bill for Anthropic's Prompt Cache feature.

## Executive Summary

**Problem**: Anthropic's Prompt Cache allows caching repeated prompt prefixes to reduce costs by 90%. Before the current Rust implementation, the gateway forwarded `cache_control` but did not expose enough structured cache usage data for operators or users.

**Current solution**: The Rust gateway now parses cache-token usage fields, stores them in request audits and usage reports, exports baseline aggregate metrics, and exposes summary/trend read models for both operator analysis and user-facing benefit flows.

**Impact**:
- **Billing visibility**: Cache-token fields are captured and available for final billing/quote decisions.
- **User visibility**: Operator and benefit-center surfaces can show hit rate, saved tokens, and estimated savings.
- **Operational insights**: Rust read models distinguish client-managed prompt caching from Rust auto-cache behavior.

---

## Background: How Prompt Cache Works

### Client-Side Configuration

Clients mark cache points using `cache_control` parameter:

```json
{
  "model": "claude-3-5-sonnet-20241022",
  "messages": [{
    "role": "user",
    "content": [{
      "type": "text",
      "text": "Long document...",
      "cache_control": {"type": "ephemeral"}  // ← Cache this
    }, {
      "type": "text",
      "text": "Summarize it"  // ← Don't cache
    }]
  }]
}
```

### Anthropic's Response

Anthropic returns detailed token usage in the `usage` field:

```json
{
  "id": "msg_123",
  "content": [...],
  "usage": {
    "input_tokens": 100,                    // New input tokens
    "cache_creation_input_tokens": 15000,   // Tokens cached (first request)
    "cache_read_input_tokens": 0,           // Tokens read from cache (0 = no hit)
    "output_tokens": 200
  }
}
```

**Second request (within 5 minutes)**:
```json
{
  "usage": {
    "input_tokens": 100,
    "cache_creation_input_tokens": 0,
    "cache_read_input_tokens": 15000,  // ← Cache hit! 15000 tokens
    "output_tokens": 200
  }
}
```

### Cost Calculation

```
First request (no cache):
- Input: 15100 tokens × $15/M = $0.2265
- Output: 200 tokens × $75/M = $0.015
- Total: $0.2415

Second request (cache hit):
- New input: 100 tokens × $15/M = $0.0015
- Cached input: 15000 tokens × $1.5/M = $0.0225  // 10% of normal price
- Output: 200 tokens × $75/M = $0.015
- Total: $0.039

Savings: $0.2415 - $0.039 = $0.2025 (84%)
```

---

## Current State Analysis

### What We Do Now

Current code-backed implementation:

1. ✅ Preserve and forward client `cache_control` markers, while recording whether the client supplied prompt-cache markers.
2. ✅ Capture `cache_creation_input_tokens` and `cache_read_input_tokens` from Anthropic and compatible usage payloads.
3. ✅ Preserve cache usage fields when Rust rebuilds Anthropic protocol responses.
4. ✅ Persist cache-token usage, `client_has_cache_control`, and `auto_cache_applied` into `gateway_request_audits` and usage-report flows.
5. ✅ Export baseline prompt-cache aggregate counters from `/metrics`.
6. ✅ Expose Rust internal prompt-cache summary and trend reads with hit rate, saved tokens, adoption counters, and estimated cost savings.
7. ✅ Surface operator prompt-cache analysis through `/ops/gateway/prompt-cache` via the transitional account-api to ai-gateway-domain path.
8. ✅ Surface user-facing benefit prompt-cache summary/trend reads for `service_proxy` flows without requiring operator privileges.

### Remaining Scope

The remaining work is not a Rust gateway release-candidate blocker. It is higher-level reporting and product integration work:

1. Final billing/quote policy for cached tokens.
2. Richer Prometheus/Grafana dashboards beyond the current aggregate counters.
3. Wider user documentation, tested examples, and screenshots.
4. Optional richer dashboard filters and historical reporting views.

---

## Industry Standard: What Other Gateways Do

### One-API / New-API

These commercial platforms **must** parse `usage` for billing:

```go
// Pseudo-code from One-API
func HandleAnthropicResponse(response *AnthropicResponse) {
    usage := response.Usage

    // Calculate costs
    inputCost := usage.InputTokens * ANTHROPIC_INPUT_PRICE
    cacheCreationCost := usage.CacheCreationInputTokens * ANTHROPIC_INPUT_PRICE
    cacheReadCost := usage.CacheReadInputTokens * ANTHROPIC_CACHE_READ_PRICE  // 10%
    outputCost := usage.OutputTokens * ANTHROPIC_OUTPUT_PRICE

    totalCost := inputCost + cacheCreationCost + cacheReadCost + outputCost

    // Store to database
    db.SaveUsageRecord(UsageRecord{
        UserID: user.ID,
        InputTokens: usage.InputTokens,
        CacheCreationTokens: usage.CacheCreationInputTokens,
        CacheReadTokens: usage.CacheReadInputTokens,
        OutputTokens: usage.OutputTokens,
        TotalCost: totalCost,
    })

    // Deduct user balance
    user.Balance -= totalCost
    db.UpdateUser(user)
}
```

**Why they do this**:
- They charge users based on actual token usage
- They need to show itemized billing (input/output/cached tokens)
- They need to track user balance and quota

---

## Design Decisions

### Decision 1: Parse `usage` Field

**Options**:
1. **Continue transparent forwarding**: Simple but no visibility
2. **Parse and store**: More complex but enables billing and analytics
3. **Optional parsing**: Parse only if user opts in

**Decision**: Parse and store (Option 2)

**Rationale**:
- ✅ Required for accurate billing
- ✅ Enables cost optimization insights
- ✅ Industry standard practice
- ✅ Minimal performance overhead
- ❌ Adds complexity to response handling

### Decision 2: Storage Strategy

**Options**:
1. **Extend `gateway_request_audits` table**: Add cache columns
2. **Create separate `gateway_cache_statistics` table**: Dedicated cache tracking
3. **Store only in Prometheus**: No persistent storage

**Decision**: Extend `gateway_request_audits` table (Option 1)

**Rationale**:
- ✅ Keep all request data together
- ✅ Easier to query (no joins needed)
- ✅ Consistent with existing audit pattern
- ❌ Slightly larger table size

**Schema**:
```sql
ALTER TABLE gateway_request_audits
ADD COLUMN IF NOT EXISTS cache_creation_input_tokens BIGINT DEFAULT 0,
ADD COLUMN IF NOT EXISTS cache_read_input_tokens BIGINT DEFAULT 0,
ADD COLUMN IF NOT EXISTS cache_cost_saved DECIMAL(10, 6) DEFAULT 0;

CREATE INDEX idx_request_audits_cache_read
ON gateway_request_audits(cache_read_input_tokens)
WHERE cache_read_input_tokens > 0;
```

### Decision 3: Metrics Strategy

**Metrics currently exported from Rust `/metrics`**:
```text
gateway_prompt_cache_hit_requests_total
gateway_prompt_cache_creation_requests_total
gateway_prompt_cache_client_marked_requests_total
gateway_prompt_cache_auto_applied_requests_total
gateway_prompt_cache_creation_input_tokens_total
gateway_prompt_cache_read_input_tokens_total
```

These counters are generated from `gateway_request_audits` by `Gateway/src/http/routes/metrics.rs`. Hit-rate, saved-token, adoption, and estimated-savings views are derived by the internal summary/trend read models rather than by a separate gauge metric.

### Decision 4: User Dashboard

**Current surfaces**:
- Operator analysis page: `/ops/gateway/prompt-cache`.
- Rust internal analysis reads: `/v1/internal/gateway/analysis/prompt-cache/summary` and `/v1/internal/gateway/analysis/prompt-cache/trend-report`.
- Project-scoped Rust reads for benefit-service flows: `/v1/internal/gateway/projects/:project_id/prompt-cache/summary` and `/v1/internal/gateway/projects/:project_id/prompt-cache/trend-report`.
- User-facing benefit routes: `/v1/me/benefits/services/:serviceId/prompt-cache-summary` and `/v1/me/benefits/services/:serviceId/prompt-cache-trend-report`.

**Displayed baseline**:
- Cache hit and creation request counts.
- Saved/read-token and creation-token totals.
- Client-marked versus Rust auto-applied adoption counters.
- Estimated savings from cached read tokens.
- Bucketed trend data for recent history.

---

## Implementation Strategy

The implementation is already landed in Rust. This section records the current code-backed components instead of the old week-based plan.

### Phase 1: Parse, preserve, and persist (Implemented)

- `Gateway/src/protocol/canonical.rs` carries `cache_creation_input_tokens` and `cache_read_input_tokens` in canonical token usage.
- `Gateway/src/protocol/anthropic.rs` captures Anthropic usage fields, preserves them when rebuilding responses, and records whether the request had client cache markers or Rust auto-applied them.
- `Gateway/src/redis/usage_tracking.rs` parses Anthropic and OpenAI-style cached-token usage payloads for usage reports.
- `Gateway/src/pipeline/stage_finalize.rs` persists cache-token usage plus `client_has_cache_control` and `auto_cache_applied` into request-audit and usage-report paths.
- `Gateway/src/db/request_audits.rs` stores the cache fields and exposes summary/trend reads.

**Acceptance criteria**:
- [x] Canonical token usage includes cache creation/read token fields.
- [x] Anthropic responses preserve cache usage in rebuilt responses.
- [x] Usage-report parsing handles Anthropic and OpenAI-style cached-token fields.
- [x] Request audits persist cache tokens and prompt-cache adoption telemetry.
- [x] Unit coverage exists for usage parsing, response preservation, and prompt-cache summary helpers.

---

### Phase 2: Metrics and monitoring (Implemented baseline; richer Grafana pending)

`Gateway/src/http/routes/metrics.rs` exports the current aggregate counters:

```text
gateway_prompt_cache_hit_requests_total
gateway_prompt_cache_creation_requests_total
gateway_prompt_cache_client_marked_requests_total
gateway_prompt_cache_auto_applied_requests_total
gateway_prompt_cache_creation_input_tokens_total
gateway_prompt_cache_read_input_tokens_total
```

Current summary/trend reads derive hit rate, saved tokens, adoption coverage, and estimated savings from persisted request-audit data. Richer Grafana panels remain future reporting work and should consume the actual counters/read models above.

**Acceptance criteria**:
- [x] Rust `/metrics` exports baseline prompt-cache counters.
- [x] Hit/creation/adoption/token totals are derived from `gateway_request_audits`.
- [x] Summary/trend reads expose hit rate, saved tokens, adoption counters, and estimated savings.
- [ ] Richer Grafana dashboards and alerting panels are finalized.

---

### Phase 3: Operator and user read models (Implemented baseline; richer filters pending)

Rust exposes internal read models through:

- `/v1/internal/gateway/analysis/prompt-cache/summary`
- `/v1/internal/gateway/analysis/prompt-cache/trend-report`
- `/v1/internal/gateway/projects/:project_id/prompt-cache/summary`
- `/v1/internal/gateway/projects/:project_id/prompt-cache/trend-report`

Operator and user-facing surfaces consume those reads through the current transitional web/API path:

- Operator page: `/ops/gateway/prompt-cache`.
- Benefit-service routes: `/v1/me/benefits/services/:serviceId/prompt-cache-summary` and `/v1/me/benefits/services/:serviceId/prompt-cache-trend-report`.
- Benefit-center overlay: personal hit rate, prompt-cache coverage, saved tokens, estimated savings, and recent bucketed trend.

**Acceptance criteria**:
- [x] Rust internal endpoints return prompt-cache summary and trend data.
- [x] Operator dashboard surfaces prompt-cache statistics through `/ops/gateway/prompt-cache`.
- [x] User-facing benefit flows surface personal summary and trend data without requiring operator privileges.
- [ ] Richer filters and long-horizon reporting are finalized.

---

### Phase 4: Documentation, billing product integration, and richer reporting (Pending product scope)

This remaining scope is not a code-local Rust gateway release-candidate blocker:

- Final cached-token billing and quote policy approval.
- Itemized billing product UI/API integration once that policy is approved.
- Wider user guide, tested examples, and screenshots.
- Richer Grafana/Prometheus dashboard breakdowns beyond the current aggregate counters.

**Acceptance criteria**:
- [ ] Final cached-token billing and quote policy is approved.
- [ ] Itemized billing product surfaces show cache-token breakdowns.
- [ ] Wider user-facing guide/examples/screenshots are complete.
- [ ] Richer reporting dashboards are finalized.

---

## Current Verification Strategy

- Rust unit coverage exercises Anthropic cache usage parsing, response reconstruction, usage-report parsing, and request-audit summary/trend helpers.
- `Gateway/tests/python/test_gateway_docs_consistency.py` guards this document and its `Gateway/` duplicate against stale metric names, old routes, and old week-plan labels.
- `scripts/verify-gateway-release-candidate.ps1` includes the safe-by-default local RC gate. Live-provider canary execution remains explicit opt-in and is not part of default/local/CI gates.
- Credential, quota, upstream-provider, and network failures remain classified separately from code-local RC gate failures.

---

## Cost Analysis

### Anthropic Pricing (Claude 3.5 Sonnet)

| Token Type | Price per Million |
|-----------|------------------|
| Input tokens | $15.00 |
| Cache write (creation) | $15.00 |
| Cache read (hit) | $1.50 (10% of input) |
| Output tokens | $75.00 |

### Example Calculation

**Scenario**: RAG application with 50-page document (15,000 tokens)

```
Without cache (100 requests):
- Input: 100 × 15,100 tokens × $15/M = $22.65
- Output: 100 × 200 tokens × $75/M = $1.50
- Total: $24.15

With cache (1 creation + 99 hits):
- First request: 15,100 × $15/M = $0.2265
- Cache hits: 99 × (100 × $15/M + 15,000 × $1.5/M) = $2.3715
- Output: 100 × 200 × $75/M = $1.50
- Total: $4.10

Savings: $24.15 - $4.10 = $20.05 (83%)
```

---

## Current Verification Strategy

Code-backed verification currently lives in:

- `Gateway/src/redis/usage_tracking.rs::test_parse_anthropic_usage`: verifies Anthropic `cache_creation_input_tokens` and `cache_read_input_tokens` parsing for usage reports.
- `Gateway/src/redis/usage_tracking.rs::test_parse_anthropic_case_insensitive`: verifies compatible cached-token field parsing variants.
- `Gateway/src/protocol/anthropic.rs`: unit coverage verifies cache-token usage capture and response reconstruction preserves `cache_creation_input_tokens` / `cache_read_input_tokens`.
- `Gateway/src/upstream/stream.rs`: unit coverage verifies streamed usage accumulation preserves cache-read tokens.
- `Gateway/src/db/request_audits.rs::prompt_cache_summary_tracks_client_and_auto_applied_counts`: verifies prompt-cache summary views include client-marked and Rust auto-applied adoption counters.
- `Gateway/tests/python/test_gateway_docs_consistency.py`: guards this document and its `Gateway/` duplicate against stale metric names, old routes, old week-plan labels, and old pseudo-test names.
- `scripts/verify-gateway-release-candidate.ps1`: includes the safe-by-default local RC gate. Live-provider canary execution remains explicit opt-in and is not part of default/local/CI gates.

Credential, quota, upstream-provider, and network failures remain classified separately from code-local RC gate failures.

---

## Current Rollout Status

Code-local Rust gateway rollout is complete for prompt-cache parsing, persistence, aggregate metrics, and read models. The remaining rollout scope is outside the code-local release-candidate gate and should be tracked as product/reporting work:

- Final cached-token billing and quote policy approval.
- Itemized billing product surface after that policy is approved.
- Wider user-facing guide, tested examples, and screenshots.
- Richer Grafana/Prometheus dashboards beyond the current aggregate counters.

Default/local/CI verification must continue to avoid live third-party calls. Live-provider canary execution remains explicit opt-in only and must continue to report credential, quota, upstream, or network failures separately from code-local RC gate failures.

---

## Acceptance Criteria

The Prompt Cache monitoring feature is complete when:

1. **Functionality**:
   - [x] Anthropic `usage` field is parsed correctly
   - [x] Cache metrics are stored to database-backed request audits
   - [x] Estimated cost savings are calculated for summary/trend reads
   - [x] Baseline Prometheus metrics are exported from Rust

2. **User Experience**:
   - [x] Operator dashboard surfaces prompt-cache statistics
   - [x] User-facing benefit flows surface personal summary and trend data
   - [ ] Richer dashboard filters and long-horizon reporting are finalized
   - [ ] Wider user documentation is complete

3. **Billing**:
   - [ ] Final cached-token billing and quote policy is approved
   - [ ] Itemized billing shows cache breakdown in the billing product surface
   - [x] Estimated cost savings are visible in prompt-cache read models

4. **Testing**:
   - [x] Unit tests cover cache usage parsing and summary helpers
   - [x] Integration-style contract coverage exists for the current read paths
   - [ ] End-to-end billing/product tests are complete after final billing policy approval

---

**Last Updated**: 2026-06-09
**Owner**: Rust Gateway Team
**Status**: Core prompt-cache monitoring implemented; final billing/quote integration and richer reporting docs pending