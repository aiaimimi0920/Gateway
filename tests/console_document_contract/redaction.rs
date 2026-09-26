//! Management views and Debug output must not disclose configured secret values.

use super::{document, provider_document};
use neuro_gateway::console::document::{inspect_route_document, validate_route_document};
use neuro_gateway::console::secrets::{redact_route_document, SecretPatch};
use neuro_gateway::routing::config::RouteConfigYaml;
use serde_json::json;
use std::collections::HashMap;
use std::fs;

#[test]
fn bundled_routes_example_is_readable_by_console_redaction_pipeline() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("routes.example.yaml");
    let yaml = fs::read_to_string(&path).expect("read routes.example.yaml");
    let candidate: RouteConfigYaml =
        serde_yaml::from_str(&yaml).expect("routes.example.yaml must parse");

    redact_route_document(&candidate)
        .expect("routes.example.yaml must be readable by the console redaction pipeline");
}

#[test]
fn validated_document_debug_output_never_contains_secret_material() {
    let validated = validate_route_document(provider_document(
        "provider",
        "debug-output-must-not-contain-this-secret",
    ))
    .expect("validated document");
    let debug = format!("{validated:?}");
    assert!(!debug.contains("debug-output-must-not-contain-this-secret"));
    assert!(debug.contains(validated.document_digest()));
}

#[test]
fn redaction_covers_typed_nested_and_keepalive_secrets_without_false_positives() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    api_key: provider-key
    auth_token: provider-auth
    credential_storage_password: provider-storage-password
    keepalive:
      serviceUrl: https://keeper.example.com
      authToken: keeper-auth
    headers:
      Authorization: Bearer provider
      max_tokens: visible-header
    extra_body:
      token: body-token
      nested:
        client_secret: nested-secret
        session_id: nested-session-id
        client_secret_value: nested-secret-value
        authorization_header_value: nested-authorization-value
        aws_secret_access_key: nested-aws-secret-access-key
        max_tokens: 128
        token_expires: visible-token-expires
        token_count: 2
        session_timeout: 30
        oauth_token_endpoint: https://oauth.example.com/token
        token_expires_in_secs: 300
    credentials:
      - api_key: credential-key
        auth_token: credential-auth
        refresh_token: refresh-secret
        keepalive:
          serviceUrl: https://keeper.example.com
          authToken: credential-keeper-auth
        headers:
          Cookie: session=cookie
        extra_body:
          session: body-session
          max_tokens: 256
          token_expires_in_secs: 600
model_routes: []
aliases: {}
"#,
    );

    let redacted = redact_route_document(&active).expect("redact document");
    let provider = &redacted.document.providers[0];
    assert_eq!(provider.api_key, "");
    assert_eq!(provider.auth_token, None);
    assert_eq!(provider.credential_storage_password, None);
    assert!(!provider.headers.contains_key("Authorization"));
    assert_eq!(
        provider.headers.get("max_tokens").map(String::as_str),
        Some("visible-header")
    );
    assert!(!provider.extra_body.contains_key("token"));
    assert!(provider.extra_body["nested"].get("session_id").is_none());
    assert!(provider.extra_body["nested"]
        .get("client_secret_value")
        .is_none());
    assert!(provider.extra_body["nested"]
        .get("authorization_header_value")
        .is_none());
    assert!(provider.extra_body["nested"]
        .get("aws_secret_access_key")
        .is_none());
    assert_eq!(provider.extra_body["nested"]["max_tokens"], json!(128));
    assert_eq!(
        provider.extra_body["nested"]["token_expires"],
        json!("visible-token-expires")
    );
    assert_eq!(provider.extra_body["nested"]["token_count"], json!(2));
    assert_eq!(provider.extra_body["nested"]["session_timeout"], json!(30));
    assert_eq!(
        provider.extra_body["nested"]["oauth_token_endpoint"],
        json!("https://oauth.example.com/token")
    );
    assert_eq!(
        provider.extra_body["nested"]["token_expires_in_secs"],
        json!(300)
    );

    let credential = &provider.credentials[0];
    assert_eq!(credential.id.as_deref(), Some("provider-cred-0"));
    assert_eq!(credential.api_key, None);
    assert_eq!(credential.auth_token, None);
    assert_eq!(credential.refresh_token, None);
    assert!(!credential.headers.contains_key("Cookie"));
    assert!(!credential.extra_body.contains_key("session"));
    assert_eq!(credential.extra_body["max_tokens"], json!(256));
    assert_eq!(credential.extra_body["token_expires_in_secs"], json!(600));

    let paths: Vec<&str> = redacted
        .secrets
        .iter()
        .filter(|descriptor| descriptor.configured)
        .map(|descriptor| descriptor.path.as_str())
        .collect();
    for expected in [
        "/providers/0/api_key",
        "/providers/0/auth_token",
        "/providers/0/credential_storage_password",
        "/providers/0/keepalive/authToken",
        "/providers/0/headers/Authorization",
        "/providers/0/extra_body/token",
        "/providers/0/extra_body/nested/client_secret",
        "/providers/0/extra_body/nested/session_id",
        "/providers/0/extra_body/nested/client_secret_value",
        "/providers/0/extra_body/nested/authorization_header_value",
        "/providers/0/extra_body/nested/aws_secret_access_key",
        "/providers/0/credentials/0/api_key",
        "/providers/0/credentials/0/auth_token",
        "/providers/0/credentials/0/refresh_token",
        "/providers/0/credentials/0/keepalive/authToken",
        "/providers/0/credentials/0/headers/Cookie",
        "/providers/0/credentials/0/extra_body/session",
    ] {
        assert!(paths.contains(&expected), "missing descriptor {expected}");
    }
    assert!(redacted
        .secrets
        .windows(2)
        .all(|pair| pair[0].path <= pair[1].path));
}

