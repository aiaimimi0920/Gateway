//! Shared Codex client identity: the upstream model catalog filters by this version.
use crate::error::GatewayError;
use serde_json::Value;

pub const VERSION: &str = "0.154.0";
pub const USER_AGENT: &str = "codex_cli_rs/0.154.0 (Mac OS 26.3.1; arm64) iTerm.app/3.6.9";
pub const CLIENT_ID: &str = "app_EMoamEEZ73f0CkXaXp7hrann";
pub const BASE_URL: &str = "https://chatgpt.com/backend-api/codex";

pub async fn read_json(mut response: rquest::Response) -> Result<Value, GatewayError> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| GatewayError::bad_request("ChatGPT response interrupted"))?
    {
        if bytes.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err(GatewayError::bad_request("ChatGPT response too large"));
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes)
        .map_err(|_| GatewayError::bad_request("ChatGPT returned invalid JSON"))
}

pub async fn models(
    client: &rquest::Client,
    token: &str,
    account: &str,
) -> Result<Vec<String>, GatewayError> {
    let response = client
        .get(format!("{BASE_URL}/models?client_version={VERSION}"))
        .bearer_auth(token)
        .header("Chatgpt-Account-Id", account)
        .header("Originator", "codex_cli_rs")
        .header("User-Agent", USER_AGENT)
        .timeout(std::time::Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| GatewayError::service_unavailable("Cannot read ChatGPT model catalog"))?;
    if !response.status().is_success() {
        return Err(GatewayError::bad_request(format!(
            "ChatGPT model catalog HTTP {}",
            response.status().as_u16()
        )));
    }
    let body = read_json(response).await?;
    catalog_models(&body)
}

pub fn catalog_models(body: &Value) -> Result<Vec<String>, GatewayError> {
    let mut models = Vec::new();
    for item in body["models"]
        .as_array()
        .ok_or_else(|| GatewayError::bad_request("Missing ChatGPT models"))?
    {
        if let Some(slug) = item["slug"]
            .as_str()
            .filter(|s| !s.is_empty() && s.len() <= 200 && !s.contains(['*', '?']))
        {
            if !models.iter().any(|model| model == slug) {
                models.push(slug.to_string());
            }
        }
    }
    if models.is_empty() {
        return Err(GatewayError::bad_request(
            "ChatGPT returned no models for this account",
        ));
    }
    Ok(models)
}

pub fn preferred_model(models: &[String]) -> Option<String> {
    models
        .iter()
        .find(|m| m.as_str() == "gpt-5.6-luna")
        .or_else(|| {
            models
                .iter()
                .find(|m| m.starts_with("gpt-") && m.as_str() != "gpt-reserve")
        })
        .or_else(|| models.first())
        .cloned()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn preserve_account_catalog_and_prefer_normal_generation() {
        let models = catalog_models(&serde_json::json!({"models":[{"slug":"codex-auto-review"},{"slug":"gpt-5.6-luna"},{"slug":"gpt-5.6-luna"}]})).unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(preferred_model(&models).as_deref(), Some("gpt-5.6-luna"));
        assert!(catalog_models(&serde_json::json!({"models":[]})).is_err());
    }
}
