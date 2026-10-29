//! Search tests share the three-adapter transport, with explicit provider wire contracts.
use std::time::{Duration, Instant};

use futures::StreamExt;
use serde_json::{json, Value};

use super::{
    console_model_probe::ModelProbeResult, ProviderPayloadProbeReport, ProviderPayloadProbeStatus,
};
use crate::{
    routing::config::CredentialProbeTarget, state::AppState, upstream::client::UpstreamClient,
};

#[derive(Clone, Copy)]
pub(super) enum SearchProbeKind {
    You,
    Exa,
    Tavily,
    Linkup,
    WebSearchApi,
    JinaSearch,
    JinaReader,
}

impl SearchProbeKind {
    pub(super) fn from_preset(preset: Option<&str>) -> Option<Self> {
        match preset {
            Some("you") => Some(Self::You),
            Some("exa") => Some(Self::Exa),
            Some("tavily") => Some(Self::Tavily),
            Some("linkup") => Some(Self::Linkup),
            Some("websearchapi") => Some(Self::WebSearchApi),
            Some("jina") => Some(Self::JinaSearch),
            _ => None,
        }
    }

    pub(super) fn supports_model(self, model: &str) -> bool {
        // Operation aliases are checked after mapping; a fetch must not pass a search test.
        match self {
            Self::You => matches!(model, "you-search" | "you-fetch"),
            Self::Exa => model == "exa-search",
            Self::Tavily => matches!(model, "tavily-search" | "tavily-fetch"),
            Self::Linkup => model == "linkup-search",
            Self::WebSearchApi => matches!(model, "websearchapi-search" | "websearchapi-fetch"),
            Self::JinaSearch => matches!(model, "jina-search" | "jina-fetch"),
            Self::JinaReader => model == "jina-fetch",
        }
    }

    pub(super) fn default_prompt(self, model: &str) -> &'static str {
        if model.ends_with("-fetch") {
            "https://example.com/"
        } else {
            "Rust programming language official documentation"
        }
    }

    fn point(self) -> &'static str {
        match self {
            Self::You => "You Search POST /v1/search",
            Self::Exa => "Exa Search POST /search",
            Self::Tavily => "Tavily Search POST /search",
            Self::Linkup => "Linkup Search POST /v1/search",
            Self::WebSearchApi => "WebSearchAPI Search POST /ai-search",
            Self::JinaSearch => "Jina Search POST /search",
            Self::JinaReader => "Jina Reader POST /",
        }
    }

    fn request(self, model: &str, query: &str) -> Value {
        match self {
            Self::You => json!({"model":model,"query":query,"count":3}),
            Self::Exa => json!({"model":model,"query":query,"numResults":3}),
            // Pin a bounded basic search; do not opt in to answer generation or raw pages.
            Self::Tavily => json!({"model":model,"query":query,"max_results":3,
                "search_depth":"basic","auto_parameters":false,"include_answer":false,
                "include_raw_content":false,"include_images":false}),
            Self::Linkup => json!({"model":model,"query":query,"maxResults":3,
                "depth":"standard","outputType":"searchResults","includeImages":false}),
            Self::WebSearchApi => json!({"model":model,"query":query,"maxResults":3,
                "includeContent":false,"includeAnswer":false}),
            Self::JinaSearch => json!({"model":model,"query":query,"num":3,"fallback":false,
                "retainImages":"none","withGeneratedAlt":false}),
            Self::JinaReader => json!({"model":model,"url":query,"timeout":30,"tokenBudget":10000,
                "retainImages":"none","withGeneratedAlt":false}),
        }
    }

    fn results_path(self) -> &'static str {
        match self {
            Self::You => "/results/web",
            Self::Exa | Self::Tavily | Self::Linkup => "/results",
            Self::WebSearchApi => "/organic",
            Self::JinaSearch | Self::JinaReader => "/data",
        }
    }

    fn title_field(self) -> &'static str {
        match self {
            Self::Linkup => "name",
            _ => "title",
        }
    }
}

pub(super) async fn probe(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    query: &str,
    kind: SearchProbeKind,
) -> (String, ProviderPayloadProbeReport) {
    let kind = if matches!(kind, SearchProbeKind::JinaSearch) && model == "jina-fetch" {
        SearchProbeKind::JinaReader
    } else {
        kind
    };
    let recording = if model.ends_with("-fetch") {
        super::model_probe_recording::begin_fetch(state, target, model).await
    } else {
        super::model_probe_recording::begin_search(state, target, model).await
    };
    let started = Instant::now();
    let result = tokio::time::timeout(
        Duration::from_secs(60),
        search(state, target, model, query, kind),
    )
    .await
    .unwrap_or_else(|_| failed(None, "Search call timed out after 60 seconds."));
    let health_recovery = super::model_probe_recording::finish(
        state,
        target,
        model,
        recording,
        started.elapsed(),
        &result,
    )
    .await;
    let answer = result.reply.as_ref().ok().cloned();
    let (status, message) = match result.reply {
        Ok(summary) => (ProviderPayloadProbeStatus::Passed, summary),
        Err(error) => (ProviderPayloadProbeStatus::Failed, error),
    };
    (
        if model.ends_with("-fetch") {
            format!(
                "Fetch POST {}",
                target.payload.fetch_path.as_deref().unwrap_or_default()
            )
        } else {
            kind.point().into()
        },
        ProviderPayloadProbeReport {
            health_recovery: Some(health_recovery),
            status,
            message,
            answer,
        },
    )
}

