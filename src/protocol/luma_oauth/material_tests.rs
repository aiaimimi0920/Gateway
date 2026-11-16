//! Token lifetime and typed parsing never borrow identity from unverified JWT payloads.
use super::{material::TokenMaterial, OAuthError};
use time::OffsetDateTime;

#[test]
fn luma_oauth_token_material_rejects_expiry_overflow_empty_and_oversized_secrets() {
    let now = OffsetDateTime::now_utc();
    for body in [
        r#"{"access_token":"secret","token_type":"bearer","expires_in":18446744073709551615}"#
            .to_owned(),
        r#"{"access_token":"secret","token_type":"bearer","expires_in":9223372036854775807}"#
            .to_owned(),
        r#"{"access_token":"","token_type":"bearer"}"#.to_owned(),
        r#"{"access_token":"secret\r\nInjected","token_type":"bearer"}"#.to_owned(),
        r#"{"access_token":"secret","token_type":"bearer","token_type":"bearer"}"#.to_owned(),
        format!(
            r#"{{"access_token":"{}","token_type":"bearer"}}"#,
            "x".repeat(32_769)
        ),
    ] {
        assert_eq!(
            TokenMaterial::parse(body.as_bytes(), now, "client".into(), "redirect".into())
                .unwrap_err(),
            OAuthError::InvalidResponse
        );
    }
    let expired = br#"{"access_token":"secret","token_type":"bearer","expires_in":1}"#;
    assert_eq!(
        TokenMaterial::parse(
            expired,
            now - time::Duration::minutes(1),
            "client".into(),
            "redirect".into()
        )
        .unwrap_err(),
        OAuthError::Expired
    );
}
