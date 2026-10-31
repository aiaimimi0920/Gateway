//! Preserve effective scope and absolute expiry; never extend Max-Age on restart.
use super::{
    allowed_cookie, invalid, within_limits, SessionCookies, MAX_COOKIES, MAX_COOKIE_BYTES,
};
use crate::error::GatewayError;
use cookie_store::{Cookie, CookieExpiration, CookiePath};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::upstream::gemini::web_reverse) struct CookieSnapshot {
    cookies: Vec<SavedCookie>,
    seed_names: Vec<String>,
    order: Vec<String>,
}

#[cfg(test)]
mod tests;
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedCookie {
    raw: String,
    path: String,
    expires: Option<i64>,
}

impl SessionCookies {
    pub(in crate::upstream::gemini::web_reverse) fn snapshot(&self) -> CookieSnapshot {
        let cookies = self
            .store
            .iter_unexpired()
            .map(|cookie| SavedCookie {
                raw: cookie.to_string(),
                path: cookie.path.to_string(),
                expires: match &cookie.expires {
                    CookieExpiration::AtUtc(time) => Some(time.unix_timestamp()),
                    CookieExpiration::SessionEnd => None,
                },
            })
            .collect();
        let present = |name: &String| {
            self.store
                .iter_unexpired()
                .any(|cookie| cookie.name() == name)
        };
        CookieSnapshot {
            cookies,
            seed_names: self
                .seed_names
                .iter()
                .filter(|name| present(name))
                .cloned()
                .collect(),
            order: self
                .order
                .iter()
                .filter(|name| present(name))
                .cloned()
                .collect(),
        }
    }

    pub(in crate::upstream::gemini::web_reverse) fn restore(
        base: &str,
        state: CookieSnapshot,
    ) -> Result<Self, GatewayError> {
        if state.cookies.len() > MAX_COOKIES
            || state.seed_names.len() > MAX_COOKIES
            || state.order.len() > MAX_COOKIES
        {
            return Err(invalid());
        }
        let mut restored = Self::new(base, &rquest::header::HeaderMap::new())?;
        let url = url::Url::parse(base).map_err(|_| invalid())?;
        for saved in state.cookies {
            if saved.raw.len() > MAX_COOKIE_BYTES || saved.path.len() > MAX_COOKIE_BYTES {
                return Err(invalid());
            }
            let mut cookie = Cookie::parse(saved.raw, &url)
                .map_err(|_| invalid())?
                .into_owned();
            cookie.path = CookiePath::parse(&saved.path).ok_or_else(invalid)?;
            cookie.expires = match saved.expires {
                None => CookieExpiration::SessionEnd,
                Some(seconds) => CookieExpiration::AtUtc(
                    time::OffsetDateTime::from_unix_timestamp(seconds).map_err(|_| invalid())?,
                ),
            };
            if !allowed_cookie(&cookie, &url) {
                return Err(invalid());
            }
            if cookie.is_expired() {
                continue;
            }
            restored.store.insert(cookie, &url).map_err(|_| invalid())?;
        }
        if !within_limits(&restored.store) {
            return Err(invalid());
        }
        restored.seed_names = state.seed_names.into_iter().collect();
        restored.order = state.order;
        Ok(restored)
    }
}
