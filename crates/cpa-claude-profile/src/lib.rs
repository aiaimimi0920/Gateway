//! Opt-in CPA Claude Code shaping for custom Messages API-key providers, not OAuth authority.
mod betas;
mod body;
mod cache;
mod canonical;
mod date_pin;
mod identity;
mod request_kind;
pub use betas::headers as beta_headers;
pub use betas::select as select_beta_headers;
use serde_json::Value;

pub fn prepare(
    body: &mut Value,
    original: &Value,
    seed: &str,
    session_hint: Option<&str>,
    caller_scope: &str,
) -> Result<(), &'static str> {
    prepare_mode(
        body,
        original,
        seed,
        session_hint,
        caller_scope,
        "claude",
        false,
    )
}

/// `native` is determined by the authenticated ingress adapter, never by body text alone.
pub fn prepare_mode(
    body: &mut Value,
    original: &Value,
    seed: &str,
    session_hint: Option<&str>,
    caller_scope: &str,
    source_format: &str,
    native: bool,
) -> Result<(), &'static str> {
    if seed.trim().is_empty() {
        return Err("CPA CLI identity requires a selected credential");
    }
    if seed.starts_with("sk-ant-oat") {
        return Err("CPA CLI OAuth credential identity is not configured");
    }
    if !body.is_object() {
        return Err("CPA CLI body must be an object");
    }
    if !body["metadata"].is_null() && !body["metadata"].is_object() {
        return Err("CPA CLI metadata must be an object");
    }
    let native_session = if native {
        session_header(original).map(|s| format!("claude:{s}"))
    } else {
        None
    };
    let session = identity::session_for(
        original,
        session_hint.or(native_session.as_deref()),
        caller_scope,
        source_format,
    );
    let date = date_pin::pinned_date(seed, &session);
    let previous = body
        .pointer("/metadata/user_id")
        .and_then(Value::as_str)
        .map(str::to_owned);
    let user_id = identity::user_id(seed, &session, previous.as_deref());
    // Reject incompatible payloads without leaving a half-shaped retry request behind.
    let mut prepared = body.clone();
    let helper = request_kind::helper(body);
    let subagent = request_kind::subagent(original);
    let hour = betas::has_hour_cache(body);
    let explicit = cache::explicit(body);
    if native {
        cache::preserve(&mut prepared);
    } else {
        body::shape(&mut prepared, &date)?;
    }
    if !explicit && (helper || (subagent && !hour)) {
        cache::strip_ttl(&mut prepared);
    }
    prepared["metadata"]["user_id"] = Value::String(user_id);
    *body = prepared;
    Ok(())
}

pub fn session_header(body: &Value) -> Option<String> {
    let identity: Value =
        serde_json::from_str(body.pointer("/metadata/user_id")?.as_str()?).ok()?;
    Some(identity["session_id"].as_str()?.to_owned())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod cache_tests;
#[cfg(test)]
mod mode_tests;
