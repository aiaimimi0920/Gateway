//! OAuth transport and native credential conversion; token values never enter session views.
use crate::{
    error::GatewayError, protocol::chatgpt::codex_client, routing::config::ProviderCredentialYaml,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{json, Value};

pub const REDIRECT_URI: &str = "http://localhost:1455/auth/callback";
pub const GROUPS: &[&str] = &["free", "plus", "pro", "pro20x", "team-mother", "team-child"];

pub(super) struct Material {
    pub token: String,
    pub refresh: String,
    pub account: String,
    pub email: String,
    pub expires: u64,
    pub expires_at: String,
}

pub(super) async fn exchange(
    client: &rquest::Client,
    code: &str,
    verifier: &str,
) -> Result<Material, GatewayError> {
    let response = send_token_request(|| {
        client
            .post("https://auth.openai.com/oauth/token")
            .form(&[
                ("grant_type", "authorization_code"),
                ("client_id", codex_client::CLIENT_ID),
                ("code", code),
                ("redirect_uri", REDIRECT_URI),
                ("code_verifier", verifier),
            ])
            .timeout(std::time::Duration::from_secs(30))
            .send()
    })
    .await?;
    if !response.status().is_success() {
        return Err(GatewayError::bad_request(format!(
            "ChatGPT OAuth token exchange HTTP {}",
            response.status().as_u16()
        )));
    }
    parse_material(&codex_client::read_json(response).await?)
}

async fn send_token_request<F, Fut>(mut send: F) -> Result<rquest::Response, GatewayError>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<rquest::Response, rquest::Error>>,
{
    let result = tokio::time::timeout(std::time::Duration::from_secs(30), async {
        match send().await {
            // Connect errors precede HTTP delivery. Never replay a code after a response
            // or a read/write failure, where the authorization code may be consumed.
            Err(error) if error.is_connect() => {
                tracing::warn!(error = ?error.without_url(), "Retrying ChatGPT OAuth connection once");
                tokio::time::sleep(std::time::Duration::from_millis(250)).await;
                send().await
            }
            result => result,
        }
    }).await;
    match result {
        Ok(Ok(response)) => Ok(response),
        Ok(Err(error)) => {
            tracing::warn!(error = ?error.without_url(), "ChatGPT OAuth transport failed");
            Err(GatewayError::service_unavailable(
                "ChatGPT OAuth token exchange could not connect; check the network and retry login",
            )
            .with_code("chatgpt_oauth_transport_failed"))
        }
        Err(_) => Err(GatewayError::service_unavailable(
            "ChatGPT OAuth token exchange timed out; retry login",
        )
        .with_code("chatgpt_oauth_transport_failed")),
    }
}

fn string(body: &Value, key: &str) -> Result<String, GatewayError> {
    body[key]
        .as_str()
        .filter(|s| !s.trim().is_empty())
        .map(str::to_string)
        .ok_or_else(|| GatewayError::bad_request(format!("ChatGPT OAuth response missing {key}")))
}

fn claims(token: &str) -> Option<Value> {
    let bytes = URL_SAFE_NO_PAD.decode(token.split('.').nth(1)?).ok()?;
    serde_json::from_slice(&bytes).ok()
}

fn parse_material(body: &Value) -> Result<Material, GatewayError> {
    let token = string(body, "access_token")?;
    let id = body["id_token"]
        .as_str()
        .and_then(claims)
        .or_else(|| claims(&token))
        .ok_or_else(|| {
            GatewayError::bad_request("ChatGPT OAuth response missing account identity")
        })?;
    let account = string(&id["https://api.openai.com/auth"], "chatgpt_account_id")?;
    let email =
        string(&id, "email").or_else(|_| string(&id["https://api.openai.com/profile"], "email"))?;
    let expires = body["expires_in"]
        .as_u64()
        .filter(|v| *v > 0 && *v <= 366 * 86400)
        .ok_or_else(|| GatewayError::bad_request("ChatGPT OAuth response has invalid expiry"))?;
    Ok(Material {
        token,
        refresh: string(body, "refresh_token")?,
        account,
        email,
        expires,
        expires_at: crate::db::format_timestamp(
            time::OffsetDateTime::now_utc() + time::Duration::seconds(expires as i64),
        ),
    })
}

pub(super) fn credential(
    material: &Material,
    group: &str,
    models: &[String],
) -> Result<ProviderCredentialYaml, GatewayError> {
    let id = format!(
        "chatgpt-{}",
        hex::encode(sha2::Sha256::digest(material.account.as_bytes()))
    );
    serde_json::from_value(json!({"id":id, "account_name":material.email, "credential_identity_category_id":group,
        "api_key":material.token, "refresh_token":material.refresh, "refresh_endpoint":"https://auth.openai.com/oauth/token",
        "refresh_client_id":codex_client::CLIENT_ID, "token_expires_in_secs":material.expires, "expires_at":material.expires_at,
        "headers":{"Chatgpt-Account-Id":material.account}, "supported_models":models, "enabled":true}))
        .map_err(|_| GatewayError::bad_request("Cannot construct native ChatGPT credential"))
}
use sha2::Digest;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn converts_trusted_token_endpoint_response_without_echoing_secrets() {
        let jwt = format!("e30.{}.sig", URL_SAFE_NO_PAD.encode(json!({"email":"a@example.com", "https://api.openai.com/auth":{"chatgpt_account_id":"acct"}}).to_string()));
        let material = parse_material(&json!({"access_token":"access", "refresh_token":"refresh", "id_token":jwt, "expires_in":3600})).unwrap();
        let result = credential(&material, "free", &["gpt-test".into()]).unwrap();
        assert_eq!(result.account_name.as_deref(), Some("a@example.com"));
        assert_eq!(result.refresh_token.as_deref(), Some("refresh"));
        assert_eq!(
            result.credential_identity_category_id.as_deref(),
            Some("free")
        );
        assert!(!parse_material(&json!({"access_token":"sensitive"}))
            .err()
            .unwrap()
            .message
            .contains("sensitive"));
    }
}

#[cfg(test)]
#[path = "token_transport_tests.rs"]
mod token_transport_tests;
