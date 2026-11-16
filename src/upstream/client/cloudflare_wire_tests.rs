//! Keep official account identity until final validated transport, then use loopback only.
use super::*;
use crate::protocol::cloudflare_workers_ai as cf;
use axum::{extract::State, http::HeaderMap as AxumHeaders, Json, Router};
use std::sync::{Arc, Mutex};
tokio::task_local! {static ORIGIN:String;}
type Calls = Arc<Mutex<Vec<(String, AxumHeaders, Value)>>>;

pub(crate) fn transport_plan(
    payload: &ProviderAccountPayload,
    plan: &RequestPlan,
) -> Option<RequestPlan> {
    if !cf::owns_payload(payload) {
        return None;
    }
    cf::validate_plan(payload, plan).unwrap();
    Some(
        ORIGIN
            .try_with(|origin| RequestPlan {
                method: plan.method.clone(),
                url: format!(
                    "{origin}{}",
                    plan.url.strip_prefix("https://api.cloudflare.com").unwrap()
                ),
                query: plan.query.clone(),
                body: plan.body.clone(),
                response_kind: plan.response_kind,
            })
            .expect("Cloudflare tests must select loopback transport"),
    )
}
fn payload() -> ProviderAccountPayload {
    let preset = crate::preset::get_builtin_preset(cf::PRESET).unwrap();
    let account = serde_json::from_value(json!({"base_url":format!("https://api.cloudflare.com/client/v4/accounts/{}/ai/v1", "a".repeat(32)),"api_key":"selected-cf-token"})).unwrap();
    let mut payload = crate::preset::compile_provider_account(&preset, &account);
    payload.call_flow = Some(
        serde_json::from_value(
            json!({"call_adapter":"auto","fingerprint_adapter":"auto","special_adapter":"auto"}),
        )
        .unwrap(),
    );
    payload
}
async fn reply(
    State(calls): State<Calls>,
    uri: axum::http::Uri,
    headers: AxumHeaders,
    Json(body): Json<Value>,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    calls
        .lock()
        .unwrap()
        .push((uri.path().into(), headers, body.clone()));
    if body["stream"] == true {
        return ([("content-type", "text/event-stream")], "data: {\"choices\":[{\"index\":0,\"delta\":{\"content\":\"OK\"}}]}\n\ndata: [DONE]\n\n").into_response();
    }
    if uri.path().ends_with("/responses") {
        Json(json!({"id":"resp_cf","object":"response","model":"@cf/openai/gpt-oss-20b","status":"completed","output":[{"type":"message","role":"assistant","content":[{"type":"output_text","text":"OK"}]}]})).into_response()
    } else if uri.path().ends_with("/embeddings") {
        Json(json!({"object":"list","data":[{"object":"embedding","index":0,"embedding":[0.1,0.2]}],"model":"@cf/baai/bge-contract"})).into_response()
    } else {
        Json(json!({"model":"@cf/meta/llama-contract","choices":[{"message":{"role":"assistant","content":"OK"},"finish_reason":"stop"}]})).into_response()
    }
}
#[tokio::test]
async fn cloudflare_new_flow_chat_sse_responses_embeddings_use_selected_account_token() {
    let calls: Calls = Arc::new(Mutex::new(Vec::new()));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let app = Router::new().fallback(reply).with_state(calls.clone());
    let server = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    let mut client = UpstreamClient::new(3);
    client.http = rquest::Client::builder().no_proxy().build().unwrap();
    let payload = payload();
    let extra = std::collections::HashMap::from([
        ("authorization".into(), "Bearer foreign".into()),
        ("cookie".into(), "foreign-cookie".into()),
        ("host".into(), "foreign.invalid".into()),
    ]);
    ORIGIN.scope(origin, async {
        for stream in [false, true] {
            let req = openai::normalize_chat_completions(json!({"model":"@cf/meta/llama-contract","messages":[{"role":"user","content":"hello"}],"stream":stream})).unwrap();
            if stream {
                let UpstreamStreamingResponse::Http(response) = client.execute_stream(&payload, &req, "@cf/meta/llama-contract", Some(&extra)).await.unwrap() else { panic!("HTTP required") };
                let output = response.text().await.unwrap(); assert!(output.contains("OK") && output.contains("[DONE]"));
            } else { assert_eq!(client.execute(&payload, &req, "@cf/meta/llama-contract", Some(&extra)).await.unwrap().text, "OK"); }
        }
        let req = responses::normalize_responses(json!({"model":"@cf/openai/gpt-oss-20b","input":"hello","stream":false})).unwrap();
        assert_eq!(client.execute(&payload, &req, "@cf/openai/gpt-oss-20b", None).await.unwrap().text, "OK");
        let mut req = openai::normalize_chat_completions(json!({"messages":[{"role":"user","content":"hello"}]})).unwrap();
        req.endpoint_kind = EndpointKind::Embeddings;
        req.raw_body = json!({"input":"hello"});
        let result = client.execute_json_passthrough("cf", ProviderExecutionMode::DirectHttp, &payload, &req, "@cf/baai/bge-contract", None).await.unwrap();
        assert!(result.to_string().contains("embedding"));
    }).await;
    let captured = calls.lock().unwrap().clone();
    assert_eq!(captured.len(), 4);
    for (index, (path, headers, body)) in captured.iter().enumerate() {
        assert_eq!(headers["authorization"], "Bearer selected-cf-token");
        assert!(!headers.contains_key("cookie"));
        assert_ne!(headers["host"], "foreign.invalid");
        assert!(path.ends_with(if index == 3 {
            "/embeddings"
        } else if index == 2 {
            "/responses"
        } else {
            "/chat/completions"
        }));
        assert!(!body.to_string().contains("selected-cf-token"));
    }
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}
#[test]
fn cloudflare_rejects_streaming_or_other_model_responses_and_late_destination_mutations() {
    let payload = payload();
    for (model, stream) in [
        ("@cf/meta/llama-contract", false),
        ("@cf/openai/gpt-oss-20b", true),
    ] {
        let req =
            responses::normalize_responses(json!({"model":model,"input":"hello","stream":stream}))
                .unwrap();
        assert!(UpstreamClient::build_request_plan(&payload, &req, model, stream).is_err());
    }
    let req = responses::normalize_responses(json!({"input":"hello"})).unwrap();
    let client = UpstreamClient::new(1);
    for case in 0..3 {
        let mut plan =
            UpstreamClient::build_request_plan(&payload, &req, "@cf/openai/gpt-oss-20b", false)
                .unwrap();
        match case {
            0 => plan.url = plan.url.replace(&"a".repeat(32), &"b".repeat(32)),
            1 => plan.query.push(("leak".into(), "x".into())),
            _ => plan.body.as_mut().unwrap()["stream"] = json!(true),
        }
        assert!(client
            .send_profiled_plan(&payload, &plan, HeaderMap::new())
            .is_err());
    }
}
