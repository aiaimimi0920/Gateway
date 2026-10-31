//! Request-local Cookie mechanics; HttpSession owns persistence, never the shared client.
use cookie_store::{Cookie, CookieStore};
use rquest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use std::{cmp::Reverse, collections::HashSet, fmt};
use url::Url;

mod snapshot;
pub(super) use snapshot::CookieSnapshot;

use crate::error::GatewayError;

const MAX_COOKIE_BYTES: usize = 8 * 1024;
const MAX_COOKIES: usize = 64;
const MAX_STORE_BYTES: usize = 32 * 1024;

pub(in crate::upstream) struct SessionCookies {
    origin: String,
    store: CookieStore,
    seed_names: HashSet<String>,
    order: Vec<String>,
}

impl fmt::Debug for SessionCookies {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionCookies([redacted])")
    }
}

fn invalid() -> GatewayError {
    GatewayError::bad_request("Gemini Web HTTP Cookie chain is invalid or exceeds limits.")
        .with_provider(super::PROVIDER)
        .with_code("gemini_web_http_cookie_chain_invalid")
}

impl SessionCookies {
    pub(in crate::upstream) fn new(base: &str, headers: &HeaderMap) -> Result<Self, GatewayError> {
        let url = Url::parse(base).map_err(|_| invalid())?;
        if !matches!(url.scheme(), "http" | "https")
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return Err(invalid());
        }
        let mut result = Self {
            origin: url.origin().ascii_serialization(),
            store: CookieStore::default(),
            seed_names: HashSet::new(),
            order: vec![],
        };
        let raw = headers
            .get(COOKIE)
            .map(|h| h.to_str())
            .transpose()
            .map_err(|_| invalid())?
            .unwrap_or("");
        if raw.len() > MAX_STORE_BYTES {
            return Err(invalid());
        }
        for pair in raw.split(';').map(str::trim).filter(|s| !s.is_empty()) {
            if pair.len() > MAX_COOKIE_BYTES || !pair.contains('=') {
                return Err(invalid());
            }
            // Imported headers lack scope metadata: bind to this host and root, not all Google sites.
            let secure = if url.scheme() == "https" {
                "; Secure"
            } else {
                ""
            };
            let cookie =
                Cookie::parse(format!("{pair}; Path=/{secure}"), &url).map_err(|_| invalid())?;
            if !result.seed_names.insert(cookie.name().to_string()) {
                return Err(invalid());
            }
            result.order.push(cookie.name().to_string());
            result
                .store
                .insert(cookie.into_owned(), &url)
                .map_err(|_| invalid())?;
            if !within_limits(&result.store) {
                return Err(invalid());
            }
        }
        Ok(result)
    }

    pub(in crate::upstream) fn apply(
        &self,
        target: &str,
        headers: &mut HeaderMap,
    ) -> Result<(), GatewayError> {
        let url = Url::parse(target).map_err(|_| invalid())?;
        if !self.accepts(&url) {
            return Err(invalid());
        }
        let mut matches = self.store.matches(&url);
        matches.sort_by_key(|c| {
            (
                Reverse(c.path.len()),
                self.order.iter().position(|name| name == c.name()),
            )
        });
        let raw = matches
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; ");
        if raw.len() > MAX_STORE_BYTES {
            return Err(invalid());
        }
        // Always remove the original header, including when a server has deleted the last Cookie.
        headers.remove(COOKIE);
        if !raw.is_empty() {
            headers.insert(COOKIE, HeaderValue::from_str(&raw).map_err(|_| invalid())?);
        }
        Ok(())
    }

    pub(in crate::upstream) fn receive(&mut self, source: &str, headers: &HeaderMap) {
        let Ok(url) = Url::parse(source) else {
            return;
        };
        if !self.accepts(&url) {
            return;
        }
        // Callers invoke this only after a successful, classified and bounded bootstrap body.
        for raw in headers.get_all(SET_COOKIE).iter().take(MAX_COOKIES) {
            let Ok(raw) = raw.to_str() else {
                continue;
            };
            if raw.len() > MAX_COOKIE_BYTES {
                continue;
            }
            let Ok(cookie) = Cookie::parse(raw.to_string(), &url) else {
                continue;
            };
            if !allowed_cookie(&cookie, &url) {
                continue;
            }
            let mut next = CookieStore::from_cookies(
                self.store
                    .iter_unexpired()
                    .cloned()
                    .map(Ok::<_, std::convert::Infallible>),
                false,
            )
            .expect("infallible Cookie clone");
            // A raw seed cannot tell host-only from Domain=google.com. A root rotation replaces
            // its synthetic seed scope instead of emitting both stale and rotated copies.
            let replaces_seed =
                cookie.path.as_ref() == "/" && self.seed_names.contains(cookie.name());
            if replaces_seed {
                next.remove(url.host_str().unwrap_or(""), "/", cookie.name());
            }
            let name = cookie.name().to_string();
            let inserted = next.insert(cookie.into_owned(), &url);
            if (inserted.is_err() && !matches!(inserted, Err(cookie_store::CookieError::Expired)))
                || !within_limits(&next)
            {
                continue;
            }
            self.store = next;
            if replaces_seed {
                self.seed_names.remove(&name);
            }
            if !self.order.contains(&name) {
                self.order.push(name);
            }
        }
    }

    fn accepts(&self, url: &Url) -> bool {
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && url.origin().ascii_serialization() == self.origin
    }
}

fn within_limits(store: &CookieStore) -> bool {
    store.iter_unexpired().count() <= MAX_COOKIES
        && store
            .iter_unexpired()
            .map(|c| c.to_string().len())
            .sum::<usize>()
            <= MAX_STORE_BYTES
}

fn allowed_cookie(cookie: &Cookie<'_>, url: &Url) -> bool {
    let secure = cookie.secure().unwrap_or(false);
    if secure && url.scheme() != "https" {
        return false;
    }
    if cookie.domain().is_some_and(|domain| {
        let domain = domain.trim_start_matches('.');
        !url.host_str().is_some_and(|host| {
            host.eq_ignore_ascii_case(domain)
                || (host.eq_ignore_ascii_case("gemini.google.com")
                    && domain.eq_ignore_ascii_case("google.com"))
        })
    }) {
        return false;
    }
    if cookie.name().starts_with("__Secure-") && (!secure || url.scheme() != "https") {
        return false;
    }
    !(cookie.name().starts_with("__Host-")
        && (!secure
            || url.scheme() != "https"
            || cookie.domain().is_some()
            || cookie.path() != Some("/")))
}

#[cfg(test)]
mod tests;