#[test]
fn secret_descriptor_preview_is_unicode_safe_and_never_enters_the_document() {
    let active = provider_document("unicode", "密钥秘密值很长而且不能泄露");
    let redacted = redact_route_document(&active).expect("unicode redaction");
    let descriptor = redacted
        .secrets
        .iter()
        .find(|descriptor| descriptor.path == "/providers/0/api_key")
        .expect("api key descriptor");
    assert!(descriptor.configured);
    assert!(descriptor.fingerprint.is_some());
    let preview = descriptor.preview.as_deref().expect("preview");
    assert_ne!(preview, "密钥秘密值很长而且不能泄露");
    assert_eq!(redacted.document.providers[0].api_key, "");
    let serialized = serde_json::to_string(&redacted.document).unwrap();
    assert!(!serialized.contains(preview));
}

#[test]
fn configured_state_distinguishes_provider_empty_string_from_credential_some_empty() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com
    api_key: ""
    credentials:
      - api_key: ""
model_routes: []
aliases: {}
"#,
    );
    let redacted = redact_route_document(&active).expect("redact empty values");
    let descriptors: HashMap<&str, bool> = redacted
        .secrets
        .iter()
        .map(|descriptor| (descriptor.path.as_str(), descriptor.configured))
        .collect();
    assert_eq!(descriptors["/providers/0/api_key"], false);
    assert_eq!(descriptors["/providers/0/credentials/0/api_key"], true);
}

#[test]
fn secret_patch_debug_output_redacts_replace_values() {
    let patch = SecretPatch::replace(
        "/providers/0/api_key",
        json!("debug-must-not-print-this-replacement-secret"),
    );
    let debug = format!("{patch:?}");
    assert!(!debug.contains("debug-must-not-print-this-replacement-secret"));
    assert!(debug.contains("<redacted>"));
}

#[test]
fn composite_cookie_value_keys_are_secret_but_cookie_name_metadata_is_not() {
    let active = document(
        r#"
providers:
  - id: provider
    base_url: https://example.com/v1?cookie_value=query-cookie-secret&cookie_name=session
    headers:
      cookie_value: header-cookie-value-secret
      cookieValue: header-cookie-camel-secret
      set_cookie_value: header-set-cookie-secret
      cookie_header_value: header-cookie-header-secret
      cookie_name: visible-cookie-name
      cookie_names: visible-cookie-names
      cookie_header_name: visible-cookie-header-name
    extra_body:
      nested:
        cookie_value: body-cookie-value-secret
        cookieValue: body-cookie-camel-secret
        set_cookie_value: body-set-cookie-secret
        cookie_header_value: body-cookie-header-secret
        cookie_name: visible-body-cookie-name
        cookie_names: visible-body-cookie-names
model_routes: []
aliases: {}
"#,
    );

    let inspection = inspect_route_document(&active);
    assert!(inspection
        .diagnostics
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "provider_base_url_query_secret_forbidden"));

    let redacted = redact_route_document(&active).expect("composite cookie redaction");
    let provider = &redacted.document.providers[0];
    for key in [
        "cookie_value",
        "cookieValue",
        "set_cookie_value",
        "cookie_header_value",
    ] {
        assert!(!provider.headers.contains_key(key), "header retained {key}");
        assert!(
            provider.extra_body["nested"].get(key).is_none(),
            "body retained {key}"
        );
    }
    for key in ["cookie_name", "cookie_names", "cookie_header_name"] {
        assert!(provider.headers.contains_key(key), "header removed {key}");
    }
    for key in ["cookie_name", "cookie_names"] {
        assert!(
            provider.extra_body["nested"].get(key).is_some(),
            "body removed {key}"
        );
    }
    assert!(!provider.base_url.contains("query-cookie-secret"));
    assert!(provider.base_url.contains("cookie_name=session"));

    let serialized = serde_json::to_string(&redacted.document).unwrap();
    for secret in [
        "query-cookie-secret",
        "header-cookie-value-secret",
        "header-cookie-camel-secret",
        "header-set-cookie-secret",
        "header-cookie-header-secret",
        "body-cookie-value-secret",
        "body-cookie-camel-secret",
        "body-set-cookie-secret",
        "body-cookie-header-secret",
    ] {
        assert!(
            !serialized.contains(secret),
            "cookie secret leaked: {secret}"
        );
    }
}
