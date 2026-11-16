//! Normal public streaming keeps replacement observations separate from the final answer.
use super::*;
use crate::protocol::perplexity_web::public_snapshot::{EVENT, KEY};

#[test]
fn perplexity_web_public_schematized_completes_without_socket_eof() {
    run(schematized_completion);
}
async fn schematized_completion() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            for answer_only in [false, true] {
                let partial = json!({"status":"PENDING","final":true,"blocks":[{
                "intended_usage":"ask_text_0_markdown","diff_block":{"field":"markdown_block",
                "patches":[{"op":"replace","path":"","value":{"chunks":["temporary"]}}]}}]});
                let mut complete = json!({"status":"COMPLETED","blocks":[{
                "intended_usage":"ask_text_0_markdown","markdown_block":{"chunks":["final"],"answer":"final"}}],
                "read_write_token":"private-schematized-token"});
                if answer_only {
                    complete["blocks"][0]["markdown_block"]
                        .as_object_mut()
                        .unwrap()
                        .remove("chunks");
                }
                let server = wire::Server::start(
                    format!("data: {partial}\n\ndata: {complete}\n\n"),
                    200,
                    true,
                )
                .await;
                let fixture = Fixture::new(&server).await;
                let (status, bytes) = tokio::time::timeout(
                    Duration::from_secs(3),
                    fixture.request(&server, endpoint, body(endpoint, stream), true),
                )
                .await
                .unwrap();
                assert_eq!(status, StatusCode::OK);
                if stream {
                    let events = values(&bytes);
                    let text: String = events
                        .iter()
                        .filter_map(|value| {
                            if endpoint == "responses" {
                                (value["type"] == "response.output_text.delta")
                                    .then(|| value["delta"].as_str())
                                    .flatten()
                            } else {
                                value["choices"][0]["delta"]["content"].as_str()
                            }
                        })
                        .collect();
                    assert_eq!(text, "final");
                    assert!(events
                        .iter()
                        .any(|value| value["type"] == "response.completed"
                            || value["choices"][0]["finish_reason"] == "stop"));
                } else {
                    let value: Value = serde_json::from_slice(&bytes).unwrap();
                    let text = if endpoint == "responses" {
                        &value["output"][0]["content"][0]["text"]
                    } else {
                        &value["choices"][0]["message"]["content"]
                    };
                    assert_eq!(text, "final");
                }
                assert!(!String::from_utf8_lossy(&bytes).contains("private-schematized-token"));
                assert_eq!(fixture.audits().await[0].status, "completed");
                assert_wire(&server);
                wait_closed(&server).await;
                fixture.finish().await;
            }
        }
    }
}

