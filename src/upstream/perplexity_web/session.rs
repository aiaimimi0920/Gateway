//! 仅接收显式、未分片的本站 session 值；完整 Cookie/storage-state 的导入属于材料边界。
use crate::{
    error::GatewayError,
    protocol::perplexity_web::{input_failure, material},
    routing::candidate::ProviderAccountPayload,
};
use rquest::header::{HeaderMap, HeaderValue};

pub struct Session {
    cookie: HeaderValue,
    user_agent: HeaderValue,
}

impl Session {
    /// Normalized account material is audience-bound and checked again immediately before sending.
    pub fn from_payload(payload: &ProviderAccountPayload) -> Result<Self, GatewayError> {
        if !material::owns_payload(payload) {
            return Err(material::invalid());
        }
        material::validate(payload, payload.canonical_adapter())?;
        if material::expiry(payload)?
            .is_some_and(|expiry| expiry <= time::OffsetDateTime::now_utc())
        {
            return Err(input_failure(
                "perplexity_web_session_expired",
                "Perplexity Web 会话已过期；请重新导入，未自动刷新或发送。",
            ));
        }
        let parts = material::parts(payload)?;
        Self::from_tokens(parts.token, parts.clearance, parts.user_agent)
    }

    /// 材料形状有效不代表已登录、未过期或拥有任何订阅额度。
    pub fn from_tokens(
        session_token: &str,
        cf_clearance: Option<&str>,
        user_agent: &str,
    ) -> Result<Self, GatewayError> {
        if !material::cookie_value(session_token)
            || cf_clearance.is_some_and(|value| !material::cookie_value(value))
            || user_agent.trim().is_empty()
            || user_agent.len() > 1024
            || !user_agent.bytes().all(|v| matches!(v, 0x20..=0x7e))
        {
            return Err(invalid());
        }
        let mut cookie = format!("__Secure-next-auth.session-token={session_token}");
        if let Some(clearance) = cf_clearance {
            cookie.push_str("; cf_clearance=");
            cookie.push_str(clearance);
        }
        let mut cookie = HeaderValue::from_str(&cookie).map_err(|_| invalid())?;
        cookie.set_sensitive(true);
        Ok(Self {
            cookie,
            user_agent: HeaderValue::from_str(user_agent).map_err(|_| invalid())?,
        })
    }

    pub(super) fn headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("cookie", self.cookie.clone());
        headers.insert("user-agent", self.user_agent.clone());
        for (name, value) in [
            ("accept", "text/event-stream"),
            ("content-type", "application/json"),
            ("accept-language", "en-US,en;q=0.9"),
            ("origin", "https://www.perplexity.ai"),
            ("referer", "https://www.perplexity.ai/"),
            ("sec-fetch-mode", "cors"),
            ("sec-fetch-dest", "empty"),
            ("sec-fetch-site", "same-origin"),
        ] {
            headers.insert(name, HeaderValue::from_static(value));
        }
        headers
    }
}

fn invalid() -> GatewayError {
    input_failure(
        "perplexity_web_material_invalid",
        "Perplexity Web session 材料无效；未发送请求。",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perplexity_web_session_rejects_missing_login_and_cookie_injection() {
        for value in [
            "",
            "a;b=c",
            "a\r\nAuthorization: leak",
            "a b",
            "\"a\"",
            "中文",
        ] {
            assert!(Session::from_tokens(value, Some("challenge"), "fixture-agent").is_err());
        }
        assert!(Session::from_tokens("valid", Some("bad;cookie"), "fixture-agent").is_err());
        assert!(Session::from_tokens("valid", None, "bad\nagent").is_err());
    }

    #[test]
    fn perplexity_web_session_limits_material_and_marks_cookie_sensitive() {
        assert!(Session::from_tokens(&"x".repeat(16 * 1024 + 1), None, "fixture").is_err());
        let session =
            Session::from_tokens("private-session", Some("private-challenge"), "fixture").unwrap();
        let headers = session.headers();
        assert_eq!(
            headers["cookie"].to_str().unwrap(),
            "__Secure-next-auth.session-token=private-session; cf_clearance=private-challenge"
        );
        assert!(headers["cookie"].is_sensitive());
        assert!(!format!("{headers:?}").contains("private-session"));
        assert!(!headers.contains_key("authorization"));
    }
}
