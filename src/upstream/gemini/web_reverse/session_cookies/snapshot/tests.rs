//! A restart must not widen scope, revive deletion or reset relative expiration.
use super::*;
use rquest::header::{HeaderMap, COOKIE, SET_COOKIE};
const BASE: &str = "https://gemini.google.com/app";
fn seed() -> SessionCookies {
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, "pref=synthetic-seed".parse().unwrap());
    SessionCookies::new(BASE, &headers).unwrap()
}
fn receive(jar: &mut SessionCookies, url: &str, raw: &str) {
    let mut headers = HeaderMap::new();
    headers.insert(SET_COOKIE, raw.parse().unwrap());
    jar.receive(url, &headers);
}
fn sent(jar: &SessionCookies, url: &str) -> String {
    let mut headers = HeaderMap::new();
    jar.apply(url, &mut headers).unwrap();
    headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .into()
}

#[test]
fn http_snapshot_absolute_expiry_does_not_restart_max_age() {
    let mut jar = seed();
    receive(&mut jar, BASE, "pref=rotated; Max-Age=3600; Path=/; Secure");
    let mut snapshot = jar.snapshot();
    snapshot.cookies[0].expires = Some(time::OffsetDateTime::now_utc().unix_timestamp() - 1);
    let restored = SessionCookies::restore(BASE, snapshot).unwrap();
    assert!(sent(&restored, BASE).is_empty());
}

#[test]
fn http_snapshot_retains_effective_default_path() {
    let mut jar = seed();
    receive(
        &mut jar,
        "https://gemini.google.com/a/b",
        "scoped=synthetic; Secure",
    );
    let restored = SessionCookies::restore(BASE, jar.snapshot()).unwrap();
    assert!(!sent(&restored, BASE).contains("scoped="));
    assert!(sent(&restored, "https://gemini.google.com/a/c").contains("scoped=synthetic"));
}

#[test]
fn http_snapshot_domain_rotation_stays_single_and_deletion_stays_empty() {
    let mut jar = seed();
    receive(
        &mut jar,
        BASE,
        "pref=rotated; Domain=.google.com; Path=/; Secure",
    );
    let mut restored = SessionCookies::restore(BASE, jar.snapshot()).unwrap();
    receive(
        &mut restored,
        BASE,
        "pref=next; Domain=.google.com; Path=/; Secure",
    );
    assert_eq!(sent(&restored, BASE), "pref=next");
    receive(
        &mut restored,
        BASE,
        "pref=; Domain=.google.com; Path=/; Max-Age=0; Secure",
    );
    assert!(sent(
        &SessionCookies::restore(BASE, restored.snapshot()).unwrap(),
        BASE
    )
    .is_empty());
}

#[test]
fn http_snapshot_revalidates_secure_prefix_and_bounds() {
    let mut snapshot = seed().snapshot();
    snapshot.cookies[0].raw = "__Host-pref=bad; Domain=google.com; Path=/; Secure".into();
    assert!(SessionCookies::restore(BASE, snapshot).is_err());
    let mut snapshot = seed().snapshot();
    snapshot.cookies[0].path = "x".repeat(MAX_COOKIE_BYTES + 1);
    assert!(SessionCookies::restore(BASE, snapshot).is_err());
}
