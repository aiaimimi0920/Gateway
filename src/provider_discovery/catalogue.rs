//! Model-directory formats are independent of generation protocols.
use super::{registry, transport, DiscoveredProtocol as P};
use serde_json::Value;

pub(super) fn parse(body: &Value) -> Vec<String> {
    if body.get("error").is_some_and(|e| !e.is_null()) {
        return Vec::new();
    }
    let entries = body
        .get("data")
        .or_else(|| body.get("models"))
        .or_else(|| body.get("modelSummaries"))
        .and_then(Value::as_array);
    let mut models = Vec::new();
    for item in entries.into_iter().flatten().take(2048) {
        let id = item
            .get("id")
            .or_else(|| item.get("name"))
            .or_else(|| item.get("model"))
            .or_else(|| item.get("modelId"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        let id = id.strip_prefix("models/").unwrap_or(id).trim();
        if !id.is_empty()
            && id.len() <= 256
            && !id.chars().any(char::is_control)
            && !models.iter().any(|m| m == id)
        {
            models.push(id.to_owned());
        }
    }
    models
}

pub(super) async fn load(
    client: &rquest::Client,
    source: &str,
    key: &str,
) -> anyhow::Result<Vec<String>> {
    let specs = [
        (P::DashscopeText, "/models"),
        (P::ChatCompletions, "/models"),
        (P::Messages, "/models"),
        (P::GeminiGenerateContent, "/models"),
        (P::OllamaChat, "/tags"),
        (P::BedrockConverse, "/foundation-models"),
    ];
    let mut union = Vec::new();
    let mut visited = std::collections::HashSet::new();
    let mut succeeded = std::collections::HashSet::new();
    {
        for (protocol, path) in specs {
            let bases = if protocol == P::DashscopeText {
                let root = source.trim_end_matches('/');
                let root = root
                    .strip_suffix("/compatible-mode/v1")
                    .or_else(|| root.strip_suffix("/api/v1"))
                    .unwrap_or(root);
                vec![format!("{root}/compatible-mode/v1")]
            } else {
                registry::bases(source, protocol)?
            };
            for base in bases {
                let endpoint = format!("{base}{path}");
                if succeeded.contains(&endpoint) {
                    continue;
                }
                if !visited.insert((endpoint.clone(), protocol)) {
                    continue;
                }
                let mut found = false;
                let mut page: Option<String> = None;
                for _ in 0..4 {
                    let mut request = client.get(&endpoint).timeout(super::REQUEST_TIMEOUT);
                    if let Some(token) = page.as_deref() {
                        request = request.query(&[(
                            if protocol == P::GeminiGenerateContent {
                                "pageToken"
                            } else {
                                "page_token"
                            },
                            token,
                        )]);
                    }
                    let Ok(response) = registry::authenticate(request, protocol, key).send().await
                    else {
                        break;
                    };
                    let Ok(body) = transport::read_json(response).await else {
                        break;
                    };
                    let models = parse(&body);
                    found |= !models.is_empty();
                    for model in models {
                        if union.len() < 2048 && !union.contains(&model) {
                            union.push(model);
                        }
                    }
                    page = body
                        .get("nextPageToken")
                        .or_else(|| body.get("next_page_token"))
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty() && s.len() < 4096)
                        .map(str::to_owned);
                    if page.is_none() || union.len() == 2048 {
                        break;
                    }
                }
                if found {
                    succeeded.insert(endpoint);
                }
            }
        }
    }
    anyhow::ensure!(!union.is_empty(), "No model catalogue found.");
    Ok(union)
}
