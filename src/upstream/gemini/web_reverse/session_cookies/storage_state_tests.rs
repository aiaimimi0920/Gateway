use super::*;
use serde_json::json;

fn item(name: &str, domain: &str, path: &str, expires: i64) -> Value {
    json!({"name":name,"value":"synthetic","domain":domain,"path":path,"secure":true,"expires":expires})
}

fn header(cookies: &SessionCookies, path: &str) -> String {
    let mut headers = HeaderMap::new();
    cookies
        .apply(&format!("https://gemini.google.com{path}"), &mut headers)
        .unwrap();
    headers
        .get(COOKIE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .to_string()
}

#[test]
fn imported_scope_and_absolute_expiry_are_preserved() {
    let state = json!({"cookies":[item("root",".google.com","/",-1),
        item("expired",".google.com","/",1), item("appOnly","gemini.google.com","/app",-1),
        item("accountsOnly","accounts.google.com","/",-1), item("sibling",".evilgoogle.com","/",-1),
        item("childHost","notgemini.google.com","/",-1)]});
    let cookies = SessionCookies::from_storage_state("https://gemini.google.com", &state).unwrap();
    assert_eq!(
        header(&cookies, "/_/BardChatUi/data/batchexecute"),
        "root=synthetic"
    );
    let app = header(&cookies, "/app");
    assert!(app.starts_with("appOnly=synthetic"));
    assert!(app.contains("root=synthetic"));
    assert!(!header(&cookies, "/application").contains("appOnly"));
    assert!(cookies
        .apply("https://accounts.google.com/", &mut HeaderMap::new())
        .is_err());
    assert_eq!(format!("{cookies:?}"), "SessionCookies([redacted])");
}

#[test]
fn imported_seed_stays_scoped_during_rotation_and_deletion() {
    let state = json!({"cookies":[item("SAPISID",".google.com","/",-1)]});
    let mut cookies =
        SessionCookies::from_storage_state("https://gemini.google.com", &state).unwrap();
    let mut response = HeaderMap::new();
    response.insert(
        SET_COOKIE,
        HeaderValue::from_static("SAPISID=rotated; Domain=.google.com; Path=/; Secure"),
    );
    cookies.receive("https://gemini.google.com/app", &response);
    assert_eq!(header(&cookies, "/"), "SAPISID=rotated");
    response.insert(
        SET_COOKIE,
        HeaderValue::from_static("SAPISID=; Domain=.google.com; Path=/; Max-Age=0; Secure"),
    );
    cookies.receive("https://gemini.google.com/app", &response);
    assert!(header(&cookies, "/").is_empty());
}

#[test]
fn malformed_and_excessive_imports_fail_closed() {
    for (field, value) in [
        ("name", json!("SID; injected")),
        ("value", json!("value; injected=secret")),
        ("path", json!("/; Domain=evil.test")),
        ("expires", json!("never")),
    ] {
        let mut cookie = item("SID", ".google.com", "/", -1);
        cookie[field] = value;
        assert!(SessionCookies::from_storage_state(
            "https://gemini.google.com",
            &json!({"cookies":[cookie]})
        )
        .is_err());
    }
    let cookies: Vec<_> = (0..65)
        .map(|i| item(&format!("name{i}"), ".google.com", "/", -1))
        .collect();
    assert!(SessionCookies::from_storage_state(
        "https://gemini.google.com",
        &json!({"cookies":cookies})
    )
    .is_err());
    let cookies = SessionCookies::from_storage_state(
        "https://gemini.google.com",
        &json!({"cookies":[
        {"name":"__Secure-SID","value":"x","domain":".google.com","secure":false},
        {"name":"__Host-SID","value":"x","domain":".google.com","secure":true}]}),
    )
    .unwrap();
    assert!(header(&cookies, "/").is_empty());
}
