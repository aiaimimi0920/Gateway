use crate::error::GatewayError;
use crate::protocol::canonical::{CanonicalRelayRequest, EndpointKind};
use crate::protocol::search_api;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::common::RequestPlan;
use rquest::Method;

pub(crate) fn require_search_api_path<'a>(
    payload: &'a ProviderAccountPayload,
    endpoint_kind: EndpointKind,
) -> Result<&'a str, GatewayError> {
    let path = match endpoint_kind {
        EndpointKind::Search => payload.search_path.as_deref(),
        EndpointKind::Fetch => payload.fetch_path.as_deref(),
        EndpointKind::ResearchCreate | EndpointKind::ResearchList | EndpointKind::ResearchGet => {
            payload.research_path.as_deref()
        }
        EndpointKind::CreditsBalance => payload.balance_path.as_deref(),
        _ => None,
    };

    path.ok_or_else(|| {
        GatewayError::bad_request(format!(
            "Selected search provider does not support {} requests",
            search_endpoint_label(endpoint_kind)
        ))
        .with_code("unsupported_search_provider_endpoint")
    })
}

pub(crate) fn search_endpoint_label(endpoint_kind: EndpointKind) -> &'static str {
    match endpoint_kind {
        EndpointKind::Search => "search",
        EndpointKind::Fetch => "fetch",
        EndpointKind::Completions => "completions",
        EndpointKind::Embeddings => "embeddings",
        EndpointKind::ImagesGenerations => "image generations",
        EndpointKind::ImagesEdits => "image edits",
        EndpointKind::MusicGenerations => "music generations",
        EndpointKind::VideosGenerations => "video generations",
        EndpointKind::AudioTranscriptions => "audio transcriptions",
        EndpointKind::AudioSpeech => "audio speech",
        EndpointKind::ResearchCreate | EndpointKind::ResearchList | EndpointKind::ResearchGet => {
            "research"
        }
        EndpointKind::CreditsBalance => "credits balance",
        EndpointKind::ChatCompletions => "chat completions",
        EndpointKind::Messages => "messages",
        EndpointKind::Responses => "responses",
    }
}

