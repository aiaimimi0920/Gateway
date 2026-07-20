# Automatic Prompt Caching Design

## Implementation Status

Current Rust gateway status as of 2026-04-14:

- `Phase 1` detection is partially implemented on the request packing path:
  - Rust now detects existing Anthropic `cache_control` markers on `system` and `tools`
  - Native Anthropic requests keep their original cached `system` / `tools` blocks during repacking instead of losing them through canonical flattening
- `Phase 2` simple auto-cache is partially implemented:
  - only Claude-model Anthropic upstream requests are eligible
  - only requests without existing `cache_control` markers are mutated
  - Rust currently auto-marks the last `system` block and last tool with `{"type":"ephemeral"}`
- Supporting observability is now partially implemented:
  - persistent request-audit columns for cache token usage are live
  - Rust finalize writes cache usage through the audit path
  - baseline Prometheus cache counters are exported from `/metrics`
  - Rust internal prompt-cache summary/trend endpoints now expose hit rate, saved tokens, and estimated cost savings
  - Rust now also persists `client_has_cache_control` and `auto_cache_applied` per request, so we can separate client-supplied Anthropic caching from Rust auto-cache adoption
  - Rust summary/trend reads and `/metrics` now expose adoption counters for `client marked` vs `auto applied`
  - Web operator console now surfaces prompt-cache reads at `/ops/gateway/prompt-cache`, and the current `account-api -> ai-gateway-domain` transitional path mirrors the Rust summary/trend contract for that page
- User-facing AI benefit `service_proxy` flows now also surface rolling personal prompt-cache summary + trend data in the benefit-center overlay, backed by `/v1/me/benefits/services/:serviceId/prompt-cache-summary` and `/v1/me/benefits/services/:serviceId/prompt-cache-trend-report`
- Later phases in this document are still pending:
  - static/dynamic block analysis
  - dashboard-level cache effectiveness views
  - final billing/quote integration for cached token pricing

## Purpose

This document defines the design for automatic prompt caching in the Rust gateway. The feature intelligently adds `cache_control` markers to requests that don't already have them, providing cost savings for clients that don't implement their own caching logic.

## Core Principles

**1. Only apply to Anthropic Claude models**

`cache_control` 是 Anthropic 特有的功能，只对发往 Anthropic Claude API 的请求应用自动缓存。

**关键限制：**
- ✅ Anthropic Claude 模型：支持 `cache_control`
- ❌ OpenAI GPT 模型：不支持，添加会导致 400 错误
- ❌ Google Gemini 模型：不支持，添加会导致请求失败
- ❌ 其他第三方模型：不支持

**实现检查：**
```rust
// 必须在路由阶段检查目标提供商
fn is_anthropic_provider(route_policy: &RoutePolicy) -> bool {
    route_policy.provider == "anthropic"
}
```

**2. Only apply to requests that DON'T already contain `cache_control` markers**

如果客户端（如 Claude Code）已经发送了 `cache_control` 参数，说明他们已经实现了自己的智能缓存策略。我们应该尊重他们的选择，不要干预。

**检测逻辑：**
- 检查 `system` 数组中的所有块是否包含 `cache_control`
- 检查 `tools` 数组中的所有工具是否包含 `cache_control`
- 如果发现任何现有标记，跳过自动缓存

## How Claude Code Implements Auto-Caching

Based on analysis of a local Claude Code source checkout, here's what they do:

### 1. System Prompt Caching Strategy

Claude Code splits system prompts into multiple blocks with different cache scopes:

```typescript
// From src/utils/api.ts:321 (splitSysPromptPrefix)
export function splitSysPromptPrefix(
  systemPrompt: SystemPrompt,
  options?: { skipGlobalCacheForSystemPrompt?: boolean },
): SystemPromptBlock[]
```

**Three caching modes:**

1. **Tool-based caching** (when MCP tools present):
   - Attribution header: `cacheScope=null` (no cache)
   - System prompt prefix: `cacheScope='org'`
   - Rest of system prompt: `cacheScope='org'`

