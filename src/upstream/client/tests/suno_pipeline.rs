//! Full public music -> original generation -> actual MP3 -> Local settlement/rollback.
#[path = "suno_pipeline/fixture.rs"]
mod fixture;
#[path = "../../suno/tests/fixture.rs"]
mod peer;
#[path = "suno_pipeline/remote.rs"]
mod remote;
use fixture::*;
use peer::{clip, Reply, Server};
use serde_json::{json, Value};

#[tokio::test]
async fn suno_pipeline_decoded_music_and_explicit_pending_settle_once_without_fake_usage() {
    Box::pin(async {
        for wait in [true,false] {
            let mut replies = vec![Reply::json(json!({"required":false})),Reply::json(json!({"clips":[clip("one",if wait {"complete"} else {"submitted"}),clip("two",if wait {"complete"} else {"queued"})]}))];
            if wait { replies.extend([Reply::audio(),Reply::audio()]); }
            let peer=Server::start(replies).await;
            let fixture=Box::pin(Fixture::new(&peer)).await;
            let response=crate::upstream::public_assets::within(&peer.origin,fixture.open(wait)).await;
            let status=response.status(); let text=drain(response.into_body()).await;
            assert_eq!(status,axum::http::StatusCode::OK,"{text}");
            let body:Value=serde_json::from_str(&text).unwrap();
            assert_eq!(body["object"],"music.generation"); assert_eq!(body["model"],NATIVE);
            assert_eq!(body["completed"],wait); assert_eq!(body["data"][0]["id"],"one");
            assert_eq!(body["data"][1]["id"],"two"); assert!(body.get("usage").is_none());
            assert!(!text.contains(SESSION)&&!text.contains("caller-secret"));
            let row=fixture.audit().await; assert_eq!(row.status,"completed"); identity(&row);
            assert_eq!(fixture.balance().await.remaining_messages,Some(1));
            wire(&peer,if wait {2} else {0});
            fixture.finish().await; peer.stop().await;
        }
    }).await;
}
#[tokio::test]
async fn suno_pipeline_second_asset_failure_refunds_instead_of_returning_partial_success() {
    Box::pin(async {
        let peer = Server::start(vec![
            Reply::json(json!({"required":false})),
            Reply::json(json!({"clips":[clip("one","complete"),clip("two","complete")]})),
            Reply::audio(),
            Reply::status(403, "private signed-token"),
        ])
        .await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        let response =
            crate::upstream::public_assets::within(&peer.origin, fixture.open(true)).await;
        assert!(!response.status().is_success());
        let text = drain(response.into_body()).await;
        assert!(
            text.contains("suno_asset_http")
                && !text.contains("private")
                && !text.contains(SESSION),
            "{text}"
        );
        let row = fixture.audit().await;
        assert_eq!(row.status, "failed");
        identity(&row);
        assert_eq!(row.route_trace.as_ref().unwrap()["candidateCount"], 2);
        assert_eq!(row.route_trace.as_ref().unwrap()["fallbackEligible"], false);
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        wire(&peer, 2);
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
#[tokio::test]
async fn suno_pipeline_drop_during_audio_read_is_neutral_and_refunds_original_reservation() {
    Box::pin(async {
        let mut audio=Reply::audio(); audio.hold=true;
        let peer=Server::start(vec![Reply::json(json!({"required":false})),Reply::json(json!({"clips":[clip("one","complete")]})),audio]).await;
        let fixture=Box::pin(Fixture::new(&peer)).await;
        let controller=fixture.state.concurrency_registry.get_or_create("suno-pipeline");
        let initial=controller.snapshot().current_limit;
        let mut call=Box::pin(crate::upstream::public_assets::within(&peer.origin,fixture.open(true)));
        tokio::select! { _=peer.wait_calls(3)=>{}, _=call.as_mut()=>panic!("unread asset must not already be successful") }
        assert_eq!(fixture.balance().await.remaining_messages,Some(1));
        assert_eq!(controller.snapshot().active_count,1); drop(call);
        let row=fixture.audit().await; assert_eq!(row.status,"cancelled"); identity(&row);
        assert_eq!(fixture.balance().await.remaining_messages,Some(2));
        let states=fixture.state.local_runtime.as_ref().unwrap().list_model_states(crate::db::CredentialModelStateFilters::default()).await.unwrap();
        assert!(states.iter().all(|s|s.failure_count==0),"{states:?}");
        assert_eq!(controller.snapshot().active_count,0); assert_eq!(controller.snapshot().current_limit,initial);
        peer.wait_closed().await; wire(&peer,1);
        fixture.finish().await; peer.stop().await;
    }).await;
}
