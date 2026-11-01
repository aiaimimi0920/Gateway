//! Request-local Cookie jar preserving source scopes. Never send Google.com cookies to withgoogle.com.
use super::{failure, ORIGIN};
use crate::error::GatewayError;
use cookie_store::{Cookie, CookieStore};
use rquest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use serde::Deserialize;
use std::collections::HashSet;
use url::Url;

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

pub(super) struct StitchCookies(CookieStore);
fn invalid() -> GatewayError {
    failure(
        "stitch_web_session_invalid",
        "Stitch Cookie 材料无效、过期或作用域不匹配。",
    )
}
fn target(address: &str) -> Result<Url, GatewayError> {
    let url = Url::parse(address).map_err(|_| invalid())?;
    if url.origin().ascii_serialization() != ORIGIN
        || !url.username().is_empty()
        || url.password().is_some()
    {
        return Err(invalid());
    }
    Ok(url)
}
fn domain(value: &str) -> bool {
    matches!(
        value,
        ".withgoogle.com" | "stitch.withgoogle.com" | ".stitch.withgoogle.com"
    )
}

impl StitchCookies {
    pub(super) fn from_bundle(bytes: &[u8]) -> Result<Self, GatewayError> {
        if bytes.len() > 32768 {
            return Err(invalid());
        }
        let bundle: Bundle = serde_json::from_slice(bytes).map_err(|_| invalid())?;
        if bundle.schema_version != 1
            || bundle.provider != "stitch-web"
            || bundle.auth_user != "0"
            || bundle.cookies.is_empty()
            || bundle.cookies.len() > 64
        {
            return Err(invalid());
        }
        let base = target(ORIGIN)?;
        let mut store = CookieStore::default();
        let mut seen = HashSet::new();
        for c in bundle.cookies {
            if !domain(&c.domain)
                || c.name.is_empty()
                || c.name.len() > 256
                || c.value.len() > 8192
                || !c
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
                || !c
                    .value
                    .bytes()
                    .all(|b| (0x21..=0x7e).contains(&b) && !b";,\"\\".contains(&b))
                || !c.path.starts_with('/')
                || c.path.len() > 2048
                || c.path.bytes().any(|b| b < 0x20 || b == 0x7f || b == b';')
                || !(c.expires == -1.0 || (0.0..=253402300799.0).contains(&c.expires))
                || (c.name.starts_with("__Secure-") || c.name.starts_with("__Host-")) && !c.secure
                || c.name.starts_with("__Host-") && (c.domain.starts_with('.') || c.path != "/")
                || c.same_site
                    .as_deref()
                    .is_some_and(|s| !matches!(s, "Strict" | "Lax" | "None"))
                || !seen.insert((c.domain.clone(), c.path.clone(), c.name.clone()))
            {
                return Err(invalid());
            }
            if c.expires != -1.0
                && c.expires <= time::OffsetDateTime::now_utc().unix_timestamp() as f64
            {
                continue;
            }
            let mut raw = format!("{}={}; Path={}", c.name, c.value, c.path);
            if c.secure {
                raw.push_str("; Secure");
            }
            if c.http_only == Some(true) {
                raw.push_str("; HttpOnly");
            }
            if c.domain.starts_with('.') {
                raw.push_str(&format!("; Domain={}", c.domain));
            }
            if let Some(site) = c.same_site {
                raw.push_str(&format!("; SameSite={site}"));
            }
            let mut cookie = Cookie::parse(raw, &base)
                .map_err(|_| invalid())?
                .into_owned();
            if c.expires != -1.0 {
                cookie.expires = cookie_store::CookieExpiration::AtUtc(
                    time::OffsetDateTime::from_unix_timestamp(c.expires as i64)
                        .map_err(|_| invalid())?,
                );
            }
            store.insert(cookie, &base).map_err(|_| invalid())?;
        }
        if !store.matches(&base).iter().any(|c| !c.value().is_empty()) {
            return Err(invalid());
        }
        Ok(Self(store))
    }

    pub(super) fn apply(&self, address: &str, headers: &mut HeaderMap) -> Result<(), GatewayError> {
        let url = target(address)?;
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

    pub(super) fn receive(
        &mut self,
        address: &str,
        headers: &HeaderMap,
    ) -> Result<(), GatewayError> {
        let url = target(address)?;
        for value in headers.get_all(SET_COOKIE).iter().take(64) {
            let raw = value.to_str().map_err(|_| invalid())?;
            if raw.len() > 8192 {
                return Err(invalid());
            }
            let cookie = Cookie::parse(raw.to_string(), &url).map_err(|_| invalid())?;
            if cookie
                .domain()
                .is_some_and(|s| !matches!(s, "withgoogle.com" | "stitch.withgoogle.com"))
                || (cookie.name().starts_with("__Secure-") || cookie.name().starts_with("__Host-"))
                    && cookie.secure() != Some(true)
                || cookie.name().starts_with("__Host-")
                    && (cookie.domain().is_some() || cookie.path() != Some("/"))
            {
                continue;
            }
            let result = self.0.insert(cookie.into_owned(), &url);
            if result.is_err() && !matches!(result, Err(cookie_store::CookieError::Expired)) {
                return Err(invalid());
            }
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
        Ok(())
    }

    pub(super) fn bundle_bytes(&self) -> Result<Vec<u8>, GatewayError> {
        let cookies:Vec<_>=self.0.iter_unexpired().map(|c|serde_json::json!({
            "name":c.name(),"value":c.value(),"path":c.path.to_string(),
            "domain":c.domain().map(|s|format!(".{}",s.trim_start_matches('.'))).unwrap_or_else(||"stitch.withgoogle.com".into()),
            "secure":c.secure().unwrap_or(false),"httpOnly":c.http_only().unwrap_or(false),"sameSite":c.same_site().map(|s|s.to_string()),
            "expires":match &c.expires {cookie_store::CookieExpiration::AtUtc(t)=>t.unix_timestamp(),cookie_store::CookieExpiration::SessionEnd=>-1}
        })).collect();
        let bytes=serde_json::to_vec(&serde_json::json!({"schemaVersion":1,"provider":"stitch-web","authUser":"0","cookies":cookies})).map_err(|_|invalid())?;
        Self::from_bundle(&bytes)?;
        Ok(bytes)
    }
}
