//! Cookies belong to one authenticated request chain, never to the shared HTTP client.
use cookie_store::{Cookie, CookieStore};
use rquest::header::{HeaderMap, HeaderValue, COOKIE, SET_COOKIE};
use std::{cmp::Reverse, fmt, sync::Mutex};
use url::Url;

use super::PROVIDER;
use crate::error::GatewayError;

const MAX_COOKIE_BYTES: usize = 8 * 1024;
const MAX_COOKIES: usize = 64;
const MAX_STORE_BYTES: usize = 32 * 1024;

#[derive(Default)]
struct State {
    store: CookieStore,
    initialized: bool,
}

pub(super) struct SessionCookies {
    origin: String,
    state: Mutex<State>,
}

impl fmt::Debug for SessionCookies {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionCookies([redacted])")
    }
}

fn invalid() -> GatewayError {
    GatewayError::bad_request("ChatGPT Web session Cookie material is invalid or exceeds limits.")
        .with_provider(PROVIDER)
        .with_code("chatgpt_web_session_cookie_invalid")
}

impl SessionCookies {
    pub(super) fn new(origin: &str) -> Self {
        let origin = Url::parse(origin)
            .map(|url| url.origin().ascii_serialization())
            .unwrap_or_default();
        Self {
            origin,
            state: Mutex::new(State::default()),
        }
    }

    pub(super) fn apply(&self, url: &str, headers: &mut HeaderMap) -> Result<(), GatewayError> {
        let url = Url::parse(url).map_err(|_| invalid())?;
        // Validate the actual target before sending Authorization as well as Cookie.
        if !self.accepts(&url) {
            return Err(GatewayError::bad_request(
                "ChatGPT Web session target must use the configured origin.",
            )
            .with_provider(PROVIDER)
            .with_code("chatgpt_web_session_origin_mismatch"));
        }
        let mut state = self.state.lock().map_err(|_| invalid())?;
        if !state.initialized {
            let mut store = CookieStore::default();
            let raw = headers
                .get(COOKIE)
                .map(|h| h.to_str())
                .transpose()
                .map_err(|_| invalid())?;
            if let Some(raw) = raw {
                if raw.len() > MAX_STORE_BYTES {
                    return Err(invalid());
                }
                let mut names = std::collections::HashSet::new();
                // An imported Cookie header lacks scope metadata: bind it host-only to /.
                for pair in raw.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                    if pair.len() > MAX_COOKIE_BYTES || !pair.contains('=') {
                        return Err(invalid());
                    }
                    let secure = if url.scheme() == "https" {
                        "; Secure"
                    } else {
                        ""
                    };
                    let cookie = Cookie::parse(format!("{pair}; Path=/{secure}"), &url)
                        .map_err(|_| invalid())?;
                    // A raw header cannot recover distinct scopes for duplicate names.
                    if !names.insert(cookie.name().to_string()) || !allowed_cookie(&cookie, &url) {
                        return Err(invalid());
                    }
                    store
                        .insert(cookie.into_owned(), &url)
                        .map_err(|_| invalid())?;
                    if !within_limits(&store) {
                        return Err(invalid());
                    }
                }
            }
            state.store = store;
            state.initialized = true;
        }
        // Deletion must not resurrect the original persisted/extra header on the next phase.
        headers.remove(COOKIE);
        let mut matched = state.store.matches(&url);
        matched.sort_by_key(|cookie| Reverse(cookie.path.len()));
        let value = matched
            .iter()
            .map(|c| format!("{}={}", c.name(), c.value()))
            .collect::<Vec<_>>()
            .join("; ");
        if value.len() > MAX_STORE_BYTES {
            return Err(invalid());
        }
        if !value.is_empty() {
            headers.insert(
                COOKIE,
                HeaderValue::from_str(&value).map_err(|_| invalid())?,
            );
        }
        Ok(())
    }

    pub(super) fn receive(&self, response: &rquest::Response) {
        // Intentionally narrower than a browser: rejected/challenge responses never update state.
        if response.status().is_success() {
            if let Ok(url) = Url::parse(&response.uri().to_string()) {
                self.receive_at(&url, response.headers());
            }
        }
    }

    fn accepts(&self, url: &Url) -> bool {
        matches!(url.scheme(), "http" | "https")
            && url.username().is_empty()
            && url.password().is_none()
            && url.origin().ascii_serialization() == self.origin
    }

    fn receive_at(&self, url: &Url, headers: &HeaderMap) {
        if !self.accepts(url) {
            return;
        }
        let Ok(mut state) = self.state.lock() else {
            return;
        };
        for value in headers.get_all(SET_COOKIE).iter().take(MAX_COOKIES) {
            let Ok(value) = value.to_str() else {
                continue;
            };
            if value.len() > MAX_COOKIE_BYTES {
                continue;
            }
            let Ok(cookie) = Cookie::parse(value.to_string(), url) else {
                continue;
            };
            if !allowed_cookie(&cookie, url) {
                continue;
            }
            let mut next = CookieStore::from_cookies(
                state
                    .store
                    .iter_unexpired()
                    .cloned()
                    .map(Ok::<_, std::convert::Infallible>),
                false,
            )
            .expect("infallible cookie clone");
            let _ = next.insert(cookie.into_owned(), url);
            if within_limits(&next) {
                state.store = next;
            }
        }
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
        !url.host_str()
            .is_some_and(|host| host.eq_ignore_ascii_case(domain.trim_start_matches('.')))
    }) {
        return false;
    }
    if cookie.name().starts_with("__Secure-") && (!secure || url.scheme() != "https") {
        return false;
    }
    if cookie.name().starts_with("__Host-")
        && (!secure
            || url.scheme() != "https"
            || cookie.domain().is_some()
            || cookie.path() != Some("/"))
    {
        return false;
    }
    true
}

#[cfg(test)]
mod tests;
