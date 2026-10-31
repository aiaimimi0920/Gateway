use super::*;

const BASE: &str = "https://gemini.google.com";

fn headers(value: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, value.parse().unwrap());
    headers
}

fn receive(jar: &mut SessionCookies, values: &[&str]) {
    let mut response = HeaderMap::new();
    for value in values {
        response.append(SET_COOKIE, value.parse().unwrap());
    }
    jar.receive(&format!("{BASE}/app"), &response);
}

fn sent(jar: &SessionCookies, path: &str) -> String {
    let mut headers = headers("pref=must-not-resurrect");
    jar.apply(&format!("{BASE}{path}"), &mut headers).unwrap();
    headers
        .get(COOKIE)
        .map(|v| v.to_str().unwrap())
        .unwrap_or("")
        .into()
}

#[test]
fn imported_header_order_and_parent_domain_rotation_are_preserved_without_duplicates() {
    let mut jar = SessionCookies::new(BASE, &headers("__Secure-1PSID=seed; pref=one")).unwrap();
    assert_eq!(sent(&jar, "/app"), "__Secure-1PSID=seed; pref=one");
    receive(
        &mut jar,
        &["__Secure-1PSID=rotated; Domain=.google.com; Path=/; Secure"],
    );
    assert_eq!(sent(&jar, "/app"), "__Secure-1PSID=rotated; pref=one");
    receive(
        &mut jar,
        &["__Secure-1PSID=; Domain=.google.com; Path=/; Secure; Max-Age=0"],
    );
    assert_eq!(sent(&jar, "/app"), "pref=one");
}

#[test]
fn scoped_cookie_does_not_delete_or_escape_its_path() {
    let mut jar = SessionCookies::new(BASE, &headers("pref=root")).unwrap();
    receive(&mut jar, &["pref=scoped; Path=/rpc"]);
    assert_eq!(sent(&jar, "/rpc/call"), "pref=scoped; pref=root");
    assert_eq!(sent(&jar, "/rpc-other"), "pref=root");
    receive(&mut jar, &["pref=; Path=/rpc; Max-Age=0"]);
    assert_eq!(sent(&jar, "/rpc/call"), "pref=root");
    receive(&mut jar, &["pref=; Path=/; Max-Age=0"]);
    assert_eq!(sent(&jar, "/app"), "");
}

#[test]
fn foreign_targets_and_invalid_cookie_scopes_or_prefixes_do_not_change_session() {
    let mut jar = SessionCookies::new(BASE, &headers("pref=seed")).unwrap();
    receive(
        &mut jar,
        &[
            "pref=evil; Domain=com; Path=/",
            "pref=evil; Domain=evil.invalid; Path=/",
            "__Secure-bad=value; Path=/",
            "__Host-bad=value; Domain=google.com; Path=/; Secure",
        ],
    );
    let mut foreign = HeaderMap::new();
    foreign.insert(SET_COOKIE, "pref=foreign; Path=/".parse().unwrap());
    jar.receive("https://accounts.google.com/app", &foreign);
    assert_eq!(sent(&jar, "/app"), "pref=seed");
    for url in [
        "https://accounts.google.com/app",
        "http://gemini.google.com/app",
        "https://gemini.google.com:444/app",
        "https://user@gemini.google.com/app",
    ] {
        assert!(jar.apply(url, &mut HeaderMap::new()).is_err());
    }
}

#[test]
fn separate_invocations_never_share_rotated_cookies_or_rewrite_seed_headers() {
    let seed = headers("pref=seed");
    let mut first = SessionCookies::new(BASE, &seed).unwrap();
    let second = SessionCookies::new(BASE, &seed).unwrap();
    receive(&mut first, &["pref=rotated; Path=/"]);
    assert_eq!(sent(&first, "/app"), "pref=rotated");
    assert_eq!(sent(&second, "/app"), "pref=seed");
    assert_eq!(seed[COOKIE], "pref=seed");
    assert!(!format!("{first:?}").contains("rotated"));
}

#[test]
fn cookie_material_is_bounded_and_errors_never_quote_secret_values() {
    for value in [
        "secret-without-equals".into(),
        "pref=a; pref=b".into(),
        format!("pref={}", "x".repeat(MAX_COOKIE_BYTES)),
        (0..=MAX_COOKIES)
            .map(|i| format!("c{i}=x"))
            .collect::<Vec<_>>()
            .join("; "),
    ] {
        let error = SessionCookies::new(BASE, &headers(&value)).unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("gemini_web_http_cookie_chain_invalid")
        );
        assert!(!error.message.contains("secret-without-equals"));
    }
    let mut jar = SessionCookies::new(BASE, &headers("pref=seed")).unwrap();
    receive(
        &mut jar,
        &[&format!("pref={}; Path=/", "x".repeat(MAX_COOKIE_BYTES))],
    );
    assert_eq!(sent(&jar, "/app"), "pref=seed");
}

#[test]
fn full_store_still_accepts_a_valid_deletion_without_evicting_other_cookies() {
    let seed = (0..MAX_COOKIES)
        .map(|i| format!("c{i}=x"))
        .collect::<Vec<_>>()
        .join("; ");
    let mut jar = SessionCookies::new(BASE, &headers(&seed)).unwrap();
    receive(
        &mut jar,
        &["extra=rejected; Path=/", "c0=; Path=/; Max-Age=0"],
    );
    let result = sent(&jar, "/app");
    assert!(!result.contains("extra=") && !result.split("; ").any(|s| s.starts_with("c0=")));
    assert_eq!(result.split("; ").count(), MAX_COOKIES - 1);
}
