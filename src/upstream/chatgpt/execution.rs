use std::collections::HashMap;
use std::time::Duration;

use rquest::{Client, Method};
use serde_json::{json, Value};
use tracing::debug;

use crate::error::{classify_network_error, GatewayError};
use crate::protocol::canonical::{CanonicalRelayRequest, CanonicalRelayResponse};
use crate::protocol::chatgpt::web_reverse as surface;
use crate::routing::candidate::ProviderAccountPayload;
use crate::upstream::response_types::UpstreamStreamingResponse;

use super::bootstrap::bootstrap_site;
use super::common::{
    classify_chatgpt_web_text_response, insert_header_map_value, is_event_stream_content_type,
};
use super::headers::build_request_headers;
use super::web_reverse::{
    build_legacy_requirements_token, build_proof_token, build_request_context, build_target_url,
    solve_turnstile_token, translate_chatgpt_web_stream, ChatGptWebRequestContext,
};
use super::PROVIDER;

pub async fn execute(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<CanonicalRelayResponse, GatewayError> {
    let request_context = build_request_context(payload);
    let cached_requirements = cached_f_conversation_requirements(payload);
    let using_cached_f_conversation_material = cached_requirements.is_some();
    let requirements = if let Some(requirements) = cached_requirements {
        requirements
    } else {
        let bootstrap =
            bootstrap_site(http, timeout, payload, &request_context, extra_headers).await?;
        get_requirements(
            http,
            timeout,
            payload,
            &request_context,
            &bootstrap,
            extra_headers,
        )
        .await?
    };
    let timezone = extra_body_string(payload, &["timezone"])
        .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string());
    let body = surface::pack_request(req, model, &timezone)?;
    let turn_trace_id = cached_f_conversation_turn_trace_id(payload)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let conversation_path = conversation_path(payload, using_cached_f_conversation_material);
    if should_post_prepare(payload, using_cached_f_conversation_material) {
        if let Some(prepare_path) = conversation_prepare_path(payload, &conversation_path) {
            post_prepare(
                http,
                timeout,
                payload,
                &request_context,
                extra_headers,
                &prepare_path,
                &turn_trace_id,
            )
            .await?;
        }
    }
    let response = post_json(
        http,
        timeout,
        payload,
        &request_context,
        extra_headers,
        &conversation_path,
        &body,
        Some(&requirements),
        true,
        Some(&turn_trace_id),
    )
    .await?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)?;

    surface::accumulate_response(&body_text, model).map_err(|error| error.with_provider(PROVIDER))
}

pub async fn execute_stream(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<UpstreamStreamingResponse, GatewayError> {
    let request_context = build_request_context(payload);
    let cached_requirements = cached_f_conversation_requirements(payload);
    let using_cached_f_conversation_material = cached_requirements.is_some();
    let requirements = if let Some(requirements) = cached_requirements {
        requirements
    } else {
        let bootstrap =
            bootstrap_site(http, timeout, payload, &request_context, extra_headers).await?;
        get_requirements(
            http,
            timeout,
            payload,
            &request_context,
            &bootstrap,
            extra_headers,
        )
        .await?
    };
    let timezone = extra_body_string(payload, &["timezone"])
        .unwrap_or_else(|| surface::CHATGPT_WEB_DEFAULT_TIMEZONE.to_string());
    let body = surface::pack_request(req, model, &timezone)?;
    let turn_trace_id = cached_f_conversation_turn_trace_id(payload)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let conversation_path = conversation_path(payload, using_cached_f_conversation_material);
    if should_post_prepare(payload, using_cached_f_conversation_material) {
        if let Some(prepare_path) = conversation_prepare_path(payload, &conversation_path) {
            post_prepare(
                http,
                timeout,
                payload,
                &request_context,
                extra_headers,
                &prepare_path,
                &turn_trace_id,
            )
            .await?;
        }
    }
    let response = post_json(
        http,
        timeout,
        payload,
        &request_context,
        extra_headers,
        &conversation_path,
        &body,
        Some(&requirements),
        true,
        Some(&turn_trace_id),
    )
    .await?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    if !(200..300).contains(&status) {
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
        classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)?;
        unreachable!("non-success ChatGPT web reverse response unexpectedly passed validation");
    }
    if !is_event_stream_content_type(content_type.as_deref()) {
        let body_text = response
            .text()
            .await
            .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
        classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)?;
        unreachable!("non-SSE ChatGPT web reverse response unexpectedly passed validation");
    }
    Ok(UpstreamStreamingResponse::Bytes(Box::pin(
        translate_chatgpt_web_stream(response.bytes_stream(), model.to_string()),
    )))
}

