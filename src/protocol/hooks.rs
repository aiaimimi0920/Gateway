// ---------------------------------------------------------------------------
// Protocol hooks
//
// Provides pre-send / post-pack extension points for provider-specific
// request mutations without modifying the core pack/unpack logic.
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use serde_json::Value;

// ---------------------------------------------------------------------------
// HookContext
// ---------------------------------------------------------------------------

/// Contextual information passed to each hook invocation.
#[derive(Debug, Clone)]
pub struct HookContext {
    /// The model name being sent to the upstream (after alias resolution).
    pub model: String,
    /// The provider name, e.g. `"openai"`, `"mistral"`, `"anthropic"`.
    pub provider: String,
    /// The adapter name, e.g. `"openai_compatible"`, `"anthropic_compatible"`.
    pub adapter: String,
}

// ---------------------------------------------------------------------------
// ProtocolHook trait
// ---------------------------------------------------------------------------

/// Extension point for provider-specific request mutation.
///
/// Implementations are called in order after `pack_*` and before the HTTP
/// request is dispatched.  All methods have no-op defaults so implementations
/// only override what they need.
pub trait ProtocolHook: Send + Sync {
    /// Human-readable identifier for logging.
    fn name(&self) -> &str;

    /// Called after the request body has been packed.
    ///
    /// Mutate `body` in place to add, remove, or rewrite fields before the
    /// HTTP request is built.
    fn after_pack(&self, _ctx: &HookContext, _body: &mut Value) {}

    /// Called just before the HTTP request is dispatched.
    ///
    /// Mutate `headers` in place to inject or override HTTP headers.
    fn before_send(&self, _ctx: &HookContext, _headers: &mut HashMap<String, String>) {}
}

// ---------------------------------------------------------------------------
// Built-in hooks
// ---------------------------------------------------------------------------

/// Mistral's API requires tool-call IDs to be non-empty strings.
///
/// OpenAI tool call IDs are `call_<random>` strings.  Mistral rejects IDs
/// that look like `null` or are absent, so this hook ensures every tool call
/// in the packed body has a non-empty string ID.
pub struct MistralToolIdHook;

impl ProtocolHook for MistralToolIdHook {
    fn name(&self) -> &str {
        "mistral_tool_id"
    }

    fn after_pack(&self, _ctx: &HookContext, body: &mut Value) {
        let messages = match body.get_mut("messages").and_then(|m| m.as_array_mut()) {
            Some(m) => m,
            None => return,
        };

        for msg in messages.iter_mut() {
            if let Some(tool_calls) = msg.get_mut("tool_calls").and_then(|tc| tc.as_array_mut()) {
                for (i, tc) in tool_calls.iter_mut().enumerate() {
                    // Ensure `id` is a non-empty string.
                    let needs_fix = tc
                        .get("id")
                        .map(|v| v.is_null() || v.as_str().map(str::is_empty).unwrap_or(true))
                        .unwrap_or(true);

                    if needs_fix {
                        tc["id"] = serde_json::json!(format!("call_{i:08x}"));
                    }
                }
            }
        }
    }
}

/// Injects arbitrary additional HTTP headers before a request is sent.
///
/// Useful for provider-specific headers that are not covered by the standard
/// authentication logic (e.g., custom tracing headers, beta flags).
pub struct HeaderInjectionHook {
    /// Static headers to inject on every request for this provider.
    headers: HashMap<String, String>,
}

impl HeaderInjectionHook {
    pub fn new(headers: HashMap<String, String>) -> Self {
        Self { headers }
    }
}

impl ProtocolHook for HeaderInjectionHook {
    fn name(&self) -> &str {
        "header_injection"
    }

    fn before_send(&self, _ctx: &HookContext, headers: &mut HashMap<String, String>) {
        for (k, v) in &self.headers {
            headers.entry(k.clone()).or_insert_with(|| v.clone());
        }
    }
}

// ---------------------------------------------------------------------------
// resolve_hooks_for_provider
// ---------------------------------------------------------------------------

