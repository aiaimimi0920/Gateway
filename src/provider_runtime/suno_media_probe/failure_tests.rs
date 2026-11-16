//! Failed playback, challenge, original deadline and caller Drop cannot become a successful test.
use super::*;

#[tokio::test]
async fn suno_probe_asset_failure_challenge_and_incomplete_audio_never_replay_or_recover_health() {
    Box::pin(async {
        for scenario in [
            "html",
            "second-http",
            "truncated",
            "challenge",
            "incomplete",
        ] {
            let mut replies = completed();
            if scenario == "html" {
                replies[2] = Reply::status(200, "private signed-token");
                replies[2].mime = "audio/mpeg";
            }
            if scenario == "second-http" {
                replies[3] = Reply::status(403, "private signed-token");
            }
            if scenario == "truncated" {
                replies[2].bytes.pop();
            }
            if scenario == "challenge" {
                replies = vec![Reply::json(json!({"required":true}))];
            }
            if scenario == "incomplete" {
                replies = vec![
                    Reply::json(json!({"required":false})),
                    Reply::json(json!({"clips":[clip("one","error")]})),
                ];
            }
            let peer = Server::start(replies).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            let report = crate::upstream::public_assets::within(
                &peer.origin,
                run(&fixture, Some(&parsed(options()))),
            )
            .await;
            assert_eq!(
                report.status,
                ProviderPayloadProbeStatus::Failed,
                "{}",
                report.message
            );
            assert!(
                report.answer.is_none()
                    && !report.message.contains("private")
                    && !report.message.contains("track.mp3")
            );
            let row = fixture.audit().await;
            assert_eq!(row.status, "failed");
            closed_audit(&row, false);
            let states = fixture
                .state
                .local_runtime
                .as_ref()
                .unwrap()
                .list_model_states(Default::default())
                .await
                .unwrap();
            assert!(
                states.iter().all(|state| state.last_success_at.is_none()),
                "{states:?}"
            );
            assert_eq!(fixture.balance().await.remaining_messages, Some(2));
            assert_eq!(
                peer.calls().len(),
                match scenario {
                    "challenge" => 1,
                    "incomplete" => 2,
                    "second-http" => 4,
                    _ => 3,
                }
            );
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}

#[tokio::test]
async fn suno_probe_rejects_operation_parameters_unconfigured_models_and_legacy_flow_before_network(
) {
    Box::pin(async {
        let peer=Server::start(vec![]).await;let fixture=Box::pin(Fixture::new(&peer)).await;
        for change in [json!({"model":"chirp-v3-0"}),json!({"model":"not-implemented"}),
            json!({"media":{"operation":"image","confirmCharge":true}}),
            json!({"media":{"operation":"video","confirmCharge":true}}),
            json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"waitCompletion":false}}}),
            json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"continueClipId":"other"}}}),
            json!({"media":{"operation":"music","confirmCharge":true,"parameters":{"token":"private"}}})] {
            let mut input=options();input.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
            let report=run(&fixture,Some(&parsed(input))).await;
            assert_eq!(report.status,ProviderPayloadProbeStatus::Unsupported,"{change}: {}",report.message);
        }
        let mut target=fixture.state.route_config.snapshot().select_credential_probe_target("exact").unwrap();
        target.payload.call_flow=None;
        let (_,report)=Box::pin(probe_console_target_with_request(&fixture.state,&target,Some(&parsed(options())))).await;
        assert_eq!(report.status,ProviderPayloadProbeStatus::Unsupported);
        assert!(peer.calls().is_empty());assert_eq!(fixture.balance().await.remaining_messages,Some(2));
        fixture.finish().await;peer.stop().await;
    }).await;
}

#[tokio::test]
async fn suno_probe_drop_during_challenge_generation_or_audio_closes_stream_and_is_health_neutral()
{
    Box::pin(async {
        for stage in 0..3 {
            let mut replies=completed();replies[stage].hold=true;
            let peer=Server::start(replies).await;let fixture=Box::pin(Fixture::new(&peer)).await;
            let input=parsed(options());
            let mut call=Box::pin(crate::upstream::public_assets::within(&peer.origin,run(&fixture,Some(&input))));
            tokio::select! { _=peer.wait_calls(stage+1)=>{}, result=call.as_mut()=>panic!("held probe returned early: {result:?}") }
            drop(call);peer.wait_closed().await;
            let row=fixture.audit().await;assert_eq!(row.status,"cancelled");closed_audit(&row,false);
            let states=fixture.state.local_runtime.as_ref().unwrap().list_model_states(Default::default()).await.unwrap();
            assert!(states.iter().all(|row|row.failure_count==0),"{states:?}");
            assert_eq!(fixture.balance().await.remaining_messages,Some(2));assert_eq!(peer.calls().len(),stage+1);
            fixture.finish().await;peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn suno_probe_original_deadline_fails_closed_without_detached_generation_or_key_charge() {
    Box::pin(async {
        let mut replies = completed();
        replies[2].hold = true;
        let peer = Server::start(replies).await;
        let fixture = Box::pin(Fixture::with_client(&peer, |client| {
            client.timeout = std::time::Duration::from_secs(1)
        }))
        .await;
        let report = crate::upstream::public_assets::within(
            &peer.origin,
            run(&fixture, Some(&parsed(options()))),
        )
        .await;
        assert_eq!(report.status, ProviderPayloadProbeStatus::Failed);
        assert!(report.message.contains("504"));
        peer.wait_closed().await;
        let row = fixture.audit().await;
        assert_eq!(row.status, "failed");
        closed_audit(&row, false);
        assert_eq!(fixture.balance().await.remaining_messages, Some(2));
        assert_eq!(peer.calls().len(), 3);
        fixture.finish().await;
        peer.stop().await;
    })
    .await;
}
