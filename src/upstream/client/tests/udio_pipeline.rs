//! Public music metadata must use exact identity and normal settlement, not fake token usage.
#[path = "udio_pipeline/fixture.rs"]
mod fixture;
#[path = "udio_pipeline/peer.rs"]
mod peer;
use fixture::*;
use peer::Peer;
use serde_json::{json, Value};

fn complete(id: &str) -> Value {
    json!({"id":id,"status":"complete","finished":true,"song_path":"https://asset.invalid/not-audio"})
}
fn envelope(result: Value) -> Value {
    json!({"ok":true,"result":result})
}

#[tokio::test]
async fn udio_pipeline_completed_and_pending_metadata_settle_once_without_fake_usage() {
    Box::pin(async {
        for wait in [true,false] {
            let result=if wait {json!({"trackIds":["one","two"],"songs":[complete("two"),complete("one")],"completed":true})}
                else {json!({"trackIds":["one","two"],"songs":[{"id":"two","status":"queued"}],"completed":false})};
            let peer=Peer::start(envelope(result),200,false).await;
            let fixture=Box::pin(Fixture::new(&peer)).await;
            let response=fixture.open(wait).await;
            let status=response.status();
            let text=drain(response.into_body()).await;
            assert_eq!(status,axum::http::StatusCode::OK,"{text}");
            let body:Value=serde_json::from_str(&text).unwrap();
            assert_eq!(body["object"],"music.generation");
            assert_eq!(body["model"],NATIVE_MODEL);
            assert_eq!(body["completed"],wait);
            assert_eq!(body["data"][0]["id"],"one");
            assert_eq!(body["data"][1]["id"],"two");
            assert!(!text.contains(SESSION) && !text.contains("caller-secret"));
            assert!(body.get("usage").is_none());
            let row=fixture.audit().await;
            assert_eq!(row.status,"completed");
            identity(&row);
            assert_eq!(fixture.balance().await.remaining_messages,Some(1));
            wire(&peer);
            fixture.finish().await;
            peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn udio_pipeline_model_override_rejected_before_executor_and_refunds_without_fallback() {
    Box::pin(async {
        for nested in [false, true] {
            let peer = Peer::start(Value::Null, 200, false).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            let mut body = json!({"model":MODEL,"prompt":"fixture","wait_audio":true});
            if nested {
                body["gen_params"] = json!({"model_type":"foreign-private-model"});
            } else {
                body["modelType"] = json!("foreign-private-model");
            }
            let response = fixture.open_body(body).await;
            assert_eq!(response.status(), axum::http::StatusCode::BAD_REQUEST);
            let text = drain(response.into_body()).await;
            assert!(
                text.contains("udio_model_binding_invalid")
                    && !text.contains("foreign-private-model")
            );
            assert!(
                peer.calls().is_empty(),
                "invalid model must not submit, switch account or fallback"
            );
            let row = fixture.audit().await;
            assert_eq!(row.status, "failed");
            identity(&row);
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
#[tokio::test]
async fn udio_pipeline_uncertain_or_invalid_result_refunds_without_retry_or_fallback() {
    Box::pin(async {
        for (status,result) in [(503,json!({"private":"private-result"})),
            (200,envelope(json!({"trackIds":["one","two"],"songs":[complete("one")],"completed":true}))),
            (200,envelope(json!({"trackIds":["one"],"songs":[],"completed":false}))),
            (200,envelope(json!({"trackIds":["one"],"songs":[complete("private-result")],"completed":true})))] {
            let peer=Peer::start(result,status,false).await;
            let fixture=Box::pin(Fixture::new(&peer)).await;
            let response=fixture.open(true).await;
            assert!(!response.status().is_success());
            let text=drain(response.into_body()).await;
            assert!(!text.contains(SESSION) && !text.contains("private-result"),"{text}");
            let row=fixture.audit().await;
            assert_eq!(row.status,"failed");
            identity(&row);
            assert_eq!(row.route_trace.as_ref().unwrap()["candidateCount"],2);
            assert_eq!(row.route_trace.as_ref().unwrap()["fallbackEligible"],false);
            assert_eq!(fixture.balance().await.remaining_messages,Some(2));
            wire(&peer);
            fixture.finish().await;
            peer.stop().await;
        }
    }).await;
}
#[tokio::test]
async fn udio_pipeline_caller_drop_is_neutral_releases_source_and_refunds_local_reservation() {
    Box::pin(async {
        let peer = Peer::start(Value::Null, 200, true).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let controller = fixture
            .state
            .concurrency_registry
            .get_or_create("udio-pipeline");
        let initial = controller.snapshot().current_limit;
        let mut call = Box::pin(fixture.open(true));
        tokio::select! {
            _ = peer.wait_called() => {},
            _ = call.as_mut() => panic!("held remote response must not complete before cancellation"),
        }
        wire(&peer);
        assert_eq!(fixture.balance().await.remaining_messages, Some(1));
        drop(call);
        let row = fixture.audit().await;
        assert_eq!(row.status, "cancelled");
        identity(&row);
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        let states = fixture
            .state
            .local_runtime
            .as_ref()
            .unwrap()
            .list_model_states(crate::db::CredentialModelStateFilters::default())
            .await
            .unwrap();
        assert!(states.iter().all(|s| s.failure_count == 0), "{states:?}");
        assert_eq!(controller.snapshot().active_count, 0);
        assert_eq!(controller.snapshot().current_limit, initial);
        peer.wait_closed().await;
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
