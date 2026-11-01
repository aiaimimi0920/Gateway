//! Domain/path/expiry-aware, request-local Cookie jar; never a shared browser profile.
use super::failure;
use crate::error::GatewayError;
use cookie_store::{Cookie, CookieStore};
use rquest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use serde::Deserialize;
use std::collections::HashSet;
use url::Url;

const ORIGIN: &str = "https://flow.google.com";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Bundle {
    schema_version: u8,
    provider: String,
    auth_user: String,
    cookies: Vec<Seed>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Seed {
    name: String,
    value: String,
    domain: String,
    path: String,
    expires: f64,
    secure: bool,
    http_only: Option<bool>,
    same_site: Option<String>,
}

pub(super) struct FlowCookies(CookieStore);

fn invalid() -> GatewayError {
    failure(
        "flow_web_session_invalid",
        "Flow Cookie 材料无效、过期或作用域不匹配。",
    )
}
fn target(url: &str) -> Result<Url, GatewayError> {
    let parsed = Url::parse(url).map_err(|_| invalid())?;
    if parsed.origin().ascii_serialization() != ORIGIN
        || !parsed.username().is_empty()
        || parsed.password().is_some()
    {
        return Err(invalid());
    }
    Ok(parsed)
}
fn allowed_domain(domain: &str) -> bool {
    matches!(
        domain,
        ".google.com" | "flow.google.com" | ".flow.google.com"
    )
}

impl FlowCookies {
    pub(super) fn bundle_bytes(&self) -> Result<Vec<u8>, GatewayError> {
        let cookies: Vec<_> = self.0.iter_unexpired().map(|cookie| serde_json::json!({
            "name":cookie.name(), "value":cookie.value(), "path":cookie.path.to_string(),
            "domain":cookie.domain().map(|domain| format!(".{}", domain.trim_start_matches('.'))).unwrap_or_else(|| "flow.google.com".into()),
            "secure":cookie.secure().unwrap_or(false), "httpOnly":cookie.http_only().unwrap_or(false),
            "sameSite":cookie.same_site().map(|site| site.to_string()),
            "expires":match &cookie.expires {
                cookie_store::CookieExpiration::AtUtc(time) => time.unix_timestamp(),
                cookie_store::CookieExpiration::SessionEnd => -1,
            }
        })).collect();
        let bytes = serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"provider":"flow-web","authUser":"0","cookies":cookies})).map_err(|_| invalid())?;
        // Revalidate the exact import contract, including a still-current root authentication Cookie.
        Self::from_bundle(&bytes)?;
        Ok(bytes)
    }

    pub(super) fn from_bundle(bytes: &[u8]) -> Result<Self, GatewayError> {
        if bytes.len() > 32768 {
            return Err(invalid());
        }
        let bundle: Bundle = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        if bundle.schema_version != 1
            || bundle.provider != "flow-web"
            || bundle.auth_user != "0"
            || bundle.cookies.is_empty()
            || bundle.cookies.len() > 64
        {
            return Err(invalid());
        }
        let base = target(ORIGIN)?;
        let mut store = CookieStore::default();
        let mut seen = HashSet::new();
        for seed in bundle.cookies {
            if !allowed_domain(&seed.domain)
                || seed.name.is_empty()
                || seed.name.len() > 256
                || !seed
                    .name
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&c))
                || !seed
                    .value
                    .bytes()
                    .all(|c| (0x21..=0x7e).contains(&c) && !b";,\"\\".contains(&c))
                || seed.value.len() > 8192
                || !seed.path.starts_with('/')
                || seed.path.len() > 2048
                || seed
                    .path
                    .bytes()
                    .any(|c| c < 0x20 || c == 0x7f || c == b';')
                || (seed.name.starts_with("__Secure-") || seed.name.starts_with("__Host-"))
                    && !seed.secure
                || !(seed.expires == -1.0 || (0.0..=253402300799.0).contains(&seed.expires))
                || seed
                    .same_site
                    .as_deref()
                    .is_some_and(|s| !matches!(s, "Strict" | "Lax" | "None"))
                || !seen.insert((seed.domain.clone(), seed.path.clone(), seed.name.clone()))
                || seed.name.starts_with("__Host-")
                    && (seed.domain.starts_with('.') || seed.path != "/")
            {
                return Err(invalid());
            }
            if seed.expires != -1.0
                && seed.expires <= time::OffsetDateTime::now_utc().unix_timestamp() as f64
            {
                continue;
            }
            let mut raw = format!("{}={}; Path={}", seed.name, seed.value, seed.path);
            if seed.secure {
                raw.push_str("; Secure");
            }
            if seed.domain.starts_with('.') {
                raw.push_str(&format!("; Domain={}", seed.domain));
            }
            if seed.http_only == Some(true) {
                raw.push_str("; HttpOnly");
            }
            let mut cookie = Cookie::parse(raw, &base)
                .map_err(|_| invalid())?
                .into_owned();
            if seed.expires != -1.0 {
                cookie.expires = cookie_store::CookieExpiration::AtUtc(
                    time::OffsetDateTime::from_unix_timestamp(seed.expires as i64)
                        .map_err(|_| invalid())?,
                );
            }
            store.insert(cookie, &base).map_err(|_| invalid())?;
        }
        let result = Self(store);
        let mut headers = HeaderMap::new();
        result.apply(&format!("{ORIGIN}/projects"), &mut headers)?;
        let raw = headers
            .get(COOKIE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("");
        if !raw.split(';').any(|p| {
            p.trim()
                .split_once('=')
                .is_some_and(|(k, v)| k == "__Secure-1PSID" && !v.is_empty())
        }) {
            return Err(invalid());
        }
        Ok(result)
    }

    pub(super) fn apply(&self, url: &str, headers: &mut HeaderMap) -> Result<(), GatewayError> {
        let url = target(url)?;
        let mut cookies = self.0.matches(&url);
        cookies.sort_by_key(|c| std::cmp::Reverse(c.path.len()));
        let raw = cookies
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; ");
        if raw.len() > 32768 {
            return Err(invalid());
        }
        headers.remove(COOKIE);
        if !raw.is_empty() {
            headers.insert(COOKIE, HeaderValue::from_str(&raw).map_err(|_| invalid())?);
        }
        Ok(())
    }

    pub(super) fn receive(&mut self, url: &str, headers: &HeaderMap) -> Result<(), GatewayError> {
        let url = target(url)?;
        for header in headers.get_all(SET_COOKIE).iter().take(64) {
            let raw = header.to_str().map_err(|_| invalid())?;
            if raw.len() > 8192 {
                return Err(invalid());
            }
            let cookie = Cookie::parse(raw.to_string(), &url).map_err(|_| invalid())?;
            if cookie
                .domain()
                .is_some_and(|d| !matches!(d, "google.com" | "flow.google.com"))
                || (cookie.name().starts_with("__Secure-") || cookie.name().starts_with("__Host-"))
                    && cookie.secure() != Some(true)
            {
                continue;
            }
            if cookie.name().starts_with("__Host-")
                && (cookie.domain().is_some() || cookie.path() != Some("/"))
            {
                continue;
            }
            let result = self.0.insert(cookie.into_owned(), &url);
            if result.is_err() && !matches!(result, Err(cookie_store::CookieError::Expired)) {
                return Err(invalid());
            }
            if self.0.iter_unexpired().count() > 64
                || self
                    .0
                    .iter_unexpired()
                    .map(|c| c.to_string().len())
                    .sum::<usize>()
                    > 32768
            {
                return Err(invalid());
            }
        }
        Ok(())
    }
}
