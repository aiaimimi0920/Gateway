use super::*;

fn url(path: &str) -> Url {
    Url::parse(&format!("https://chatgpt.com{path}")).unwrap()
}

fn send(jar: &SessionCookies, path: &str, seed: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(COOKIE, HeaderValue::from_str(seed).unwrap());
    jar.apply(url(path).as_str(), &mut headers).unwrap();
    headers
}

fn receive(jar: &SessionCookies, path: &str, values: &[&str]) {
    let mut headers = HeaderMap::new();
    for value in values {
        headers.append(SET_COOKIE, HeaderValue::from_str(value).unwrap());
    }
    jar.receive_at(&url(path), &headers);
}

#[test]
fn updates_and_deletes_without_resurrecting_persisted_cookie() {
    let jar = SessionCookies::new("https://chatgpt.com");
    assert_eq!(send(&jar, "/", "sid=secret")[COOKIE], "sid=secret");
    receive(&jar, "/", &["sid=updated; Path=/; Secure"]);
    assert_eq!(
        send(&jar, "/backend-api/models", "sid=secret")[COOKIE],
        "sid=updated"
    );
    receive(
        &jar,
        "/",
        &["sid=; Path=/; Secure; Max-Age=0; Expires=Wed, 01 Jan 2099 00:00:00 GMT"],
    );
    assert!(send(&jar, "/backend-api/models", "sid=secret")
        .get(COOKIE)
        .is_none());
    assert!(!format!("{jar:?}").contains("secret"));
}

#[test]
fn same_name_cookies_keep_path_identity_and_longest_path_order() {
    let jar = SessionCookies::new("https://chatgpt.com");
    send(&jar, "/", "sid=root");
    receive(&jar, "/", &["sid=api; Path=/backend-api; Secure"]);
    assert_eq!(
        send(&jar, "/backend-api/models", "sid=root")[COOKIE],
        "sid=api; sid=root"
    );
    assert_eq!(send(&jar, "/backend-apix", "sid=root")[COOKIE], "sid=root");
    receive(
        &jar,
        "/",
        &["sid=gone; Path=/backend-api; Secure; Max-Age=0"],
    );
    assert_eq!(
        send(&jar, "/backend-api/models", "sid=root")[COOKIE],
        "sid=root"
    );
}

#[test]
fn rejects_foreign_origins_domains_and_invalid_cookie_prefixes() {
    let jar = SessionCookies::new("https://chatgpt.com");
    send(&jar, "/", "sid=secret");
    receive(
        &jar,
        "/",
        &[
            "bad=1; Domain=com",
            "bad=2; Domain=other.example",
            "__Secure-bad=1",
            "__Host-bad=1; Secure; Domain=chatgpt.com; Path=/",
            "__Host-path=1; Secure; Path=/private",
            "expired=1; Max-Age=-1",
        ],
    );
    assert_eq!(send(&jar, "/", "sid=secret")[COOKIE], "sid=secret");
    for target in [
        "https://other.example/",
        "https://chatgpt.com:444/",
        "http://chatgpt.com/",
        "https://u@chatgpt.com/",
    ] {
        let error = jar.apply(target, &mut HeaderMap::new()).unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("chatgpt_web_session_origin_mismatch")
        );
        assert!(!error.message.contains("secret"));
    }
    let mut foreign = HeaderMap::new();
    foreign.insert(SET_COOKIE, HeaderValue::from_static("sid=foreign; Path=/"));
    jar.receive_at(&Url::parse("https://other.example/").unwrap(), &foreign);
    assert_eq!(send(&jar, "/", "sid=secret")[COOKIE], "sid=secret");
    let http = SessionCookies::new("http://fixture.example");
    let mut headers = HeaderMap::new();
    http.apply("http://fixture.example/", &mut headers).unwrap();
    http.receive_at(&Url::parse("http://fixture.example/").unwrap(), &{
        let mut h = HeaderMap::new();
        h.insert(SET_COOKIE, HeaderValue::from_static("secure=1; Secure"));
        h
    });
    http.apply("http://fixture.example/", &mut headers).unwrap();
    assert!(headers.get(COOKIE).is_none());
}