2. **Global cache mode** (1P only, with boundary marker):
   - Attribution header: `cacheScope=null`
   - System prompt prefix: `cacheScope=null`
   - Static content before boundary: `cacheScope='global'`
   - Dynamic content after boundary: `cacheScope=null`

3. **Default mode** (3P providers or no boundary):
   - Attribution header: `cacheScope=null`
   - System prompt prefix: `cacheScope='org'`
   - Rest: `cacheScope='org'`

**Key insight:** They use a `SYSTEM_PROMPT_DYNAMIC_BOUNDARY` marker to split static vs dynamic content.

### 2. Tool Schema Caching

```typescript
// From src/utils/api.ts:119 (toolToAPISchema)
if (options.cacheControl) {
  schema.cache_control = options.cacheControl
}
```

Tools are marked with:
```typescript
{
  type: 'ephemeral',
  scope?: 'global' | 'org',
  ttl?: '5m' | '1h'
}
```

**Cache scope logic:**
- `global`: For 1P Anthropic API only, shared across all users
- `org`: Organization-level cache, shared within tenant
- TTL: `1h` for subscribers/ants, `5m` default

### 3. Cache Break Detection

Claude Code tracks cache effectiveness with `promptCacheBreakDetection.ts`:

```typescript
// Phase 1: Record state before API call
recordPromptState({
  system,
  toolSchemas,
  querySource,
  model,
  fastMode,
  globalCacheStrategy,
  betas,
  effortValue,
  extraBodyParams
})

// Phase 2: Check response for cache break
checkResponseForCacheBreak(
  querySource,
  cacheReadTokens,
  cacheCreationTokens,
  messages,
  agentId,
  requestId
)
```

They detect cache breaks by:
- Hashing system prompt (stripped of `cache_control`)
- Hashing tool schemas
- Comparing `cache_read_input_tokens` between requests
- Logging when cache drops >5% or >2000 tokens

## Our Gateway's Auto-Cache Strategy

### Detection Logic

**Step 1: Check if request already has caching**

```rust
// gateway/src/protocol/anthropic.rs
fn has_existing_cache_control(body: &serde_json::Value) -> bool {
    // Check system blocks
    if let Some(system) = body.get("system").and_then(|s| s.as_array()) {
        for block in system {
            if block.get("cache_control").is_some() {
                return true;
            }
        }
    }
    
    // Check tools
    if let Some(tools) = body.get("tools").and_then(|t| t.as_array()) {
        for tool in tools {
            if tool.get("cache_control").is_some() {
                return true;
            }
        }
    }
    
    false
}
```

**Step 2: Apply auto-cache only if no existing markers**

```rust
pub fn apply_auto_cache(
    body: &mut serde_json::Value,
    session: &AuthenticatedSession,
) -> Result<(), GatewayError> {
    // Skip if client already handles caching
    if has_existing_cache_control(body) {
        tracing::debug!("Request already has cache_control, skipping auto-cache");
        return Ok(());
    }
    
    // Apply our intelligent caching
    add_cache_markers(body, session)?;
    Ok(())
}
```

### Caching Heuristics

**System Prompt Caching:**

1. **Detect static vs dynamic content**
   - Static: Identity, capabilities, rules (rarely changes)
   - Dynamic: Current date, git status, file context (changes per session)

2. **Split strategy:**
   ```rust
   // Pseudo-code
   let blocks = split_system_prompt(system_text);
   
   // First 1-2 blocks: likely static instructions
   blocks[0].cache_control = Some(CacheControl {
       type_: "ephemeral",
       scope: Some("org"),
       ttl: Some("1h"),
   });
   
   // Last block: likely dynamic context
   // No cache marker (changes frequently)
   ```

3. **Heuristics for static content:**
   - Starts with "You are..." or "# Identity"
   - Contains "# Rules", "# Capabilities", "# Tools"
   - No dates, no file paths, no git hashes
   - Length > 1000 chars (substantial instruction block)

**Tool Schema Caching:**

