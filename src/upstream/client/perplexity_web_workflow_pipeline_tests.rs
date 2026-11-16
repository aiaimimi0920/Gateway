//! Public pipeline tests use a synthetic terminal, not account material or a live call.
use super::live::{values, wait_closed};
use super::*;

#[test]
fn perplexity_web_public_workflow_completes_without_socket_eof() {
    run(workflow_completion);
}

async fn workflow_completion() {
    for endpoint in ["chat/completions", "responses"] {
        for stream in [false, true] {
            let pending = json!({"status":"PENDING","final":true,"text_completed":true,
                "blocks":[{"intended_usage":"workflow_root","diff_block":{
                    "field":"workflow_block","patches":[{"op":"replace","path":"/status",
                    "value":"WORKFLOW_COMPLETED"}]}}]});
            let complete = json!({"status":"COMPLETED","final":true,"text_completed":true,
                "final_sse_message":true,"read_write_token":"synthetic-private-workflow",
                "blocks":[{"intended_usage":"workflow_root","workflow_block":{
                    "version":"1.0","status":"WORKFLOW_COMPLETED","snapshots":[],"steps":[{
                        "status":"WORKFLOW_COMPLETED","items":[{"type":"WORKFLOW_ITEM_TEXT",
                            "variant":"answer","payload":{"text_payload":{"variant":"answer",
                                "is_streaming":false,"text":"蓝色纸船","chunks":["蓝色纸船"]}}}]}]}}]});
            let server = wire::Server::start(
                format!("data: {pending}\n\ndata: {complete}\n\n"),
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
                assert_eq!(text, "蓝色纸船");
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
                assert_eq!(text, "蓝色纸船");
                assert!(value["usage"].is_null());
            }
            assert!(!String::from_utf8_lossy(&bytes).contains("synthetic-private-workflow"));
            let rows = fixture.audits().await;
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].status, "completed");
            assert!(rows[0].total_tokens.is_none());
            assert_eq!(
                rows[0].route_trace.as_ref().unwrap()["realCredentialRef"],
                "exact"
            );
            assert_wire(&server);
            wait_closed(&server).await;
            fixture.finish().await;
        }
    }
}
