//! Runtime-owned, bounded native sessions. A session cannot change credentials.
//! Callers must authenticate before using a scope; scope labels are not credentials.
use super::native_cookie_store::NativeCookieStore;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use url::Url;

const CAPACITY: usize = 128;
const IDLE_TTL: Duration = Duration::from_secs(600);
pub(crate) type Scope = [u8; 32];

pub(crate) fn scope(token: &str, label: &str) -> Scope {
    let mut hash = Sha256::new();
    for part in [token, label] {
        hash.update(part.len().to_le_bytes());
        hash.update(part.as_bytes());
    }
    hash.finalize().into()
}

struct Session {
    binding: Scope,
    touched: Instant,
    cookies: NativeCookieStore,
    initialized: std::sync::Arc<tokio::sync::OnceCell<()>>,
}

#[derive(Default)]
pub(crate) struct NativeCookieSessions(Mutex<HashMap<Scope, Session>>);

impl NativeCookieSessions {
    pub(crate) fn initialization(
        &self,
        scope: Scope,
        binding: Scope,
        url: &Url,
    ) -> Result<std::sync::Arc<tokio::sync::OnceCell<()>>, &'static str> {
        self.prepare(scope, binding, url)?;
        let sessions = self.0.lock().map_err(|_| "Native session unavailable")?;
        let session = sessions.get(&scope).ok_or("Native session expired")?;
        if session.binding != binding {
            return Err("Native session binding changed");
        }
        Ok(session.initialized.clone())
    }
    pub(crate) fn prepare(
        &self,
        scope: Scope,
        binding: Scope,
        url: &Url,
    ) -> Result<Option<String>, &'static str> {
        let mut sessions = self.0.lock().map_err(|_| "Native session unavailable")?;
        let now = Instant::now();
        // An initializer/waiter pins its entry. No eviction may replace its Cookie
        // jar while the old OnceCell is being completed; capacity stays bounded.
        sessions.retain(|_, session| {
            now.duration_since(session.touched) < IDLE_TTL
                || std::sync::Arc::strong_count(&session.initialized) > 1
        });
        if !sessions.contains_key(&scope) {
            if sessions.len() >= CAPACITY {
                return Err("Native session capacity reached");
            }
            sessions.insert(
                scope,
                Session {
                    binding,
                    touched: now,
                    cookies: NativeCookieStore::default(),
                    initialized: std::sync::Arc::new(tokio::sync::OnceCell::new()),
                },
            );
        }
        let session = sessions.get_mut(&scope).expect("inserted session");
        if session.binding != binding {
            return Err("Native session credentials or origin changed; start a new session");
        }
        session.touched = now;
        Ok(session.cookies.header(url))
    }

    pub(crate) fn receive(&self, scope: Scope, binding: Scope, url: &Url, values: &[&str]) {
        let Ok(mut sessions) = self.0.lock() else {
            return;
        };
        // Do not resurrect evicted sessions or import a late response after rebinding.
        if let Some(session) = sessions.get_mut(&scope) {
            if session.binding == binding && session.touched.elapsed() < IDLE_TTL {
                for value in values.iter().take(32) {
                    session.cookies.receive(url, value);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_session_initialization_is_shared_and_bound_to_credentials() {
        let sessions = NativeCookieSessions::default();
        let url = Url::parse("https://chatgpt.com/").unwrap();
        let first = sessions.initialization([1; 32], [2; 32], &url).unwrap();
        let second = sessions.initialization([1; 32], [2; 32], &url).unwrap();
        assert!(std::sync::Arc::ptr_eq(&first, &second));
        assert!(sessions.initialization([1; 32], [3; 32], &url).is_err());
        let other = sessions.initialization([4; 32], [2; 32], &url).unwrap();
        assert!(!std::sync::Arc::ptr_eq(&first, &other));
    }
    #[tokio::test]
    async fn native_session_initialization_single_flight_failure_and_cancellation() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let sessions = NativeCookieSessions::default();
        let url = Url::parse("https://chatgpt.com/").unwrap();
        let gate = sessions.initialization([1; 32], [2; 32], &url).unwrap();
        let failed: Result<&(), &str> = gate.get_or_try_init(|| async { Err("retry") }).await;
        assert!(failed.is_err());
        let cancelled = tokio::time::timeout(
            Duration::from_millis(1),
            gate.get_or_try_init(|| async {
                std::future::pending::<Result<(), &'static str>>().await
            }),
        )
        .await;
        assert!(cancelled.is_err());
        let calls = Arc::new(AtomicUsize::new(0));
        let mut tasks = Vec::new();
        for _ in 0..6 {
            let gate = sessions.initialization([1; 32], [2; 32], &url).unwrap();
            let calls = calls.clone();
            tasks.push(tokio::spawn(async move {
                gate.get_or_try_init(|| async {
                    calls.fetch_add(1, Ordering::SeqCst);
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    Ok::<(), &'static str>(())
                })
                .await
                .unwrap();
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        assert!(gate.get().is_some());
        assert!(sessions.prepare([1; 32], [2; 32], &url).unwrap().is_none());
    }

    #[test]
    fn native_session_initialization_pins_entry_until_waiters_drop() {
        let sessions = NativeCookieSessions::default();
        let url = Url::parse("https://chatgpt.com/").unwrap();
        let gate = sessions.initialization([1; 32], [2; 32], &url).unwrap();
        sessions
            .0
            .lock()
            .unwrap()
            .get_mut(&[1; 32])
            .unwrap()
            .touched = Instant::now() - IDLE_TTL;
        let same = sessions.initialization([1; 32], [2; 32], &url).unwrap();
        assert!(std::sync::Arc::ptr_eq(&gate, &same));
        drop(same);
        drop(gate);
        sessions
            .0
            .lock()
            .unwrap()
            .get_mut(&[1; 32])
            .unwrap()
            .touched = Instant::now() - IDLE_TTL;
        assert!(sessions.initialization([1; 32], [3; 32], &url).is_ok());
    }
    #[test]
    fn identities_are_separated_and_credentials_cannot_switch() {
        let sessions = NativeCookieSessions::default();
        let url = Url::parse("https://chatgpt.com/").unwrap();
        let a = scope("key-a", "session");
        let b = scope("key-b", "session");
        let c = scope("key-a", "other-session");
        assert_ne!(a, b);
        assert_ne!(a, c);
        sessions.prepare(a, [1; 32], &url).unwrap();
        sessions.receive(a, [1; 32], &url, &["a=1; Path=/"]);
        assert_eq!(
            sessions.prepare(a, [1; 32], &url).unwrap().as_deref(),
            Some("a=1")
        );
        assert!(sessions.prepare(b, [1; 32], &url).unwrap().is_none());
        assert!(sessions.prepare(c, [1; 32], &url).unwrap().is_none());
        assert!(sessions.prepare(a, [2; 32], &url).is_err());
    }
    #[test]
    fn capacity_and_idle_expiry_are_bounded() {
        let sessions = NativeCookieSessions::default();
        let url = Url::parse("https://chatgpt.com/").unwrap();
        for n in 0..CAPACITY {
            sessions
                .prepare(scope("key", &n.to_string()), [1; 32], &url)
                .unwrap();
        }
        assert!(sessions.prepare([255; 32], [1; 32], &url).is_err());
        for session in sessions.0.lock().unwrap().values_mut() {
            session.touched = Instant::now() - IDLE_TTL;
        }
        assert!(sessions.prepare([255; 32], [1; 32], &url).is_ok());
        assert_eq!(sessions.0.lock().unwrap().len(), 1);
    }
}