```rust
// Cache all tools together (they rarely change mid-session)
if let Some(tools) = body.get_mut("tools").and_then(|t| t.as_array_mut()) {
    if let Some(last_tool) = tools.last_mut() {
        last_tool["cache_control"] = json!({
            "type": "ephemeral",
            "scope": "org",
            "ttl": "1h"
        });
    }
}
```

**Why cache the last tool?** Anthropic's prompt caching caches everything UP TO AND INCLUDING the marked block. Marking the last tool caches all tools.

### Cache Scope Selection

```rust
pub enum CacheScope {
    Org,    // Organization-level (default)
    Global, // Cross-organization (1P only, requires special permission)
}

fn determine_cache_scope(session: &AuthenticatedSession) -> CacheScope {
    // Global cache only for:
    // 1. First-party Anthropic API
    // 2. User has global cache permission
    // 3. Content is truly global (no tenant-specific data)
    
    if session.has_scope("cache:global") {
        CacheScope::Global
    } else {
        CacheScope::Org
    }
}
```

### TTL Selection

```rust
pub enum CacheTTL {
    FiveMinutes,  // Default
    OneHour,      // For subscribers/premium users
}

fn determine_cache_ttl(session: &AuthenticatedSession) -> CacheTTL {
    // 1h TTL for:
    // - Subscribers
    // - Enterprise users
    // - Users with explicit permission
    
    if session.has_scope("cache:1h") || session.is_subscriber() {
        CacheTTL::OneHour
    } else {
        CacheTTL::FiveMinutes
    }
}
```

## Implementation Phases

### Phase 1: Detection and Passthrough (Week 1)

**Goal:** Detect existing cache markers and log statistics.

**Tasks:**
1. Implement `has_existing_cache_control()` detection
2. Add metrics:
   - `gateway_requests_with_cache_control_total`
   - `gateway_requests_without_cache_control_total`
3. Log to database for analysis:
   ```sql
   ALTER TABLE gateway_request_audits ADD COLUMN client_has_cache_control BOOLEAN;
   ```

**Exit criteria:**
- [ ] Can detect Claude Code requests (have cache_control)
- [ ] Can detect other clients (no cache_control)
- [ ] Metrics show ~50% of requests already have caching (Claude Code users)

### Phase 2: Simple Auto-Cache (Week 2)

**Goal:** Apply basic caching to requests without existing markers.

**Implementation:**
```rust
// gateway/src/pipeline/stage_route.rs
pub async fn apply_auto_cache(
    body: &mut serde_json::Value,
    session: &AuthenticatedSession,
) -> Result<(), GatewayError> {
    if has_existing_cache_control(body) {
        return Ok(());
    }
    
    // Simple strategy: cache last system block + last tool
    if let Some(system) = body.get_mut("system").and_then(|s| s.as_array_mut()) {
        if let Some(last_block) = system.last_mut() {
            last_block["cache_control"] = json!({
                "type": "ephemeral",
                "scope": "org"
            });
        }
    }
    
    if let Some(tools) = body.get_mut("tools").and_then(|t| t.as_array_mut()) {
        if let Some(last_tool) = tools.last_mut() {
            last_tool["cache_control"] = json!({
                "type": "ephemeral",
                "scope": "org"
            });
        }
    }
    
    Ok(())
}
```

**Exit criteria:**
- [ ] Auto-cache applied to non-Claude-Code requests
- [ ] No cache markers added to Claude Code requests
- [ ] Metrics show cache hit rate improvement for auto-cached requests

### Phase 3: Intelligent Content Detection (Week 3-4)

**Goal:** Split system prompts into static vs dynamic blocks.