async fn get_requirements(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request_context: &ChatGptWebRequestContext,
    bootstrap: &surface::ChatGptWebBootstrap,
    extra_headers: Option<&HashMap<String, String>>,
) -> Result<surface::ChatRequirements, GatewayError> {
    let user_agent = request_context.user_agent.clone();
    let legacy_token = build_legacy_requirements_token(bootstrap, &user_agent);
    let request_body = json!({ "p": legacy_token });
    let response = post_json(
        http,
        timeout,
        payload,
        request_context,
        extra_headers,
        surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH,
        &request_body,
        None,
        false,
        None,
    )
    .await?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)?;

    let value: Value = serde_json::from_str(&body_text).map_err(|error| {
        GatewayError::server_error(format!(
            "ChatGPT Web reverse failed to decode chat requirements JSON: {error}"
        ))
        .with_provider(PROVIDER)
        .with_code("chatgpt_web_requirements_decode_failed")
    })?;
    let Some(object) = value.as_object() else {
        return Err(GatewayError::server_error(
            "ChatGPT Web reverse chat requirements payload was not a JSON object.",
        )
        .with_provider(PROVIDER)
        .with_code("chatgpt_web_requirements_invalid_shape"));
    };

    if object
        .get("arkose")
        .and_then(Value::as_object)
        .and_then(|record| record.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web reverse upstream requested Arkose; this route currently requires a browser-backed refresh or manual challenge clearance.",
        )
        .with_provider(PROVIDER)
        .with_code(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE));
    }

    let mut proof_token = None;
    if object
        .get("proofofwork")
        .and_then(Value::as_object)
        .and_then(|record| record.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        let proof = object
            .get("proofofwork")
            .and_then(Value::as_object)
            .unwrap();
        let seed = proof
            .get("seed")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let difficulty = proof
            .get("difficulty")
            .and_then(Value::as_str)
            .unwrap_or("0fffff");
        proof_token = Some(build_proof_token(bootstrap, &user_agent, seed, difficulty)?);
    }

    let turnstile = object.get("turnstile").and_then(Value::as_object);
    let turnstile_required = turnstile
        .and_then(|record| record.get("required"))
        .and_then(Value::as_bool)
        .unwrap_or(false);
    let turnstile_dx = turnstile
        .and_then(|record| record.get("dx"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let turnstile_token = turnstile
        .and_then(|record| record.get("token"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .or_else(|| {
            turnstile_dx.and_then(|dx| {
                solve_turnstile_token(dx, "").or_else(|| solve_turnstile_token(dx, &legacy_token))
            })
        });
    debug!(
        turnstile_required,
        turnstile_has_dx = turnstile_dx.is_some(),
        turnstile_dx_len = turnstile_dx.map(str::len).unwrap_or_default(),
        turnstile_solved = turnstile_token.is_some(),
        "chatgpt web turnstile requirements"
    );

    if turnstile_required && turnstile_token.is_none() {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web reverse upstream requested Turnstile and the browserless dx solver did not produce a token; this route currently requires a browser-backed refresh or manual challenge clearance.",
        )
        .with_provider(PROVIDER)
        .with_code(surface::CHATGPT_WEB_BROWSER_CHALLENGE_REQUIRED_CODE));
    }

    let token = object
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            GatewayError::server_error(
                "ChatGPT Web reverse chat requirements payload did not include token.",
            )
            .with_provider(PROVIDER)
            .with_code("chatgpt_web_requirements_missing_token")
        })?;

    Ok(surface::ChatRequirements {
        token: token.to_string(),
        proof_token,
        turnstile_token,
        so_token: object
            .get("so_token")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string),
    })
}