async fn search(
    state: &AppState,
    target: &CredentialProbeTarget,
    model: &str,
    query: &str,
    kind: SearchProbeKind,
) -> ModelProbeResult {
    let Ok(flow) = crate::protocol::call_flow::prepare(&target.payload) else {
        return failed(None, "Cannot prepare the search call flow.");
    };
    let payload = flow.payload.as_ref();
    if flow.call_adapter != "search_api_compatible" || flow.executor != "direct_http" {
        return failed(
            None,
            "Search tests require the standard search API adapter.",
        );
    }
    let is_fetch = model.ends_with("-fetch");
    let body = if is_fetch && !matches!(kind, SearchProbeKind::JinaReader) {
        json!({"model":model, "url":query})
    } else {
        kind.request(model, query)
    };
    let request = if is_fetch {
        crate::protocol::search_api::normalize_fetch(body)
    } else {
        crate::protocol::search_api::normalize_search(body)
    };
    let Ok(request) = request else {
        return failed(None, "Cannot build the search test query.");
    };
    let Ok(plan) = UpstreamClient::build_request_plan(payload, &request, model, false) else {
        return failed(None, "Cannot build the search test request.");
    };
    let headers = crate::upstream::headers::build_inference_headers(payload, &request, model, None);
    let Ok(pending) = state
        .upstream_client
        .send_profiled_plan(payload, &plan, headers)
    else {
        return failed(None, "Cannot prepare the search transport.");
    };
    let Ok(response) = pending.send().await else {
        return failed(None, "Cannot reach the search endpoint.");
    };
    let status = response.status().as_u16();
    if !response.status().is_success() {
        let reason = match status {
            401 => "Credential authentication rejected",
            402 => "Account quota or payment required",
            403 => "Credential has no access to this search endpoint",
            429 => "Upstream rate limit or quota exhausted",
            432 if matches!(kind, SearchProbeKind::Tavily) => "Account plan usage limit exceeded",
            433 if matches!(kind, SearchProbeKind::Tavily) => {
                "Account pay-as-you-go limit exceeded"
            }
            _ => "Upstream search request failed",
        };
        // Never echo an upstream error body, a search snippet, or a credential.
        return failed(Some(status), &format!("HTTP {status}: {reason}."));
    }
    let stream = response.bytes_stream();
    futures::pin_mut!(stream);
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        match chunk {
            Ok(chunk) if bytes.len() + chunk.len() <= 1_048_576 => bytes.extend_from_slice(&chunk),
            Ok(_) => return failed(Some(status), "Search response exceeded 1 MiB."),
            Err(_) => return failed(Some(status), "Search response was interrupted."),
        }
    }
    let Ok(body) = serde_json::from_slice::<Value>(&bytes) else {
        return failed(Some(status), "Search endpoint returned invalid JSON.");
    };
    if is_fetch && !matches!(kind, SearchProbeKind::JinaReader) {
        return match crate::upstream::search_fetch_response::validate(payload, &body, false) {
            Ok(count) => ModelProbeResult {
                status: Some(status),
                usage: Value::Null,
                reply: Ok(format!("Fetch returned {count} web pages.")),
            },
            Err(_) => failed(
                Some(status),
                "HTTP succeeded but no usable web page was returned.",
            ),
        };
    }
    let jina = matches!(
        kind,
        SearchProbeKind::JinaSearch | SearchProbeKind::JinaReader
    );
    let envelope_failed = jina && (body["code"] != 200 || body["status"] != 20000);
    if matches!(kind, SearchProbeKind::JinaReader) {
        if envelope_failed
            || body.get("error").is_some_and(|error| !error.is_null())
            || !usable_result(&body["data"], "title")
            || !body["data"]["content"]
                .as_str()
                .is_some_and(|content| !content.trim().is_empty())
        {
            return failed(
                Some(status),
                "HTTP succeeded but no usable web page was returned.",
            );
        }
        return ModelProbeResult {
            status: Some(status),
            usage: Value::Null,
            reply: Ok("Reader returned a web page.".into()),
        };
    }
    let count = body
        .pointer(kind.results_path())
        .and_then(Value::as_array)
        .map_or(0, |results| {
            results
                .iter()
                .filter(|item| usable_result(item, kind.title_field()))
                .count()
        });
    if count == 0
        || envelope_failed
        || body.get("error").is_some_and(|error| !error.is_null())
        || (matches!(kind, SearchProbeKind::Tavily)
            && body
                .pointer("/detail/error")
                .is_some_and(|error| !error.is_null()))
    {
        return failed(
            Some(status),
            "HTTP succeeded but no usable web results were returned.",
        );
    }
    ModelProbeResult {
        status: Some(status),
        usage: Value::Null,
        reply: Ok(format!("Search returned {count} web results.")),
    }
}

fn usable_result(item: &Value, title_field: &str) -> bool {
    let title = item
        .get(title_field)
        .and_then(Value::as_str)
        .is_some_and(|s| !s.trim().is_empty());
    let url = item
        .get("url")
        .and_then(Value::as_str)
        .and_then(|s| url::Url::parse(s).ok());
    title
        && url
            .is_some_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}

fn failed(status: Option<u16>, message: &str) -> ModelProbeResult {
    ModelProbeResult {
        status,
        reply: Err(message.into()),
        usage: Value::Null,
    }
}
