use super::*;

fn url(path: &str) -> Url {
    Url::parse(&format!("https://chatgpt.com{path}")).unwrap()
}

#[test]
fn settings_cookies_reach_responses_with_scope_and_secure_rules() {
    let mut jar = NativeCookieStore::default();
    jar.receive(
        &url("/backend-api/wham/settings/user"),
        "a=one; Path=/; Secure; HttpOnly",
    );
    assert_eq!(
        jar.header(&url("/backend-api/codex/responses")).as_deref(),
        Some("a=one")
    );
    assert!(jar
        .header(&Url::parse("http://chatgpt.com/").unwrap())
        .is_none());
    assert!(jar
        .header(&Url::parse("https://other.example/").unwrap())
        .is_none());
}

#[test]
fn replacement_deletion_expiry_and_path_are_enforced() {
    let mut jar = NativeCookieStore::default();
    jar.receive(&url("/"), "a=one; Path=/private");
    assert!(jar.header(&url("/public")).is_none());
    jar.receive(&url("/"), "a=two; Path=/private");
    assert_eq!(jar.header(&url("/private/x")).as_deref(), Some("a=two"));
    jar.receive(&url("/"), "a=deleted; Path=/private; Max-Age=0");
    assert!(jar.header(&url("/private/x")).is_none());
    jar.receive(
        &url("/"),
        "b=expired; Expires=Thu, 01 Jan 1970 00:00:00 GMT",
    );
    assert!(jar.header(&url("/")).is_none());
}

#[test]
fn unrelated_parent_domains_and_oversized_values_are_rejected() {
    let mut jar = NativeCookieStore::default();
    for value in ["a=1; Domain=com", "a=1; Domain=other.example"] {
        jar.receive(&url("/"), value);
    }
    jar.receive(
        &url("/"),
        &format!("large={}", "x".repeat(MAX_COOKIE_BYTES)),
    );
    assert!(jar.header(&url("/")).is_none());
    jar.receive(&url("/"), "allowed=1; Domain=.chatgpt.com; Path=/");
    assert_eq!(jar.header(&url("/")).as_deref(), Some("allowed=1"));
}

#[test]
fn capacity_is_bounded_and_existing_state_survives_rejection() {
    let mut jar = NativeCookieStore::default();
    for n in 0..MAX_COOKIES + 20 {
        jar.receive(&url("/"), &format!("c{n}=v; Path=/"));
    }
    assert_eq!(jar.store.iter_any().count(), MAX_COOKIES);
    assert!(jar.header(&url("/")).unwrap().contains("c0=v"));
    let mut other = NativeCookieStore::default();
    for n in 0..20 {
        other.receive(&url("/"), &format!("c{n}={}; Path=/", "x".repeat(3000)));
    }
    assert!(
        other
            .store
            .iter_any()
            .map(|c| c.to_string().len())
            .sum::<usize>()
            <= MAX_STORE_BYTES
    );
    assert!(other.header(&url("/")).is_none());
}
