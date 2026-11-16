//! Admission, non-replay and health preservation for the image-only manual test.
use super::*;

#[tokio::test]
async fn luma_web_probe_rejects_implicit_calls_and_parameter_overrides() {
    Box::pin(async {
        let peer = Peer::start(vec![]).await;
        let fixture = Box::pin(Fixture::new(&peer)).await;
        for input in [None, Some(json!({"prompt":"OK","model":MODEL,"credentialIds":["exact"]}))] {
            assert_eq!(run(&fixture,input).await.status,ProviderPayloadProbeStatus::Unsupported);
        }
        for change in [json!({"model":"undeclared"}),json!({"credentialIds":["unrelated"]}),
            json!({"media":{"operation":"video","confirmCharge":true}}),
            json!({"media":{"operation":"image_edit","confirmCharge":true}}),
            json!({"media":{"operation":"music","confirmCharge":true}}),
            json!({"media":{"operation":"image","confirmCharge":true,"parameters":{"action_type":"other"}}}),
            json!({"media":{"operation":"image","confirmCharge":true,"parameters":{"image":"https://foreign.invalid"}}})] {
            let mut input=options();input.as_object_mut().unwrap().extend(change.as_object().unwrap().clone());
            assert_eq!(run(&fixture,Some(input)).await.status,ProviderPayloadProbeStatus::Unsupported);
        }
        let original=serde_json::to_value(fixture.state.route_config.snapshot().document()).unwrap();
        for case in 0..4 {
            let mut document=original.clone();
            match case {
                0=>document["providers"][0]["base_url"]=json!("https://foreign.invalid"),
                1=>document["providers"][0].as_object_mut().unwrap().remove("call_flow").map(|_|()).unwrap(),
                2=>document["providers"][0]["execution_mode"]=json!("direct_http"),
                3=>document["providers"][0]["call_flow"]["special_adapter"]=json!("none"),
                _=>unreachable!(),
            }
            // Configuration may reject an incompatible flow before probe admission.
            if fixture.state.route_config.replace_document(serde_json::from_value(document).unwrap()).is_ok() {
                assert_eq!(run(&fixture,Some(options())).await.status,ProviderPayloadProbeStatus::Unsupported,"case {case}");
            }
        }
        assert!(peer.calls().is_empty());
        assert!(fixture.state.local_runtime.as_ref().unwrap().list_audits(&Default::default()).await.unwrap().is_empty());
        fixture.finish().await;peer.stop().await;
    }).await;
}

#[tokio::test]
async fn luma_web_probe_success_and_failure_preserve_existing_model_health() {
    Box::pin(async {
        for failed in [false,true] {
            let image=if failed { Reply::bytes("text/html",b"private remote failure".to_vec()) } else { Reply::image(image::ImageFormat::Jpeg) };
            let peer=Peer::start(vec![Reply::executor(ASSET),image]).await;
            let fixture=Box::pin(Fixture::new(&peer)).await;
            let route=fixture.state.route_config.snapshot();
            let target=route.select_credential_probe_target("exact").unwrap();
            let identity=route.model_probe_identity(&target,NATIVE).unwrap();
            let local=fixture.state.local_runtime.as_ref().unwrap();
            let old=local.model_failure(crate::db::RecordCredentialModelFailureInput {
                provider_account_id:identity.provider_account_id,
                provider_credential_id:identity.provider_credential_id,
                provider_credential_ref:identity.provider_credential_ref,
                protocol_profile:identity.protocol_profile,model:NATIVE.into(),
                upstream_status:Some(401),upstream_retry_after_ms:None,
                error_message:Some("synthetic credential denial".into()),
                classification:crate::provider_failure::classify_provider_failure(Some(401),None,Some("synthetic credential denial")),
            }).await.unwrap().unwrap();
            let before=serde_json::to_value(old).unwrap();
            let report=within(&peer.origin,run(&fixture,Some(options()))).await;
            assert_eq!(report.status,if failed { ProviderPayloadProbeStatus::Failed } else { ProviderPayloadProbeStatus::Unsupported });
            assert_eq!(report.health_recovery,failed.then_some(crate::db::provider_credential_model_states::recovery::ModelHealthRecovery::FailureNotPenalized));
            assert!(!report.message.contains("private remote failure"));
            crate::provider_runtime::record_provider_credential_probe_report(&fixture.state,std::time::Instant::now(),"luma-pipeline","exact",&report).await;
            let rows=local.list_model_states(Default::default()).await.unwrap();
            assert_eq!(rows.len(),1);assert_eq!(serde_json::to_value(&rows[0]).unwrap(),before);
            let audit=fixture.audit().await;assert_eq!(audit.status,if failed {"failed"} else {"completed"});closed_audit(&audit,!failed);
            peer::wire(&peer,true);assert_eq!(fixture.balance().await.remaining_messages,Some(2));
            fixture.finish().await;peer.stop().await;
        }
    }).await;
}

#[tokio::test]
async fn luma_web_probe_cancelled_asset_wait_closes_source_without_replay_or_health() {
    Box::pin(async {
        let mut held=Reply::image(image::ImageFormat::Png);held.hold=true;
        let peer=Peer::start(vec![Reply::executor(ASSET),held]).await;
        let fixture=Box::pin(Fixture::new(&peer)).await;
        let mut future=Box::pin(within(&peer.origin,run(&fixture,Some(options()))));
        tokio::select! { biased; _=peer.wait_calls(2)=>{}, report=&mut future=>panic!("unexpected result: {report:?}") }
        drop(future);peer.wait_closed().await;
        let row=fixture.audit().await;assert_eq!(row.status,"cancelled");closed_audit(&row,false);
        assert!(fixture.state.local_runtime.as_ref().unwrap().list_model_states(Default::default()).await.unwrap().is_empty());
        peer::wire(&peer,true);assert_eq!(fixture.balance().await.remaining_messages,Some(2));
        fixture.finish().await;peer.stop().await;
    }).await;
}