async fn post_json(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request_context: &ChatGptWebRequestContext,
    extra_headers: Option<&HashMap<String, String>>,
    path: &str,
    body: &Value,
    requirements: Option<&surface::ChatRequirements>,
    stream: bool,
    turn_trace_id: Option<&str>,
) -> Result<rquest::Response, GatewayError> {
    let url = build_target_url(request_context, path);
    let accept = if stream {
        "text/event-stream"
    } else {
        "application/json"
    };
    let mut headers = build_request_headers(
        request_context,
        payload,
        extra_headers,
        path,
        accept,
        Some("application/json"),
    );
    strip_request_headers_for_path(&mut headers, path);
    if let Some(requirements) = requirements {
        insert_header_map_value(
            &mut headers,
            "OpenAI-Sentinel-Chat-Requirements-Token",
            &requirements.token,
        );
        if let Some(proof_token) = requirements.proof_token.as_deref() {
            insert_header_map_value(&mut headers, "OpenAI-Sentinel-Proof-Token", proof_token);
        }
        if let Some(turnstile_token) = requirements.turnstile_token.as_deref() {
            insert_header_map_value(
                &mut headers,
                "OpenAI-Sentinel-Turnstile-Token",
                turnstile_token,
            );
        }
        if let Some(so_token) = requirements.so_token.as_deref() {
            insert_header_map_value(&mut headers, "OpenAI-Sentinel-SO-Token", so_token);
        }
    }
    if let Some(turn_trace_id) = turn_trace_id {
        insert_header_map_value(&mut headers, "X-OAI-Turn-Trace-Id", turn_trace_id);
    }

    let response = http
        .request(Method::POST, &url)
        .headers(headers)
        .timeout(timeout.max(Duration::from_secs(if stream { 180 } else { 60 })))
        .json(body)
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    debug!(
        path = %path,
        status = response.status().as_u16(),
        content_type = ?response
            .headers()
            .get(rquest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok()),
        "chatgpt web post_json response"
    );
    Ok(response)
}

async fn post_prepare(
    http: &Client,
    timeout: Duration,
    payload: &ProviderAccountPayload,
    request_context: &ChatGptWebRequestContext,
    extra_headers: Option<&HashMap<String, String>>,
    path: &str,
    turn_trace_id: &str,
) -> Result<(), GatewayError> {
    let url = build_target_url(request_context, path);
    let mut headers = build_request_headers(
        request_context,
        payload,
        extra_headers,
        path,
        "application/json",
        Some("application/json"),
    );
    strip_request_headers_for_path(&mut headers, path);
    insert_header_map_value(&mut headers, "X-OAI-Turn-Trace-Id", turn_trace_id);
    let response = http
        .request(Method::POST, &url)
        .headers(headers)
        .timeout(timeout.max(Duration::from_secs(60)))
        .send()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    let status = response.status().as_u16();
    let content_type = response
        .headers()
        .get(rquest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string);
    let body_text = response
        .text()
        .await
        .map_err(|error| classify_network_error(&error, Some(PROVIDER)))?;
    debug!(
        path = %path,
        status,
        content_type = ?content_type,
        "chatgpt web prepare response"
    );
    classify_chatgpt_web_text_response(status, content_type.as_deref(), &body_text)
}

fn cached_f_conversation_requirements(
    payload: &ProviderAccountPayload,
) -> Option<surface::ChatRequirements> {
    let token = extra_body_string(
        payload,
        &[
            "chatgptWebSentinelChatRequirementsToken",
            "chatgptSentinelChatRequirementsToken",
            "openaiSentinelChatRequirementsToken",
        ],
    )?;
    Some(surface::ChatRequirements {
        token,
        proof_token: extra_body_string(
            payload,
            &[
                "chatgptWebSentinelProofToken",
                "chatgptSentinelProofToken",
                "openaiSentinelProofToken",
            ],
        ),
        turnstile_token: extra_body_string(
            payload,
            &[
                "chatgptWebSentinelTurnstileToken",
                "chatgptSentinelTurnstileToken",
                "openaiSentinelTurnstileToken",
            ],
        ),
        so_token: extra_body_string(
            payload,
            &[
                "chatgptWebSentinelSoToken",
                "chatgptSentinelSoToken",
                "openaiSentinelSoToken",
            ],
        ),
    })
}