pub(crate) fn build_search_provider_request_plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
) -> Result<RequestPlan, GatewayError> {
    match req.endpoint_kind {
        EndpointKind::Search => {
            let path = require_search_api_path(payload, EndpointKind::Search)?;
            let query_field = payload.search_query_field.as_deref().unwrap_or("q");
            Ok(RequestPlan {
                method: Method::POST,
                url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
                query: Vec::new(),
                body: Some(search_api::pack_search(req, query_field)),
                response_kind: EndpointKind::Search,
            })
        }
        EndpointKind::Fetch => {
            let path = require_search_api_path(payload, EndpointKind::Fetch)?;
            let urls_field = payload.fetch_urls_field.as_deref().unwrap_or("url");
            Ok(RequestPlan {
                method: Method::POST,
                url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
                query: Vec::new(),
                body: Some(search_api::pack_fetch(req, urls_field)),
                response_kind: EndpointKind::Fetch,
            })
        }
        EndpointKind::ResearchCreate => {
            let path = require_search_api_path(payload, EndpointKind::ResearchCreate)?;
            Ok(RequestPlan {
                method: Method::POST,
                url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
                query: Vec::new(),
                body: Some(search_api::pack_search_api(req)),
                response_kind: EndpointKind::ResearchCreate,
            })
        }
        EndpointKind::ResearchList => {
            let path = require_search_api_path(payload, EndpointKind::ResearchList)?;
            Ok(RequestPlan {
                method: Method::GET,
                url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
                query: search_api::extract_search_api_query(req),
                body: None,
                response_kind: EndpointKind::ResearchList,
            })
        }
        EndpointKind::ResearchGet => {
            let path = require_search_api_path(payload, EndpointKind::ResearchGet)?;
            Ok(RequestPlan {
                method: Method::GET,
                url: format!(
                    "{}{}/{}",
                    payload.base_url.trim_end_matches('/'),
                    path,
                    search_api::research_id(req).unwrap_or_default()
                ),
                query: Vec::new(),
                body: None,
                response_kind: EndpointKind::ResearchGet,
            })
        }
        EndpointKind::CreditsBalance => {
            let path = require_search_api_path(payload, EndpointKind::CreditsBalance)?;
            Ok(RequestPlan {
                method: Method::GET,
                url: format!("{}{}", payload.base_url.trim_end_matches('/'), path),
                query: Vec::new(),
                body: None,
                response_kind: EndpointKind::CreditsBalance,
            })
        }
        _ => Err(GatewayError::bad_request(
            "Search-provider adapters only support search, fetch, research, and balance endpoints",
        )
        .with_code("unsupported_search_provider_endpoint")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::canonical::{CanonicalMessage, ContentPart, MessageRole, ProtocolFamily};
    use serde_json::json;
    use std::collections::HashMap;

    fn make_payload(adapter: &str, base_url: &str) -> ProviderAccountPayload {
        ProviderAccountPayload {
            discovered_protocols: Vec::new(),
            adapter: adapter.to_string(),
            base_url: base_url.to_string(),
            api_key: "sk-test".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers: HashMap::new(),
            auth_mode: None,
            anthropic_version: None,
            beta_headers: None,
            auth_header_name: None,
            auth_token: None,
            responses_path: None,
            chat_completions_path: None,
            completions_path: None,
            embeddings_path: None,
            audio_transcriptions_path: None,
            audio_speech_path: None,
            messages_path: None,
            search_path: None,
            fetch_path: None,
            research_path: None,
            balance_path: None,
            search_query_field: None,
            fetch_urls_field: None,
            extra_body: None,
            session_auth: None,
            keepalive: None,
        }
    }

    fn make_request(protocol: ProtocolFamily, endpoint: EndpointKind) -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: protocol,
            endpoint_kind: endpoint,
            requested_model: Some("gpt-4o".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "Hello".to_string(),
                }],
                name: None,
                tool_call_id: None,
                tool_calls: vec![],
            }],
            tools: vec![],
            tool_choice: None,
            reasoning: None,
            metadata: None,
            raw_body: json!({}),
            previous_response_id: None,
            explicit_session_key: None,
            extra: HashMap::new(),
        }
    }

    fn assert_search_provider_url(plan: &RequestPlan, expected_url: &str) {
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, expected_url);
        assert_eq!(plan.response_kind, EndpointKind::Search);
    }

    #[test]
    fn search_endpoint_label_covers_research_and_balance_routes() {
        assert_eq!(
            search_endpoint_label(EndpointKind::ResearchCreate),
            "research"
        );
        assert_eq!(
            search_endpoint_label(EndpointKind::CreditsBalance),
            "credits balance"
        );
    }

    #[test]
    fn require_search_api_path_reads_matching_surface_fields() {
        let mut payload = make_payload("search_api_compatible", "https://api.linkup.so");
        payload.search_path = Some("/v1/search".to_string());
        payload.fetch_path = Some("/v1/fetch".to_string());
        payload.research_path = Some("/v1/research".to_string());
        payload.balance_path = Some("/v1/balance".to_string());

        assert_eq!(
            require_search_api_path(&payload, EndpointKind::Search).ok(),
            Some("/v1/search")
        );
        assert_eq!(
            require_search_api_path(&payload, EndpointKind::Fetch).ok(),
            Some("/v1/fetch")
        );
        assert_eq!(
            require_search_api_path(&payload, EndpointKind::ResearchList).ok(),
            Some("/v1/research")
        );
        assert_eq!(
            require_search_api_path(&payload, EndpointKind::CreditsBalance).ok(),
            Some("/v1/balance")
        );
    }

    #[test]
    fn plan_search_api_compatible_url_and_body() {
        let mut payload = make_payload("search_api_compatible", "https://api.linkup.so");
        payload.search_path = Some("/v1/search".to_string());
        let mut req = make_request(ProtocolFamily::SearchApi, EndpointKind::Search);
        req.raw_body = json!({
            "model": "linkup-search",
            "q": "open source browser agents",
            "depth": "standard"
        });

        let plan = build_search_provider_request_plan(&payload, &req).unwrap();
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://api.linkup.so/v1/search");
        assert_eq!(plan.response_kind, EndpointKind::Search);
        assert_eq!(
            plan.body.as_ref().unwrap()["q"],
            "open source browser agents"
        );
        assert!(plan.body.as_ref().unwrap().get("model").is_none());
    }

    #[test]
    fn plan_search_api_fetch_uses_fetch_endpoint() {
        let mut payload = make_payload("search_api_compatible", "https://api.linkup.so");
        payload.fetch_path = Some("/v1/fetch".to_string());
        let mut req = make_request(ProtocolFamily::SearchApi, EndpointKind::Fetch);
        req.raw_body = json!({
            "model": "linkup-fetch",
            "url": "https://example.com/blog"
        });
        let plan = build_search_provider_request_plan(&payload, &req).unwrap();
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://api.linkup.so/v1/fetch");
        assert_eq!(
            plan.body.as_ref().unwrap()["url"],
            "https://example.com/blog"
        );
    }

    #[test]
    fn plan_exa_search_rewrites_query_field() {
        let mut payload = make_payload("search_api_compatible", "https://api.exa.ai");
        payload.search_path = Some("/search".to_string());
        payload.search_query_field = Some("query".to_string());
        let mut req = make_request(ProtocolFamily::SearchApi, EndpointKind::Search);
        req.raw_body = json!({
            "model": "exa-search",
            "q": "best rust crates for parsing markdown",
            "num_results": 5
        });

        let plan = build_search_provider_request_plan(&payload, &req).unwrap();
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://api.exa.ai/search");
        assert!(plan.body.as_ref().unwrap().get("q").is_none());
        assert_eq!(
            plan.body.as_ref().unwrap()["query"],
            "best rust crates for parsing markdown"
        );
        assert_eq!(plan.body.as_ref().unwrap()["num_results"], 5);
    }

    #[test]
    fn plan_exa_contents_rewrites_url_to_urls() {
        let mut payload = make_payload("search_api_compatible", "https://api.exa.ai");
        payload.fetch_path = Some("/contents".to_string());
        payload.fetch_urls_field = Some("urls".to_string());
        let mut req = make_request(ProtocolFamily::SearchApi, EndpointKind::Fetch);
        req.raw_body = json!({
            "model": "exa-fetch",
            "url": "https://example.com/blog",
            "text": true
        });

        let plan = build_search_provider_request_plan(&payload, &req).unwrap();
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://api.exa.ai/contents");
        assert!(plan.body.as_ref().unwrap().get("url").is_none());
        assert_eq!(
            plan.body.as_ref().unwrap()["urls"],
            json!(["https://example.com/blog"])
        );
        assert_eq!(plan.body.as_ref().unwrap()["text"], true);
    }

    #[test]
    fn plan_jina_reader_fetch_uses_root_path() {
        let mut payload = make_payload("search_api_compatible", "https://r.jina.ai");
        payload.fetch_path = Some("/".to_string());
        let mut req = make_request(ProtocolFamily::SearchApi, EndpointKind::Fetch);
        req.raw_body = json!({
            "model": "jina-fetch",
            "url": "https://example.com/blog"
        });

        let plan = build_search_provider_request_plan(&payload, &req).unwrap();
        assert_eq!(plan.method, Method::POST);
        assert_eq!(plan.url, "https://r.jina.ai/");
        assert_eq!(
            plan.body.as_ref().unwrap()["url"],
            "https://example.com/blog"
        );
    }

    #[test]
    fn plan_search_api_research_list_uses_get_and_query() {
        let mut payload = make_payload("search_api_compatible", "https://api.linkup.so");
        payload.research_path = Some("/v1/research".to_string());
        let mut req = make_request(ProtocolFamily::SearchApi, EndpointKind::ResearchList);
        req.raw_body = json!({
            "page": "2",
            "limit": "20"
        });
        let plan = build_search_provider_request_plan(&payload, &req).unwrap();
        assert_eq!(plan.method, Method::GET);
        assert_eq!(plan.url, "https://api.linkup.so/v1/research");
        assert_eq!(plan.query.len(), 2);
        assert!(plan.body.is_none());
    }

    #[test]
    fn plan_search_api_research_get_uses_id_path() {
        let mut payload = make_payload("search_api_compatible", "https://api.linkup.so");
        payload.research_path = Some("/v1/research".to_string());
        let mut req = make_request(ProtocolFamily::SearchApi, EndpointKind::ResearchGet);
        req.raw_body = json!({ "id": "research_123" });
        let plan = build_search_provider_request_plan(&payload, &req).unwrap();
        assert_eq!(plan.method, Method::GET);
        assert_eq!(plan.url, "https://api.linkup.so/v1/research/research_123");
        assert!(plan.body.is_none());
    }

    #[test]
    fn plan_search_api_balance_uses_get_endpoint() {
        let mut payload = make_payload("search_api_compatible", "https://api.linkup.so");
        payload.balance_path = Some("/v1/credits/balance".to_string());
        let req = make_request(ProtocolFamily::SearchApi, EndpointKind::CreditsBalance);
        let plan = build_search_provider_request_plan(&payload, &req).unwrap();
        assert_eq!(plan.method, Method::GET);
        assert_eq!(plan.url, "https://api.linkup.so/v1/credits/balance");
        assert!(plan.body.is_none());
    }

    #[test]
    fn legacy_search_adapter_alias_still_builds_search_plan() {
        let mut payload = make_payload("linkup_compatible", "https://api.linkup.so");
        payload.search_path = Some("/v1/search".to_string());
        let mut req = make_request(ProtocolFamily::SearchApi, EndpointKind::Search);
        req.raw_body = json!({ "q": "rust async" });

        let plan = build_search_provider_request_plan(&payload, &req)
            .expect("legacy search adapter alias should build a search plan");
        assert_search_provider_url(&plan, "https://api.linkup.so/v1/search");
    }

    #[test]
    fn search_only_provider_rejects_fetch_locally() {
        let mut payload = make_payload("search_api_compatible", "https://api.websearchapi.ai");
        payload.search_path = Some("/ai-search".to_string());
        let req = make_request(ProtocolFamily::SearchApi, EndpointKind::Fetch);
        let err = build_search_provider_request_plan(&payload, &req)
            .expect_err("fetch should be rejected without fetch_path");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_search_provider_endpoint")
        );
    }

    #[test]
    fn search_only_provider_rejects_research_locally() {
        let mut payload = make_payload("search_api_compatible", "https://api.tavily.com");
        payload.search_path = Some("/search".to_string());
        let req = make_request(ProtocolFamily::SearchApi, EndpointKind::ResearchCreate);
        let err = build_search_provider_request_plan(&payload, &req)
            .expect_err("research should be rejected without research_path");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_search_provider_endpoint")
        );
    }

    #[test]
    fn search_only_provider_rejects_balance_locally() {
        let mut payload = make_payload("search_api_compatible", "https://api.tavily.com");
        payload.search_path = Some("/search".to_string());
        let req = make_request(ProtocolFamily::SearchApi, EndpointKind::CreditsBalance);
        let err = build_search_provider_request_plan(&payload, &req)
            .expect_err("balance should be rejected without balance_path");
        assert_eq!(err.http_status, Some(400));
        assert_eq!(
            err.code.as_deref(),
            Some("unsupported_search_provider_endpoint")
        );
    }
}