/// Return the list of [`ProtocolHook`]s that should run for the given provider.
///
/// Hooks are applied in the returned order.
pub fn resolve_hooks_for_provider(provider: &str) -> Vec<Box<dyn ProtocolHook>> {
    let mut hooks: Vec<Box<dyn ProtocolHook>> = Vec::new();

    match provider {
        "mistral" | "mistral-ai" => {
            hooks.push(Box::new(MistralToolIdHook));
        }
        _ => {}
    }

    hooks
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(provider: &str) -> HookContext {
        HookContext {
            model: "test-model".to_string(),
            provider: provider.to_string(),
            adapter: "openai_compatible".to_string(),
        }
    }

    // ── MistralToolIdHook ─────────────────────────────────────────────────

    #[test]
    fn mistral_hook_fills_null_tool_call_id() {
        let hook = MistralToolIdHook;
        let mut body = json!({
            "messages": [{
                "role": "assistant",
                "tool_calls": [
                    {"id": null, "type": "function", "function": {"name": "foo", "arguments": "{}"}},
                ]
            }]
        });
        hook.after_pack(&ctx("mistral"), &mut body);
        let id = body["messages"][0]["tool_calls"][0]["id"].as_str().unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn mistral_hook_fills_empty_string_tool_call_id() {
        let hook = MistralToolIdHook;
        let mut body = json!({
            "messages": [{
                "role": "assistant",
                "tool_calls": [
                    {"id": "", "type": "function", "function": {"name": "bar", "arguments": "{}"}},
                ]
            }]
        });
        hook.after_pack(&ctx("mistral"), &mut body);
        let id = body["messages"][0]["tool_calls"][0]["id"].as_str().unwrap();
        assert!(!id.is_empty());
    }

    #[test]
    fn mistral_hook_preserves_existing_tool_call_id() {
        let hook = MistralToolIdHook;
        let mut body = json!({
            "messages": [{
                "role": "assistant",
                "tool_calls": [
                    {"id": "call_abc123", "type": "function", "function": {"name": "baz", "arguments": "{}"}},
                ]
            }]
        });
        hook.after_pack(&ctx("mistral"), &mut body);
        assert_eq!(
            body["messages"][0]["tool_calls"][0]["id"],
            json!("call_abc123")
        );
    }

    #[test]
    fn mistral_hook_noop_when_no_messages() {
        let hook = MistralToolIdHook;
        let mut body = json!({"model": "mistral-7b"});
        hook.after_pack(&ctx("mistral"), &mut body); // should not panic
    }

    // ── HeaderInjectionHook ───────────────────────────────────────────────

    #[test]
    fn header_injection_inserts_missing_headers() {
        let mut static_headers = HashMap::new();
        static_headers.insert("x-custom-trace".to_string(), "trace-123".to_string());
        let hook = HeaderInjectionHook::new(static_headers);

        let mut headers = HashMap::new();
        hook.before_send(&ctx("custom"), &mut headers);
        assert_eq!(
            headers.get("x-custom-trace").map(String::as_str),
            Some("trace-123")
        );
    }

    #[test]
    fn header_injection_does_not_overwrite_existing_headers() {
        let mut static_headers = HashMap::new();
        static_headers.insert("authorization".to_string(), "Bearer injected".to_string());
        let hook = HeaderInjectionHook::new(static_headers);

        let mut headers = HashMap::new();
        headers.insert("authorization".to_string(), "Bearer original".to_string());
        hook.before_send(&ctx("custom"), &mut headers);
        // Original should be preserved.
        assert_eq!(
            headers.get("authorization").map(String::as_str),
            Some("Bearer original")
        );
    }

    // ── resolve_hooks_for_provider ────────────────────────────────────────

    #[test]
    fn resolve_returns_mistral_hook_for_mistral_provider() {
        let hooks = resolve_hooks_for_provider("mistral");
        assert_eq!(hooks.len(), 1);
        assert_eq!(hooks[0].name(), "mistral_tool_id");
    }

    #[test]
    fn resolve_returns_empty_for_unknown_provider() {
        let hooks = resolve_hooks_for_provider("openai");
        assert!(hooks.is_empty());
    }
}