**Implementation:**
```rust
pub struct SystemPromptAnalyzer {
    static_patterns: Vec<Regex>,
    dynamic_patterns: Vec<Regex>,
}

impl SystemPromptAnalyzer {
    pub fn analyze(&self, text: &str) -> ContentType {
        // Check for static indicators
        if self.is_static_content(text) {
            ContentType::Static
        } else {
            ContentType::Dynamic
        }
    }
    
    fn is_static_content(&self, text: &str) -> bool {
        // Heuristics:
        // - Starts with identity/role declaration
        // - Contains rule/capability sections
        // - No dates (2026-04-13)
        // - No machine-specific absolute file paths
        // - No git hashes (fc091ee73)
        // - Length > 1000 chars
        
        let has_identity = text.starts_with("You are") || text.contains("# Identity");
        let has_rules = text.contains("# Rules") || text.contains("# Capabilities");
        let has_dates = Regex::new(r"\d{4}-\d{2}-\d{2}").unwrap().is_match(text);
        let has_paths = text.contains(":\\") || text.contains("/home/");
        let is_substantial = text.len() > 1000;
        
        has_identity && has_rules && !has_dates && !has_paths && is_substantial
    }
}
```

**Exit criteria:**
- [ ] Can distinguish static vs dynamic system prompt blocks
- [ ] Cache hit rate > 60% for auto-cached requests
- [ ] Cost savings visible in usage metrics

### Phase 4: Advanced Features (Week 5+)

**Optional enhancements:**

1. **Per-endpoint caching strategies:**
   ```rust
   match endpoint {
       "/v1/messages" => apply_messages_cache_strategy(body),
       "/v1/chat/completions" => apply_chat_cache_strategy(body),
       _ => apply_default_cache_strategy(body),
   }
   ```

2. **User-configurable caching:**
   ```rust
   // Allow users to opt-in/opt-out via API key metadata
   if session.cache_preference == CachePreference::Disabled {
       return Ok(());
   }
   ```

3. **Cache effectiveness feedback:**
   ```rust
   // Track cache performance per user
   let cache_stats = CacheStats {
       requests_with_cache: 100,
       cache_hit_rate: 0.75,
       tokens_saved: 50_000,
       cost_saved_usd: 0.75,
   };
   ```

## Monitoring and Metrics

### Prometheus Metrics

```rust
// gateway/src/metrics.rs
lazy_static! {
    // Detection metrics
    pub static ref REQUESTS_WITH_EXISTING_CACHE: IntCounter = register_int_counter!(
        "gateway_requests_with_existing_cache_total",
        "Requests that already have cache_control markers"
    ).unwrap();
    
    pub static ref REQUESTS_AUTO_CACHED: IntCounter = register_int_counter!(
        "gateway_requests_auto_cached_total",
        "Requests where we added cache_control markers"
    ).unwrap();
    
    // Effectiveness metrics
    pub static ref AUTO_CACHE_HIT_RATE: Histogram = register_histogram!(
        "gateway_auto_cache_hit_rate",
        "Cache hit rate for auto-cached requests"
    ).unwrap();
    
    pub static ref AUTO_CACHE_TOKENS_SAVED: IntCounter = register_int_counter!(
        "gateway_auto_cache_tokens_saved_total",
        "Tokens saved via auto-caching"
    ).unwrap();
    
    pub static ref AUTO_CACHE_COST_SAVED: Counter = register_counter!(
        "gateway_auto_cache_cost_saved_usd",
        "Cost saved via auto-caching (USD)"
    ).unwrap();
}
```

### Database Schema

```sql
-- Extend gateway_request_audits table
ALTER TABLE gateway_request_audits ADD COLUMN IF NOT EXISTS
    client_has_cache_control BOOLEAN DEFAULT FALSE,
    auto_cache_applied BOOLEAN DEFAULT FALSE,
    auto_cache_strategy TEXT, -- 'simple', 'intelligent', 'disabled'
    cache_blocks_added INTEGER DEFAULT 0;

-- New table for cache effectiveness tracking
CREATE TABLE IF NOT EXISTS gateway_cache_effectiveness (
    id TEXT PRIMARY KEY,
    user_id TEXT NOT NULL,
    project_id TEXT NOT NULL,
    request_id TEXT NOT NULL,
    
    -- Cache configuration
    auto_cache_applied BOOLEAN NOT NULL,
    cache_strategy TEXT NOT NULL,
    
    -- Cache performance
    cache_creation_tokens INTEGER DEFAULT 0,
    cache_read_tokens INTEGER DEFAULT 0,
    cache_hit_rate REAL, -- 0.0 to 1.0
    
    -- Cost savings
    tokens_saved INTEGER DEFAULT 0,
    cost_saved_usd REAL DEFAULT 0.0,
    
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    
    INDEX idx_cache_effectiveness_user (user_id, created_at),
    INDEX idx_cache_effectiveness_project (project_id, created_at)
);
```