fn snapshot(text: &str, url: &str) -> String {
    format!(
        "data: {}\n\n",
        json!({"blocks":[
        {"intended_usage":"ask_text","markdown_block":{"answer":text}},
        {"intended_usage":"web_results","web_result_block":{"web_results":[
            {"url":url,"name":"来源","snippet":"摘要"}]}}
    ],"read_write_token":"private-do-not-forward"})
    )
}
pub(super) fn values(bytes: &[u8]) -> Vec<Value> {
    std::str::from_utf8(bytes)
        .unwrap()
        .lines()
        .filter_map(|line| line.strip_prefix("data: "))
        .filter_map(|data| serde_json::from_str(data).ok())
        .collect()
}
async fn open(
    fixture: &Fixture,
    server: &wire::Server,
    endpoint: &str,
) -> axum::response::Response {
    let request = Request::post(format!("/v1/{endpoint}"))
        .header("authorization", "Bearer public-api-contract-secret")
        .header("content-type", "application/json")
        .body(Body::from(body(endpoint, true).to_string()))
        .unwrap();
    wire::within(
        &server.origin,
        build_router(fixture.console.state.clone()).oneshot(request),
    )
    .await
    .unwrap()
}
#[test]
fn perplexity_web_public_live_rewrites_clears_and_projects_only_final_answer_sources() {
    run(replacements);
}
async fn replacements() {
    for endpoint in ["chat/completions", "responses"] {
        let wire = format!(
            "{}{}data: {{}}\n\n{}event: end_of_stream\n\n",
            snapshot("旧临时答案", "https://example.invalid/old"),
            snapshot("改", "https://example.invalid/rewrite"),
            snapshot("最终", "https://example.invalid/final")
        );
        // Application end_of_stream, not socket EOF, is the product's established terminal.
        let server = wire::Server::start(wire, 200, true).await;
        let fixture = Fixture::new(&server).await;
        let (status, bytes) = fixture
            .request(&server, endpoint, body(endpoint, true), true)
            .await;
        assert_eq!(status, StatusCode::OK);
        let events = values(&bytes);
        let snapshots: Vec<_> = events
            .iter()
            .filter_map(|v| {
                v.get(KEY)
                    .or_else(|| (v["type"] == EVENT).then_some(&v["snapshot"]))
            })
            .collect();
        assert_eq!(snapshots.len(), 4, "{events:?}");
        for (i, text) in [Some("旧临时答案"), Some("改"), None, Some("最终")]
            .into_iter()
            .enumerate()
        {
            assert_eq!(snapshots[i]["sequence"], i + 1);
            assert_eq!(snapshots[i]["answer"]["text"].as_str(), text);
        }
        let text: String = events
            .iter()
            .filter_map(|v| {
                if endpoint == "responses" {
                    (v["type"] == "response.output_text.delta")
                        .then(|| v["delta"].as_str())
                        .flatten()
                } else {
                    v["choices"][0]["delta"]["content"].as_str()
                }
            })
            .collect();
        assert_eq!(text, "最终");
        let final_event = events
            .iter()
            .find_map(|v| {
                if endpoint == "responses" {
                    (v["type"] == "response.completed").then_some(&v["response"])
                } else {
                    (v["choices"][0]["finish_reason"] == "stop").then_some(v)
                }
            })
            .unwrap();
        assert_eq!(
            final_event["gateway_sources"][0]["url"],
            "https://example.invalid/final"
        );
        assert!(final_event["usage"].is_null());
        assert!(!String::from_utf8_lossy(&bytes).contains("private-do-not-forward"));
        assert_eq!(fixture.audits().await[0].status, "completed");
        assert_wire(&server);
        wait_closed(&server).await;
        fixture.finish().await;
    }
}
pub(super) async fn wait_closed(server: &wire::Server) {
    tokio::time::timeout(Duration::from_secs(3), async {
        while server.closed() == 0 {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
}
#[test]
fn perplexity_web_public_live_partial_then_failure_never_emits_standard_success_or_retries() {
    run(late_failures);
}
async fn late_failures() {
    for endpoint in ["chat/completions", "responses"] {
        for suffix in [
            "",
            "data: {bad}\n\n",
            "data: {}\n\nevent: end_of_stream\n\n",
            "data: {\"blocks\":[{\"diff_block\":{\"value\":\"private\"}}]}\n\n",
        ] {
            let server = wire::Server::start(
                format!(
                    "{}{suffix}",
                    snapshot("临时", "https://example.invalid/old")
                ),
                200,
                false,
            )
            .await;
            let fixture = Fixture::new(&server).await;
            let response = open(&fixture, &server, endpoint).await;
            assert_eq!(
                response.status(),
                StatusCode::OK,
                "Headers already handed off"
            );
            let mut body = response.into_body();
            let mut bytes = Vec::new();
            let mut failed = false;
            while let Some(frame) = body.frame().await {
                match frame {
                    Ok(frame) => {
                        if let Ok(data) = frame.into_data() {
                            bytes.extend_from_slice(&data);
                        }
                    }
                    Err(_) => {
                        failed = true;
                        break;
                    }
                }
            }
            drop(body);
            assert!(failed, "Post-header native failure must abort the body");
            let text = String::from_utf8(bytes).unwrap();
            assert!(
                text.contains("临时")
                    && !text.contains("[DONE]")
                    && !text.contains("response.completed")
            );
            assert!(!values(text.as_bytes())
                .iter()
                .any(|v| v["choices"][0]["delta"]["content"].is_string()
                    || v["type"] == "response.output_text.delta"));
            assert_eq!(fixture.audits().await[0].status, "failed");
            assert_wire(&server);
            fixture.finish().await;
        }
    }
}
#[test]
fn perplexity_web_public_live_snapshot_is_visible_before_terminal_and_drop_releases_permit() {
    run(early_drop);
}
async fn early_drop() {
    for endpoint in ["chat/completions", "responses"] {
        let server = wire::Server::start(
            snapshot("尚未完成", "https://example.invalid/partial"),
            200,
            true,
        )
        .await;
        let fixture = Fixture::new(&server).await;
        let controller = fixture
            .console
            .state
            .concurrency_registry
            .get_or_create("perplexity-pipeline");
        let limit = controller.snapshot().current_limit;
        let response = open(&fixture, &server, endpoint).await;
        let mut body = response.into_body();
        let bytes = tokio::time::timeout(Duration::from_secs(1), async {
            let mut bytes = Vec::new();
            loop {
                let frame = body.frame().await.unwrap().unwrap();
                if let Ok(data) = frame.into_data() {
                    bytes.extend_from_slice(&data);
                }
                if String::from_utf8_lossy(&bytes).contains("尚未完成") {
                    break bytes;
                }
            }
        })
        .await
        .unwrap();
        assert!(!String::from_utf8_lossy(&bytes).contains("[DONE]"));
        assert_eq!(controller.snapshot().active_count, 1);
        assert_eq!(server.closed(), 0);
        drop(body);
        assert_eq!(fixture.audits().await[0].status, "cancelled");
        wait_closed(&server).await;
        assert_wire(&server);
        let after = controller.snapshot();
        assert_eq!(after.active_count, 0);
        assert_eq!(after.current_limit, limit);
        assert_eq!(after.consecutive_successes, 0);
        fixture.finish().await;
    }
}
