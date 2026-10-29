//! Account catalog metadata is not a successful-generation attestation.
use super::*;
use serde::Serialize;

#[derive(Serialize)]
pub struct Model {
    slug: String,
    visibility: Option<String>,
    supported_in_api: Option<bool>,
}

pub async fn fetch(
    client: &rquest::Client,
    token: &str,
    account: &str,
) -> Result<Vec<Model>, GatewayError> {
    parse(
        &fetch_body(
            client,
            &format!("{BASE_URL}/models?client_version={VERSION}"),
            token,
            account,
        )
        .await?,
    )
}

fn parse(body: &Value) -> Result<Vec<Model>, GatewayError> {
    let slugs = catalog_models(body)?;
    if slugs.len() > 2048 {
        return Err(GatewayError::bad_request("ChatGPT model catalog too large"));
    }
    Ok(slugs
        .into_iter()
        .map(|slug| {
            let entry = body["models"]
                .as_array()
                .unwrap()
                .iter()
                .find(|item| item["slug"].as_str() == Some(&slug))
                .unwrap();
            Model {
                slug,
                visibility: entry["visibility"]
                    .as_str()
                    .filter(|v| v.len() <= 64)
                    .map(str::to_owned),
                supported_in_api: entry["supported_in_api"].as_bool(),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        http::{HeaderMap, StatusCode},
        routing::get,
        Json, Router,
    };

    #[tokio::test]
    async fn catalog_preserves_visibility_and_uses_current_version_without_following_redirects() {
        let app = Router::new().route("/models", get(|headers: HeaderMap| async move {
            assert_eq!(headers["chatgpt-account-id"], "fixture-account");
            assert_eq!(headers["authorization"], "Bearer fixture-token");
            assert!(headers["user-agent"].to_str().unwrap().contains(VERSION));
            Json(serde_json::json!({"models":[{"slug":"normal","visibility":"list","supported_in_api":true},
                {"slug":"hidden","visibility":"hide","supported_in_api":true},{"slug":"normal"}]}))
        })).route("/redirect", get(|| async { (StatusCode::FOUND, [("location", "/models")]) }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let client = rquest::Client::builder().no_proxy().build().unwrap();
        let body = fetch_body(
            &client,
            &format!("http://{address}/models?client_version={VERSION}"),
            "fixture-token",
            "fixture-account",
        )
        .await
        .unwrap();
        let models = serde_json::to_value(parse(&body).unwrap()).unwrap();
        assert_eq!(models.as_array().unwrap().len(), 2);
        assert_eq!(models[1]["visibility"], "hide");
        assert!(fetch_body(
            &client,
            &format!("http://{address}/redirect"),
            "fixture-token",
            "fixture-account"
        )
        .await
        .is_err());
        task.abort();
        let _ = task.await;
    }
}