fn cached_f_conversation_turn_trace_id(payload: &ProviderAccountPayload) -> Option<String> {
    extra_body_string(
        payload,
        &[
            "chatgptWebTurnTraceId",
            "chatgptTurnTraceId",
            "openaiTurnTraceId",
            "turnTraceId",
        ],
    )
    .or_else(|| {
        payload
            .headers
            .iter()
            .find(|(name, value)| {
                name.eq_ignore_ascii_case("x-oai-turn-trace-id") && !value.trim().is_empty()
            })
            .map(|(_, value)| value.trim().to_string())
    })
}

fn should_post_prepare(payload: &ProviderAccountPayload, using_cached_material: bool) -> bool {
    extra_body_bool(
        payload,
        &[
            "chatgptWebPrepareBeforeConversation",
            "chatgptPrepareBeforeConversation",
            "prepareBeforeConversation",
        ],
    )
    .unwrap_or(!using_cached_material)
}

fn conversation_path(payload: &ProviderAccountPayload, using_cached_material: bool) -> String {
    if let Some(path) = extra_body_string(
        payload,
        &[
            "conversationPath",
            "chatgptConversationPath",
            "chatgptWebConversationPath",
        ],
    ) {
        return path;
    }
    if !using_cached_material && dynamic_backend_conversation_enabled(payload) {
        return surface::CHATGPT_WEB_DEFAULT_CONVERSATION_PATH.to_string();
    }
    surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH.to_string()
}

fn dynamic_backend_conversation_enabled(payload: &ProviderAccountPayload) -> bool {
    if extra_body_bool(
        payload,
        &[
            "chatgptWebDynamicBackendConversation",
            "chatgptDynamicBackendConversation",
            "dynamicBackendConversation",
            "preferDynamicConversation",
        ],
    )
    .unwrap_or(false)
    {
        return true;
    }
    match extra_body_string(
        payload,
        &[
            "chatgptWebConversationMode",
            "chatgptConversationMode",
            "conversationMode",
        ],
    )
    .map(|value| value.trim().to_ascii_lowercase())
    .as_deref()
    {
        Some(
            "dynamic_backend_conversation"
            | "backend_conversation"
            | "browserless_dynamic_backend_conversation"
            | "chatgpt2api",
        ) => true,
        _ => false,
    }
}

fn conversation_prepare_path(
    payload: &ProviderAccountPayload,
    conversation_path: &str,
) -> Option<String> {
    if let Some(path) = extra_body_string(
        payload,
        &[
            "conversationPreparePath",
            "chatgptConversationPreparePath",
            "chatgptWebConversationPreparePath",
        ],
    ) {
        return Some(path);
    }
    (conversation_path == surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH)
        .then(|| surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PREPARE_PATH.to_string())
}

fn strip_request_headers_for_path(headers: &mut rquest::header::HeaderMap, path: &str) {
    if matches!(
        path,
        surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PATH
            | surface::CHATGPT_WEB_DEFAULT_F_CONVERSATION_PREPARE_PATH
    ) {
        for name in [
            "cookie",
            "origin",
            "cache-control",
            "pragma",
            "priority",
            "sec-fetch-dest",
            "sec-fetch-mode",
            "sec-fetch-site",
            "sec-ch-ua-arch",
            "sec-ch-ua-bitness",
            "sec-ch-ua-full-version",
            "sec-ch-ua-full-version-list",
            "sec-ch-ua-model",
            "sec-ch-ua-platform-version",
        ] {
            headers.remove(name);
        }
    }
    if !matches!(
        path,
        surface::CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH
            | surface::CHATGPT_WEB_DEFAULT_CONVERSATION_PATH
    ) {
        return;
    }
    for name in [
        "x-oai-is",
        "x-conduit-token",
        "oai-telemetry",
        "oai-echo-logs",
    ] {
        headers.remove(name);
    }
}

