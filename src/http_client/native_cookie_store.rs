//! Bounded, origin-local Cookie state for an explicitly bound native session.
//! No persistence, logging, caller Cookie import, or process-global cookie jar.
use cookie_store::{Cookie, CookieStore};
use url::Url;

const MAX_COOKIE_BYTES: usize = 4096;
const MAX_COOKIES: usize = 32;
const MAX_STORE_BYTES: usize = 16 * 1024;
const MAX_HEADER_BYTES: usize = 8 * 1024;

#[derive(Default)]
pub(crate) struct NativeCookieStore {
    store: CookieStore,
    origin: Option<String>,
}

impl NativeCookieStore {
    pub(crate) fn receive(&mut self, url: &Url, value: &str) {
        if value.len() > MAX_COOKIE_BYTES || !matches!(url.scheme(), "http" | "https") {
            return;
        }
        let origin = url.origin().ascii_serialization();
        if self.origin.as_ref().is_some_and(|bound| bound != &origin) {
            return;
        }
        let Ok(cookie) = Cookie::parse(value.to_owned(), url) else {
            return;
        };
        // Native compatibility is single-origin, not browser navigation. Reject
        // parent-domain cookies rather than depending on a stale suffix list.
        if cookie.domain().is_some_and(|domain| {
            !url.host_str()
                .is_some_and(|host| host.eq_ignore_ascii_case(domain.trim_start_matches('.')))
        }) {
            return;
        }
        let mut next = CookieStore::from_cookies(
            self.store
                .iter_unexpired()
                .cloned()
                .map(Ok::<_, std::convert::Infallible>),
            false,
        )
        .expect("infallible cookie clone");
        // Deletion may report ExpiredExisting after successfully removing state.
        let _ = next.insert(cookie.into_owned(), url);
        let count = next.iter_unexpired().count();
        let bytes: usize = next.iter_unexpired().map(|c| c.to_string().len()).sum();
        if count <= MAX_COOKIES && bytes <= MAX_STORE_BYTES {
            self.store = next;
            self.origin = Some(origin);
        }
    }

    pub(crate) fn header(&self, url: &Url) -> Option<String> {
        if self.origin.as_deref() != Some(url.origin().ascii_serialization().as_str()) {
            return None;
        }
        let value = self
            .store
            .get_request_values(url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        (!value.is_empty() && value.len() <= MAX_HEADER_BYTES).then_some(value)
    }
}

#[cfg(test)]
#[path = "native_cookie_store_tests.rs"]
mod tests;
