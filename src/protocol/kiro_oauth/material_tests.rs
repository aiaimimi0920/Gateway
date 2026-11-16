use super::*;
use serde_json::json;
use time::OffsetDateTime;

fn parse(bytes: &[u8]) -> Result<TokenMaterial, OAuthError> {
    TokenMaterial::parse(
        &Reply {
            status: 200,
            bytes: bytes.into(),
        },
        OffsetDateTime::from_unix_timestamp(1_790_000_000).unwrap(),
    )
}
#[test]
fn token_material_preserves_absent_expiry_refresh_and_exact_reported_expiry() {
    let missing = parse(br#"{"accessToken":"opaque"}"#).unwrap();
    assert_eq!(missing.refresh_token(), None);
    assert_eq!(missing.expires_at(), None);
    let material = parse(
        br#"{"accessToken":"opaque","refreshToken":"rotated","tokenType":"Bearer","expiresIn":60}"#,
    )
    .unwrap();
    assert_eq!(material.access_token(), "opaque");
    assert_eq!(material.refresh_token(), Some("rotated"));
    assert_eq!(
        material.expires_at().unwrap().unix_timestamp(),
        1_790_000_060
    );
    assert!(!format!("{material:?}").contains("opaque"));
}
#[test]
fn token_material_rejects_duplicate_fields_wrong_types_empty_and_overflow() {
    for bytes in [
        br#"{"accessToken":"a","accessToken":"b"}"#.as_slice(),
        br#"{"accessToken":"a","expiresIn":0}"#,
        br#"{"accessToken":"a","expiresIn":18446744073709551615}"#,
        br#"{"accessToken":"a","refreshToken":""}"#,
        br#"{"accessToken":"a","tokenType":"Basic"}"#,
        br#"{"accessToken":"a","error":"private-error"}"#,
        br#"{"accessToken":"a","expiresIn":"3600"}"#,
        br#"{"accessToken":"a b"}"#,
        br#"{"access_token":"wrong-wire"}"#,
    ] {
        assert_eq!(parse(bytes).unwrap_err(), OAuthError::InvalidResponse);
    }
}
#[test]
fn directory_region_start_url_and_login_destinations_are_bound() {
    let builder = Directory::builder_id();
    assert_eq!(builder.region(), "us-east-1");
    assert_eq!(builder.start_url(), "https://view.awsapps.com/start");
    let idc =
        Directory::identity_center("eu-west-1", "https://d-synthetic.awsapps.com/start").unwrap();
    assert_eq!(idc.oidc_origin(), "https://oidc.eu-west-1.amazonaws.com");
    assert!(idc.verification_url("https://device.sso.eu-west-1.api.aws/?user_code=ABCD#device"));
    assert!(!idc.verification_url("https://device.sso.us-east-1.amazonaws.com/"));
    for url in [
        "https://untrusted.invalid/start",
        "http://view.awsapps.com/start",
        "https://user@view.awsapps.com/start",
        "https://d.awsapps.com/start?redirect=private",
        "https://d.awsapps.com/start#private",
        "https://d.awsapps.com.evil/start",
        "https://d.awsapps.com:444/start",
        "https://nested.d.awsapps.com/start",
        "https://d.awsapps.com/not-start",
    ] {
        assert!(
            Directory::identity_center("eu-west-1", url).is_err(),
            "{url}"
        );
    }
    for region in [
        "../us-east-1",
        "us-east-1.evil",
        "cn-north-1",
        "us-gov-west-1",
        "us-madeup-99",
        "",
    ] {
        assert!(Directory::identity_center(region, "https://d.awsapps.com/start").is_err());
    }
    for url in [
        "https://device.sso.us-east-1.amazonaws.com.evil/",
        "http://device.sso.us-east-1.amazonaws.com/",
        "https://device.sso.us-east-1.amazonaws.com/private",
        "https://127.0.0.1/",
    ] {
        assert!(!builder.verification_url(url));
    }
}
#[test]
fn rejection_is_machine_code_only_and_never_includes_upstream_prose() {
    for (code, expected) in [
        ("invalid_grant", OAuthError::ReauthorizationRequired),
        ("invalid_client", OAuthError::ReauthorizationRequired),
        ("access_denied", OAuthError::AccessDenied),
        ("expired_token", OAuthError::Expired),
        ("private", OAuthError::Rejected),
    ] {
        let reply = Reply {
            status: 400,
            bytes: json!({"error":code,"error_description":"private-material"})
                .to_string()
                .into_bytes(),
        };
        assert_eq!(reply.rejection(), expected);
        assert!(!expected.to_string().contains("private-material"));
    }
    let duplicate = Reply {
        status: 400,
        bytes: br#"{"error":"authorization_pending","error":"slow_down"}"#.to_vec(),
    };
    assert_eq!(duplicate.error_code(), None);
}
#[cfg(not(feature = "line-kiro-official-vendor-api"))]
#[tokio::test]
async fn compiled_out_line_rejects_registration_without_network() {
    let result = ClientRegistration::register(
        &rquest::Client::new(),
        Directory::builder_id(),
        &[Scope::Conversations],
    )
    .await;
    assert_eq!(result.unwrap_err(), OAuthError::FeatureDisabled);
}