#[test]
fn cookies_are_request_local_and_bounded_without_truncating_saved_material() {
    let a = SessionCookies::new("https://chatgpt.com");
    let b = SessionCookies::new("https://chatgpt.com");
    send(&a, "/", "sid=A");
    send(&b, "/", "sid=B");
    receive(&a, "/", &["sid=A-updated; Path=/"]);
    assert_eq!(send(&b, "/", "sid=B")[COOKIE], "sid=B");
    assert_eq!(
        send(&SessionCookies::new("https://chatgpt.com"), "/", "sid=A")[COOKIE],
        "sid=A"
    );
    receive(
        &a,
        "/",
        &[&format!("sid={}; Path=/", "x".repeat(MAX_COOKIE_BYTES))],
    );
    assert_eq!(send(&a, "/", "sid=A")[COOKIE], "sid=A-updated");
    for index in 0..MAX_COOKIES + 20 {
        receive(&a, "/", &[&format!("c{index}=v; Path=/")]);
    }
    assert_eq!(
        a.state.lock().unwrap().store.iter_unexpired().count(),
        MAX_COOKIES
    );
    receive(&a, "/", &["sid=; Path=/; Max-Age=0", "released=1; Path=/"]);
    assert!(send(&a, "/", "sid=A")[COOKIE]
        .to_str()
        .unwrap()
        .contains("released=1"));
    assert!(!send(&a, "/", "sid=A")[COOKIE]
        .to_str()
        .unwrap()
        .contains("sid="));
    let large = SessionCookies::new("https://chatgpt.com");
    send(&large, "/", "keep=1");
    for index in 0..12 {
        receive(
            &large,
            "/",
            &[&format!("c{index}={}; Path=/", "x".repeat(7000))],
        );
    }
    assert!(within_limits(&large.state.lock().unwrap().store));
    assert!(send(&large, "/", "keep=1")[COOKIE].as_bytes().len() <= MAX_STORE_BYTES);
    for seed in [
        "private-imported-cookie-without-equals".to_string(),
        "sid=api; sid=root".to_string(),
        format!("sid={}", "x".repeat(MAX_STORE_BYTES)),
    ] {
        let jar = SessionCookies::new("https://chatgpt.com");
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_str(&seed).unwrap());
        let error = jar.apply(url("/").as_str(), &mut headers).unwrap_err();
        assert_eq!(
            error.code.as_deref(),
            Some("chatgpt_web_session_cookie_invalid")
        );
        assert!(!error.message.contains(&seed));
        assert!(!jar.state.lock().unwrap().initialized);
    }
}

#[test]
fn imported_cookie_prefixes_require_https_and_response_header_count_is_bounded() {
    for seed in ["__Secure-sid=secret", "__Host-sid=secret"] {
        let http = SessionCookies::new("http://fixture.example");
        let mut headers = HeaderMap::new();
        headers.insert(COOKIE, HeaderValue::from_static(seed));
        assert!(http.apply("http://fixture.example/", &mut headers).is_err());
        let https = SessionCookies::new("https://chatgpt.com");
        assert_eq!(send(&https, "/", seed)[COOKIE], seed);
    }
    let jar = SessionCookies::new("https://chatgpt.com");
    send(&jar, "/", "sid=original");
    let mut headers = HeaderMap::new();
    for _ in 0..MAX_COOKIES {
        headers.append(
            SET_COOKIE,
            HeaderValue::from_static("ignored=1; Domain=com"),
        );
    }
    headers.append(
        SET_COOKIE,
        HeaderValue::from_static("sid=; Path=/; Max-Age=0"),
    );
    jar.receive_at(&url("/"), &headers);
    assert_eq!(send(&jar, "/", "sid=original")[COOKIE], "sid=original");
}