### Dashboard Queries

```sql
-- Auto-cache adoption rate
SELECT 
    DATE(created_at) as date,
    COUNT(*) FILTER (WHERE auto_cache_applied) as auto_cached,
    COUNT(*) FILTER (WHERE client_has_cache_control) as client_cached,
    COUNT(*) FILTER (WHERE NOT auto_cache_applied AND NOT client_has_cache_control) as no_cache,
    COUNT(*) as total
FROM gateway_request_audits
WHERE created_at > NOW() - INTERVAL '7 days'
GROUP BY DATE(created_at)
ORDER BY date DESC;

-- Cost savings by user
SELECT 
    user_id,
    COUNT(*) as requests,
    SUM(tokens_saved) as total_tokens_saved,
    SUM(cost_saved_usd) as total_cost_saved_usd,
    AVG(cache_hit_rate) as avg_cache_hit_rate
FROM gateway_cache_effectiveness
WHERE created_at > NOW() - INTERVAL '30 days'
GROUP BY user_id
ORDER BY total_cost_saved_usd DESC
LIMIT 100;

-- Cache effectiveness by strategy
SELECT 
    cache_strategy,
    COUNT(*) as requests,
    AVG(cache_hit_rate) as avg_hit_rate,
    SUM(tokens_saved) as total_tokens_saved
FROM gateway_cache_effectiveness
WHERE created_at > NOW() - INTERVAL '7 days'
GROUP BY cache_strategy
ORDER BY avg_hit_rate DESC;
```

## Configuration

### Environment Variables

```bash
# Enable/disable auto-cache feature
GATEWAY_AUTO_CACHE_ENABLED=true

# Default cache TTL (5m or 1h)
GATEWAY_AUTO_CACHE_DEFAULT_TTL=5m

# Default cache scope (org or global)
GATEWAY_AUTO_CACHE_DEFAULT_SCOPE=org

# Minimum system prompt length to cache (chars)
GATEWAY_AUTO_CACHE_MIN_SYSTEM_LENGTH=1000

# Minimum tool count to cache
GATEWAY_AUTO_CACHE_MIN_TOOL_COUNT=3
```

### Per-User Configuration

```rust
// Store in user credential metadata
{
    "cache_preference": "auto",  // "auto", "disabled", "aggressive"
    "cache_ttl": "1h",           // "5m", "1h"
    "cache_scope": "org"         // "org", "global"
}
```

## Cost Savings Estimation

Based on Anthropic pricing:
- Input tokens: $15/M
- Cached input tokens: $1.5/M (10% of normal)
- Savings: $13.5/M cached tokens

**Example scenario:**
- User makes 100 requests/day
- Each request has 20K token system prompt + 5K token tools
- Cache hit rate: 75%

**Calculation:**
```
Cacheable tokens per request: 25K
Cached tokens per day: 25K × 100 × 0.75 = 1.875M
Cost without cache: 1.875M × $15/M = $28.13
Cost with cache: 1.875M × $1.5/M = $2.81
Daily savings: $25.32
Monthly savings: $759.60
```

## Security Considerations

### 1. Cache Scope Isolation

```rust
// Ensure org-scoped cache doesn't leak between tenants
fn validate_cache_scope(
    scope: CacheScope,
    session: &AuthenticatedSession,
) -> Result<(), GatewayError> {
    match scope {
        CacheScope::Global => {
            if !session.has_scope("cache:global") {
                return Err(GatewayError::Forbidden(
                    "Global cache scope requires special permission".into()
                ));
            }
        }
        CacheScope::Org => {
            // Always allowed
        }
    }
    Ok(())
}
```

### 2. Sensitive Data Detection

