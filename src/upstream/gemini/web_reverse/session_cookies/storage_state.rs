//! Explicit imported-state seed for Canvas; HTTP checkpoint persistence never calls this.
use super::*;
use cookie_store::{CookieExpiration, CookiePath};
use serde_json::Value;

impl SessionCookies {
    pub(in crate::upstream) fn from_storage_state(
        base: &str,
        state: &Value,
    ) -> Result<Self, GatewayError> {
        let mut result = Self::new(base, &HeaderMap::new())?;
        let url = Url::parse(base).map_err(|_| invalid())?;
        let cookies = state
            .get("cookies")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        if cookies.len() > 512 {
            return Err(invalid());
        }
        for item in cookies {
            let domain = item
                .get("domain")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            let host = url.host_str().ok_or_else(invalid)?;
            let normalized = domain.trim_start_matches('.');
            let allowed_domain = normalized.eq_ignore_ascii_case(host)
                || (domain.starts_with('.')
                    && host == "gemini.google.com"
                    && normalized.eq_ignore_ascii_case("google.com"));
            if !allowed_domain || item.get("partitionKey").is_some_and(|v| !v.is_null()) {
                continue;
            }
            if domain.len() > 253
                || !domain
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || matches!(c, b'.' | b'-'))
            {
                return Err(invalid());
            }
            let name = item
                .get("name")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            let value = item
                .get("value")
                .and_then(Value::as_str)
                .ok_or_else(invalid)?;
            if name.is_empty()
                || !name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
                || !value
                    .bytes()
                    .all(|c| (0x21..=0x7e).contains(&c) && !b";,\"\\".contains(&c))
            {
                return Err(invalid());
            }
            let path = item.get("path").and_then(Value::as_str).unwrap_or("/");
            if path.len() > 4096
                || !path.starts_with('/')
                || path.bytes().any(|c| c < 0x20 || c == 0x7f || c == b';')
            {
                return Err(invalid());
            }
            let secure = item.get("secure").and_then(Value::as_bool).unwrap_or(false);
            let mut raw = format!("{name}={value}; Path={path}");
            // A non-dot Playwright domain is host-only, not a Domain attribute.
            if domain.starts_with('.') {
                raw.push_str(&format!("; Domain={domain}"));
            }
            if secure {
                raw.push_str("; Secure");
            }
            if raw.len() > MAX_COOKIE_BYTES {
                return Err(invalid());
            }
            let mut cookie = Cookie::parse(raw, &url)
                .map_err(|_| invalid())?
                .into_owned();
            cookie.path = CookiePath::parse(path).ok_or_else(invalid)?;
            cookie.expires = match item.get("expires").filter(|v| !v.is_null()) {
                None => CookieExpiration::SessionEnd,
                Some(value) => {
                    let seconds = value
                        .as_f64()
                        .filter(|n| n.is_finite() && *n >= -1.0 && *n < i64::MAX as f64)
                        .ok_or_else(invalid)?;
                    if seconds == -1.0 {
                        CookieExpiration::SessionEnd
                    } else {
                        CookieExpiration::AtUtc(
                            time::OffsetDateTime::from_unix_timestamp(seconds as i64)
                                .map_err(|_| invalid())?,
                        )
                    }
                }
            };
            if !allowed_cookie(&cookie, &url) || cookie.is_expired() {
                continue;
            }
            result.store.insert(cookie, &url).map_err(|_| invalid())?;
            if !result.order.iter().any(|existing| existing == name) {
                result.order.push(name.to_string());
            }
            if !within_limits(&result.store) {
                return Err(invalid());
            }
        }
        Ok(result)
    }
}

#[cfg(test)]
#[path = "storage_state_tests.rs"]
mod tests;
