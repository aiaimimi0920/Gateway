//! Trusted Google token + userinfo exchanges. No JWT claims or caller-provided account IDs.
use crate::{
    error::GatewayError, protocol::antigravity as config, routing::config::ProviderCredentialYaml,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::time::Duration;

pub(super) struct Material {
    pub client_id: String,
    pub token: String,
    pub refresh: String,
    pub account: String,
    pub email: String,
    pub expires: u64,
    pub expires_at: String,
}

async fn read(response: rquest::Response) -> Result<Value, GatewayError> {
    let bytes = crate::upstream::antigravity::transport::read(response, 65536).await?;
    serde_json::from_slice(&bytes).map_err(|_| {
        GatewayError::bad_request("Invalid Google OAuth response; private body omitted")
    })
}

pub(super) async fn exchange(
    client: &rquest::Client,
    code: &str,
    verifier: &str,
    oauth_client: &config::OAuthClient,
) -> Result<Material, GatewayError> {
    // One exchange only: a network/read failure may have consumed the authorization code.
    tokio::time::timeout(Duration::from_secs(35), async {
        let response = client
            .post(config::TOKEN_ENDPOINT)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", oauth_client.client_id()),
                ("client_secret", oauth_client.client_secret()),
                ("code", code),
                ("redirect_uri", config::REDIRECT_URI),
                ("code_verifier", verifier),
            ])
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|_| {
                GatewayError::service_unavailable(
                    "Google OAuth exchange failed; retry authorization",
                )
            })?;
        let body = read(response).await?;
        let token = string(&body, "access_token")?;
        let response = client
            .get("https://www.googleapis.com/oauth2/v2/userinfo")
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .bearer_auth(&token)
            .timeout(Duration::from_secs(15))
            .send()
            .await
            .map_err(|_| {
                GatewayError::service_unavailable(
                    "Google account identity lookup failed; retry authorization",
                )
            })?;
        let identity = read(response).await?;
        from_responses(&body, &identity, oauth_client.client_id())
    })
    .await
    .map_err(|_| {
        GatewayError::service_unavailable("Google OAuth exchange timed out; retry authorization")
    })?
}

fn string(body: &Value, key: &str) -> Result<String, GatewayError> {
    body[key]
        .as_str()
        .filter(|s| !s.trim().is_empty() && s.len() <= 16384)
        .map(str::to_owned)
        .ok_or_else(|| {
            GatewayError::bad_request(format!("Google OAuth response missing or invalid {key}"))
        })
}

fn from_responses(
    body: &Value,
    identity: &Value,
    client_id: &str,
) -> Result<Material, GatewayError> {
    let expires = body["expires_in"]
        .as_u64()
        .filter(|v| *v > 0 && *v <= 86400)
        .ok_or_else(|| GatewayError::bad_request("Google OAuth response has invalid expiry"))?;
    Ok(Material {
        client_id: client_id.to_owned(),
        token: string(body, "access_token")?,
        refresh: string(body, "refresh_token")?,
        account: string(identity, "id")?,
        email: string(identity, "email")?,
        expires,
        expires_at: crate::db::format_timestamp(
            time::OffsetDateTime::now_utc() + time::Duration::seconds(expires as i64),
        ),
    })
}

pub(super) fn credential(
    material: &Material,
    project: &str,
    models: &[String],
) -> Result<ProviderCredentialYaml, GatewayError> {
    let id = format!(
        "antigravity-{}",
        hex::encode(Sha256::digest(material.account.as_bytes()))
    );
    serde_json::from_value(json!({"id":id, "account_name":material.email, "credential_identity_category_id":"default",
        "api_key":material.token, "refresh_token":material.refresh, "refresh_endpoint":config::TOKEN_ENDPOINT,
        "refresh_client_id":material.client_id, "token_expires_in_secs":material.expires, "expires_at":material.expires_at,
        "extra_body":{"antigravity_account_id":material.account, "antigravity_project_id":project},
        "supported_models":models, "enabled":true}))
        .map_err(|_| GatewayError::bad_request("Cannot construct native Antigravity credential"))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn antigravity_material_uses_trusted_userinfo_and_native_refresh_fields() {
        let material = from_responses(&json!({"access_token":"secret-access", "refresh_token":"secret-refresh", "expires_in":3600}),
            &json!({"id":"google-sub", "email":"fixture@example.invalid"}), "synthetic-client").unwrap();
        let cred = credential(&material, "real-project", &["catalog-model".into()]).unwrap();
        assert!(cred.id.unwrap().starts_with("antigravity-"));
        assert_eq!(
            cred.refresh_endpoint.as_deref(),
            Some(config::TOKEN_ENDPOINT)
        );
        assert_eq!(cred.extra_body["antigravity_project_id"], "real-project");
        assert!(cred.headers.is_empty());
        assert_eq!(cred.refresh_client_id.as_deref(), Some("synthetic-client"));
        let error = from_responses(
            &json!({"access_token":"secret-value"}),
            &json!({}),
            "synthetic-client",
        )
        .err()
        .unwrap();
        assert!(!error.message.contains("secret-value"));
    }
}