fn extra_body_bool(payload: &ProviderAccountPayload, keys: &[&str]) -> Option<bool> {
    let body = payload.extra_body.as_ref()?;
    for key in keys {
        let Some(value) = body.get(*key) else {
            continue;
        };
        if let Some(value) = value.as_bool() {
            return Some(value);
        }
        if let Some(value) = value.as_str() {
            match value.trim().to_ascii_lowercase().as_str() {
                "1" | "true" | "yes" | "on" | "enabled" => return Some(true),
                "0" | "false" | "no" | "off" | "disabled" | "never" => return Some(false),
                _ => {}
            }
        }
    }
    None
}

fn extra_body_string(payload: &ProviderAccountPayload, keys: &[&str]) -> Option<String> {
    let extra_body = payload.extra_body.as_ref()?;
    for key in keys {
        let value = extra_body
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .map(str::to_string);
        if value.is_some() {
            return value;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    use std::sync::{Arc, Mutex};

    use axum::{extract::State, http::HeaderMap as AxumHeaderMap, routing::post, Json, Router};
    use serde_json::json;
    use tokio::net::TcpListener;

    use crate::credential_runtime::SessionAuthConfig;
    use crate::protocol::canonical::{
        CanonicalMessage, ContentPart, EndpointKind, MessageRole, ProtocolFamily,
    };

    #[derive(Clone, Default)]
    struct RequestState {
        seen: Arc<Mutex<Vec<SeenRequest>>>,
    }

    #[derive(Clone, Debug)]
    struct SeenRequest {
        path: String,
        headers: HashMap<String, String>,
        body: String,
    }

    async fn record_requirements(
        State(state): State<RequestState>,
        headers: AxumHeaderMap,
        body: String,
    ) -> Json<Value> {
        record(
            &state,
            "/backend-api/sentinel/chat-requirements",
            headers,
            body,
        );
        Json(json!({
            "token": "requirements-token",
            "proofofwork": { "required": false },
            "turnstile": { "required": false, "token": "turnstile-token" }
        }))
    }

    async fn record_prepare(
        State(state): State<RequestState>,
        headers: AxumHeaderMap,
        body: String,
    ) -> Json<Value> {
        record(&state, "/backend-api/f/conversation/prepare", headers, body);
        Json(json!({ "ok": true }))
    }

    async fn record_conversation(
        State(state): State<RequestState>,
        headers: AxumHeaderMap,
        body: String,
    ) -> ([(&'static str, &'static str); 1], &'static str) {
        record(&state, "/backend-api/f/conversation", headers, body);
        (
            [("content-type", "text/event-stream")],
            concat!(
                "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
                "data: [DONE]\n\n"
            ),
        )
    }

    async fn record_dynamic_conversation(
        State(state): State<RequestState>,
        headers: AxumHeaderMap,
        body: String,
    ) -> ([(&'static str, &'static str); 1], &'static str) {
        record(&state, "/backend-api/conversation", headers, body);
        (
            [("content-type", "text/event-stream")],
            concat!(
                "data: {\"p\":\"/message/content/parts/0\",\"o\":\"append\",\"v\":\"Paris\"}\n\n",
                "data: [DONE]\n\n"
            ),
        )
    }

    fn record(state: &RequestState, path: &str, headers: AxumHeaderMap, body: String) {
        let headers = headers
            .iter()
            .filter_map(|(name, value)| {
                value
                    .to_str()
                    .ok()
                    .map(|value| (name.as_str().to_ascii_lowercase(), value.to_string()))
            })
            .collect::<HashMap<_, _>>();
        state.seen.lock().unwrap().push(SeenRequest {
            path: path.to_string(),
            headers,
            body,
        });
    }

    async fn spawn_chatgpt_f_conversation_server(state: RequestState) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
        let address = listener.local_addr().expect("addr");
        let app = Router::new()
            .route(
                "/backend-api/sentinel/chat-requirements",
                post(record_requirements),
            )
            .route("/backend-api/f/conversation/prepare", post(record_prepare))
            .route("/backend-api/f/conversation", post(record_conversation))
            .route(
                "/backend-api/conversation",
                post(record_dynamic_conversation),
            )
            .with_state(state);
        tokio::spawn(async move {
            axum::serve(listener, app).await.expect("serve");
        });
        format!("http://{}", address)
    }

    fn make_payload(base_url: &str) -> ProviderAccountPayload {
        let mut headers = HashMap::new();
        headers.insert("X-OAI-IS".to_string(), "is-material".to_string());
        headers.insert(
            "X-Conduit-Token".to_string(),
            "conduit-material".to_string(),
        );
        ProviderAccountPayload {
            adapter: "chatgpt_web_reverse_compatible".to_string(),
            base_url: base_url.to_string(),
            api_key: "session-token".to_string(),
            credential_id: None,
            expires_at: None,
            runtime_state_object_key: None,
            account_name: None,
            execution_mode: None,
            endpoint_execution_modes: None,
            default_model: None,
            headers,
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
            extra_body: Some(HashMap::from([
                (
                    "chatgptPowSources".to_string(),
                    json!(["https://chatgpt.com/backend-api/sentinel/sdk.js"]),
                ),
                ("chatgptPowDataBuild".to_string(), json!("prod-test-build")),
            ])),
            session_auth: Some(SessionAuthConfig {
                transport: "bearer".to_string(),
                primary_cookie_name: None,
                secondary_cookie_name: None,
                header_name: Some("authorization".to_string()),
                expires_at: None,
            }),
            keepalive: None,
        }
    }

    fn make_request() -> CanonicalRelayRequest {
        CanonicalRelayRequest {
            protocol_family: ProtocolFamily::OpenAi,
            endpoint_kind: EndpointKind::ChatCompletions,
            requested_model: Some("auto".to_string()),
            stream: false,
            messages: vec![CanonicalMessage {
                role: MessageRole::User,
                content: vec![ContentPart::Text {
                    text: "What is the capital of France?".to_string(),
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

    #[tokio::test]
    async fn execute_uses_browserless_f_conversation_prepare_contract() {
        let state = RequestState::default();
        let base_url = spawn_chatgpt_f_conversation_server(state.clone()).await;
        let payload = make_payload(&base_url);
        let response = execute(
            &Client::new(),
            Duration::from_secs(5),
            &payload,
            &make_request(),
            "auto",
            None,
        )
        .await
        .expect("f conversation response");

        assert_eq!(response.text, "Paris");

        let seen = state.seen.lock().unwrap().clone();
        assert_eq!(
            seen.iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/backend-api/sentinel/chat-requirements",
                "/backend-api/f/conversation/prepare",
                "/backend-api/f/conversation",
            ]
        );

        let prepare = &seen[1];
        let conversation = &seen[2];
        assert!(prepare.body.is_empty());
        assert_eq!(
            prepare
                .headers
                .get("x-openai-target-path")
                .map(String::as_str),
            Some("/backend-api/f/conversation/prepare")
        );
        assert_eq!(
            conversation
                .headers
                .get("x-openai-target-path")
                .map(String::as_str),
            Some("/backend-api/f/conversation")
        );

        let trace_id = prepare
            .headers
            .get("x-oai-turn-trace-id")
            .expect("prepare trace id");
        uuid::Uuid::parse_str(trace_id).expect("trace id uuid");
        assert_eq!(
            conversation
                .headers
                .get("x-oai-turn-trace-id")
                .map(String::as_str),
            Some(trace_id.as_str())
        );
        let requirements = &seen[0];
        assert!(!requirements.headers.contains_key("x-oai-is"));
        assert!(!requirements.headers.contains_key("x-conduit-token"));
        assert_eq!(
            conversation
                .headers
                .get("openai-sentinel-chat-requirements-token")
                .map(String::as_str),
            Some("requirements-token")
        );
        assert_eq!(
            conversation
                .headers
                .get("openai-sentinel-turnstile-token")
                .map(String::as_str),
            Some("turnstile-token")
        );
        assert_eq!(
            conversation.headers.get("x-oai-is").map(String::as_str),
            Some("is-material")
        );
        assert_eq!(
            conversation
                .headers
                .get("x-conduit-token")
                .map(String::as_str),
            Some("conduit-material")
        );

        let body: Value = serde_json::from_str(&conversation.body).expect("conversation body json");
        assert_eq!(body["client_prepare_state"], "success");
        assert_eq!(body["parent_message_id"], "client-created-root");
    }

    #[tokio::test]
    async fn execute_uses_cached_f_conversation_sentinel_material_without_preflight() {
        let state = RequestState::default();
        let base_url = spawn_chatgpt_f_conversation_server(state.clone()).await;
        let mut payload = make_payload(&base_url);
        let extra_body = payload.extra_body.as_mut().expect("extra body");
        extra_body.insert(
            "chatgptWebSentinelChatRequirementsToken".to_string(),
            json!("stored-requirements-token"),
        );
        extra_body.insert(
            "chatgptWebSentinelProofToken".to_string(),
            json!("stored-proof-token"),
        );
        extra_body.insert(
            "chatgptWebSentinelTurnstileToken".to_string(),
            json!("stored-turnstile-token"),
        );
        extra_body.insert(
            "chatgptWebTurnTraceId".to_string(),
            json!("11111111-1111-4111-8111-111111111111"),
        );
        payload
            .headers
            .insert("Cookie".to_string(), "session-cookie=present".to_string());

        let response = execute(
            &Client::new(),
            Duration::from_secs(5),
            &payload,
            &make_request(),
            "auto",
            None,
        )
        .await
        .expect("cached f conversation response");

        assert_eq!(response.text, "Paris");

        let seen = state.seen.lock().unwrap().clone();
        assert_eq!(
            seen.iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            vec!["/backend-api/f/conversation"]
        );

        let conversation = &seen[0];
        assert_eq!(
            conversation
                .headers
                .get("openai-sentinel-chat-requirements-token")
                .map(String::as_str),
            Some("stored-requirements-token")
        );
        assert_eq!(
            conversation
                .headers
                .get("openai-sentinel-proof-token")
                .map(String::as_str),
            Some("stored-proof-token")
        );
        assert_eq!(
            conversation
                .headers
                .get("openai-sentinel-turnstile-token")
                .map(String::as_str),
            Some("stored-turnstile-token")
        );
        assert_eq!(
            conversation
                .headers
                .get("x-oai-turn-trace-id")
                .map(String::as_str),
            Some("11111111-1111-4111-8111-111111111111")
        );
        assert!(!conversation.headers.contains_key("cookie"));
        assert!(!conversation.headers.contains_key("origin"));
        for name in [
            "cache-control",
            "pragma",
            "priority",
            "sec-fetch-dest",
            "sec-fetch-mode",
            "sec-fetch-site",
            "sec-ch-ua-arch",
            "sec-ch-ua-bitness",
            "sec-ch-ua-full-version",
            "sec-ch-ua-full-version-list",
            "sec-ch-ua-model",
            "sec-ch-ua-platform-version",
        ] {
            assert!(
                !conversation.headers.contains_key(name),
                "unexpected f-conversation header: {name}"
            );
        }
    }

    #[tokio::test]
    async fn execute_uses_dynamic_backend_conversation_when_mode_requests_it() {
        let state = RequestState::default();
        let base_url = spawn_chatgpt_f_conversation_server(state.clone()).await;
        let mut payload = make_payload(&base_url);
        let extra_body = payload.extra_body.as_mut().expect("extra body");
        extra_body.insert(
            "chatgptWebConversationMode".to_string(),
            json!("dynamic_backend_conversation"),
        );

        let response = execute(
            &Client::new(),
            Duration::from_secs(5),
            &payload,
            &make_request(),
            "auto",
            None,
        )
        .await
        .expect("dynamic backend conversation response");

        assert_eq!(response.text, "Paris");

        let seen = state.seen.lock().unwrap().clone();
        assert_eq!(
            seen.iter()
                .map(|item| item.path.as_str())
                .collect::<Vec<_>>(),
            vec![
                "/backend-api/sentinel/chat-requirements",
                "/backend-api/conversation",
            ]
        );

        let conversation = &seen[1];
        assert_eq!(
            conversation
                .headers
                .get("x-openai-target-path")
                .map(String::as_str),
            Some("/backend-api/conversation")
        );
        assert_eq!(
            conversation
                .headers
                .get("openai-sentinel-chat-requirements-token")
                .map(String::as_str),
            Some("requirements-token")
        );
        assert!(!conversation.headers.contains_key("x-oai-is"));
        assert!(!conversation.headers.contains_key("x-conduit-token"));
    }
}
