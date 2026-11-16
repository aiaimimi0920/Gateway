//! Real public authorization and unsupported media input fail before any upstream I/O.
use super::{pipeline_fixture::*, test_support::*};
use axum::http::StatusCode;
use serde_json::{json, Value};

#[tokio::test]
async fn luma_agents_pipeline_auth_models_modalities_and_parameters_reject_before_network() {
    Box::pin(async {
        for (endpoint,input,invalid_token) in [
            ("images/generations",body(IMAGE),true),
            ("images/generations",json!({"model":"not-authorized","prompt":"合成"}),false),
            ("images/generations",json!({"model":IMAGE,"prompt":"合成","n":2}),false),
            ("images/generations",json!({"model":IMAGE,"prompt":"合成","stream":true}),false),
            ("images/generations",body(VIDEO),false),
            ("videos/generations",body(IMAGE),false),
            ("chat/completions",json!({"model":IMAGE,"messages":[{"role":"user","content":"合成"}]}),false),
            ("images/edits",json!({"model":IMAGE,"prompt":"合成","source":{"url":"http://private.example/a"}}),false),
            ("images/edits",json!({"model":IMAGE,"prompt":"合成","source":{"generation_id":"../other"}}),false),
            ("images/edits",json!({"model":IMAGE,"prompt":"合成","source":{"file_id":ID},"images":[{"mime_type":"image/png","base64":"YQ=="}]}),false),
            ("images/edits",json!({"model":IMAGE,"prompt":"合成","images":[{"mime_type":"image/png","base64":"YQ==","base64_data":"Yg=="}]}),false),
        ] {
            eprintln!("Luma public rejection: {endpoint} invalid_token={invalid_token} input={input}");
            let server=Server::start(vec![]).await;
            let mut fixture=Box::pin(Fixture::new(&server,5)).await;
            if invalid_token {fixture.token="invalid-public-key".into();}
            within(&server.origin,async {
                let response=fixture.open(endpoint,input).await;
                assert!(!response.status().is_success(),"{endpoint}");
                let _=drain(response.into_body()).await;
                assert!(server.calls().is_empty());
                assert_eq!(fixture.balance().await.remaining_messages,Some(2));
            }).await;
            fixture.finish().await;
            server.stop().await;
        }
    }).await;
}

#[test]
fn luma_agents_generic_text_plans_are_rejected_instead_of_sending_the_api_key() {
    let request = crate::protocol::openai::normalize_chat_completions(
        json!({"model":"uni-1","messages":[{"role":"user","content":"synthetic"}]}),
    )
    .unwrap();
    for stream in [false, true] {
        let error = crate::upstream::client::UpstreamClient::build_request_plan(
            &payload(),
            &request,
            "uni-1",
            stream,
        )
        .err()
        .expect("media products must not enter the generic text planner");
        assert_eq!(error.code.as_deref(), Some("luma_agents_request"));
        assert!(!error.retryable);
    }
}
#[tokio::test]
async fn luma_agents_pipeline_unknown_token_usage_retains_existing_reservation_not_credits_conversion(
) {
    Box::pin(async {
        let mut completed = job("completed");
        completed["usage"]["credits_charged"] = json!(0.25);
        let server = Server::start(vec![Reply::json(201, completed), Reply::png()]).await;
        let fixture = Box::pin(Fixture::new(&server, 5)).await;
        fixture
            .adjust(json!({"balanceMode":"token_prepaid","totalTokens":100,"remainingTokens":100}))
            .await;
        within(&server.origin, async {
            let response = fixture.open("images/generations", body(IMAGE)).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{text}");
            let row = fixture.audit().await;
            assert_eq!(row.total_tokens, None);
            assert_eq!(
                row.route_trace.as_ref().unwrap()["lumaAgentsGeneration"]["usage"]
                    ["credits_charged"],
                json!(0.25)
            );
            let estimate =
                (crate::redis::usage_tracking::estimate_token_count("合成媒体测试") as f64 * 1.2)
                    .ceil() as i64;
            let balance = fixture.balance().await;
            assert_eq!(balance.total_tokens, Some(100));
            assert_eq!(balance.remaining_tokens, Some(100 - estimate));
            assert_eq!(server.calls().len(), 2);
            assert!(serde_json::from_str::<Value>(&text)
                .unwrap()
                .get("usage")
                .is_none());
        })
        .await;
        fixture.finish().await;
        server.stop().await;
    })
    .await;
}