```rust
// Don't cache blocks that might contain sensitive data
fn contains_sensitive_data(text: &str) -> bool {
    let patterns = [
        r"password",
        r"api[_-]?key",
        r"secret",
        r"token",
        r"credential",
        r"\b[A-Za-z0-9]{32,}\b", // Long hex strings
    ];
    
    patterns.iter().any(|p| {
        Regex::new(p).unwrap().is_match(&text.to_lowercase())
    })
}
```

### 3. Cache Poisoning Prevention

```rust
// Validate cache_control parameters from client
fn validate_cache_control(
    cache_control: &serde_json::Value,
) -> Result<(), GatewayError> {
    // Only allow known types
    if cache_control.get("type") != Some(&json!("ephemeral")) {
        return Err(GatewayError::BadRequest(
            "Only ephemeral cache type is supported".into()
        ));
    }
    
    // Validate scope
    if let Some(scope) = cache_control.get("scope") {
        if scope != "org" && scope != "global" {
            return Err(GatewayError::BadRequest(
                "Invalid cache scope".into()
            ));
        }
    }
    
    // Validate TTL
    if let Some(ttl) = cache_control.get("ttl") {
        if ttl != "5m" && ttl != "1h" {
            return Err(GatewayError::BadRequest(
                "Invalid cache TTL".into()
            ));
        }
    }
    
    Ok(())
}
```

## Testing Strategy

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_detect_existing_cache_control() {
        let body = json!({
            "system": [
                {
                    "type": "text",
                    "text": "You are a helpful assistant",
                    "cache_control": {"type": "ephemeral"}
                }
            ]
        });
        
        assert!(has_existing_cache_control(&body));
    }
    
    #[test]
    fn test_no_existing_cache_control() {
        let body = json!({
            "system": [
                {
                    "type": "text",
                    "text": "You are a helpful assistant"
                }
            ]
        });
        
        assert!(!has_existing_cache_control(&body));
    }
    
    #[test]
    fn test_apply_auto_cache_skips_when_exists() {
        let mut body = json!({
            "system": [
                {
                    "type": "text",
                    "text": "Test",
                    "cache_control": {"type": "ephemeral"}
                }
            ]
        });
        
        let session = create_test_session();
        apply_auto_cache(&mut body, &session).unwrap();
        
        // Should not add more cache markers
        assert_eq!(
            body["system"][0]["cache_control"],
            json!({"type": "ephemeral"})
        );
    }
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_auto_cache_end_to_end() {
    let gateway = setup_test_gateway().await;
    
    // Request without cache_control
    let request = json!({
        "model": "claude-opus-4",
        "messages": [{"role": "user", "content": "Hello"}],
        "system": [{"type": "text", "text": "You are helpful"}],
        "tools": [{"name": "test", "description": "Test tool"}]
    });
    
    let response = gateway.handle_request(request).await.unwrap();
    
    // Verify cache markers were added
    let audit = get_request_audit(&response.request_id).await;
    assert!(audit.auto_cache_applied);
    assert_eq!(audit.cache_blocks_added, 2); // system + tools
}
```

## Rollout Plan

### Week 1: Shadow Mode
- Deploy detection logic
- Log statistics but don't modify requests
- Analyze: What % of requests already have cache_control?

### Week 2: Canary (5% traffic)
- Enable auto-cache for 5% of requests without existing markers
- Monitor cache hit rates and cost savings
- Verify no negative impact on latency

### Week 3-4: Gradual Rollout
- 25% → 50% → 75% → 100%
- Monitor metrics at each step
- Rollback if cache hit rate < 50% or errors increase

### Week 5: Full Production
- 100% of requests without existing cache_control get auto-cache
- Dashboard showing cost savings per user
- Documentation for users to opt-out if needed

## Success Criteria

1. **Detection accuracy:** 99%+ correct identification of existing cache_control
2. **Cache hit rate:** >60% for auto-cached requests
3. **Cost savings:** >50% reduction in input token costs for auto-cached users
4. **No interference:** 0% of Claude Code requests get modified
5. **Performance:** <1ms overhead for auto-cache logic

---

**Last Updated:** 2026-04-13  
**Owner:** Rust Gateway Team  
**Status:** Design Phase
