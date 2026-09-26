use super::*;

#[test]
fn storage_state_to_pure_http_session_extracts_google_cookie_material() {
    let storage_state = json!({
        "cookies": [
            {
                "name": "__Secure-1PAPISID",
                "value": "sapisid-123",
                "domain": ".google.com",
                "path": "/"
            },
            {
                "name": "NID",
                "value": "nid-123",
                "domain": ".google.com",
                "path": "/"
            },
            {
                "name": "not-for-google",
                "value": "ignore",
                "domain": ".example.com",
                "path": "/"
            }
        ]
    });

    let session = storage_state_to_pure_http_session(
            &storage_state,
            "https://generativelanguage.googleapis.com/v1beta/models/gemini-3-flash-preview:generateContent",
            "https://gemini.google.com",
            "1",
        )
        .unwrap();

    assert_eq!(session.sapisid, "sapisid-123");
    assert_eq!(session.auth_user, "1");
    assert!(session
        .cookie_header
        .contains("__Secure-1PAPISID=sapisid-123"));
    assert!(session.cookie_header.contains("NID=nid-123"));
    assert!(!session.cookie_header.contains("not-for-google"));
}

#[test]
fn storage_state_to_pure_http_session_excludes_accounts_only_cookies_for_gemini_host() {
    let storage_state = json!({
        "cookies": [
            {
                "name": "SAPISID",
                "value": "sapisid-123",
                "domain": ".google.com",
                "path": "/"
            },
            {
                "name": "__Secure-1PSIDTS",
                "value": "psidts-123",
                "domain": ".google.com",
                "path": "/"
            },
            {
                "name": "LSID",
                "value": "accounts-lsid-123",
                "domain": "accounts.google.com",
                "path": "/"
            },
            {
                "name": "__Host-1PLSID",
                "value": "accounts-host-plsid-123",
                "domain": "accounts.google.com",
                "path": "/"
            }
        ]
    });

    let session = storage_state_to_pure_http_session(
        &storage_state,
        "https://gemini.google.com/share/fe24c455a570",
        "https://gemini.google.com",
        "0",
    )
    .unwrap();

    assert!(session.cookie_header.contains("SAPISID=sapisid-123"));
    assert!(session
        .cookie_header
        .contains("__Secure-1PSIDTS=psidts-123"));
    assert!(!session.cookie_header.contains("LSID=accounts-lsid-123"));
    assert!(!session
        .cookie_header
        .contains("__Host-1PLSID=accounts-host-plsid-123"));
}

#[test]
fn storage_state_to_pure_http_session_uses_fixture_host_fallback_for_google_cookies() {
    let storage_state = json!({
        "cookies": [
            {
                "name": "SAPISID",
                "value": "fixture-sapisid-123",
                "domain": ".google.com",
                "path": "/"
            },
            {
                "name": "__Secure-1PSID",
                "value": "fixture-psid-456",
                "domain": ".google.com",
                "path": "/"
            }
        ]
    });

    let session = storage_state_to_pure_http_session(
        &storage_state,
        "http://host.docker.internal:42335/app",
        "http://host.docker.internal:42335",
        "0",
    )
    .unwrap();

    assert_eq!(session.sapisid, "fixture-sapisid-123");
    assert!(session
        .cookie_header
        .contains("SAPISID=fixture-sapisid-123"));
    assert!(session
        .cookie_header
        .contains("__Secure-1PSID=fixture-psid-456"));
}

#[test]
fn sapisid_authorization_matches_google_hash_contract() {
    let header =
        build_sapisid_authorization("sapisid-123", "https://gemini.google.com", 1_700_000_000)
            .unwrap();

    assert_eq!(
            header,
            "SAPISIDHASH 1700000000_46035c0888ca202e6ab144cddb345bf95fff120c SAPISID1PHASH 1700000000_46035c0888ca202e6ab144cddb345bf95fff120c SAPISID3PHASH 1700000000_46035c0888ca202e6ab144cddb345bf95fff120c"
        );
}
