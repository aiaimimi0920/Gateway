use super::{
    business_tests::{ready, verify},
    fixture::*,
    tests::body,
    *,
};
use serde_json::json;
const BUDGET: Duration = Duration::from_secs(3);

#[tokio::test]
async fn completion_is_frozen_once_and_optional_fetch_tracks_the_refreshed_snapshot() {
    for fetch_id in [None, Some(""), Some("next-fetch")] {
        let mut next = body();
        next["rows"][0]["handle"] = json!("fbm1.next");
        next.as_object_mut().unwrap().remove("fetchId");
        if let Some(fetch) = fetch_id {
            next["fetchId"] = json!(fetch);
        }
        let peer = Peer::start(vec![
            Spec::catalog(),
            Spec::json(200, json!({"keyId":"device"})),
            Spec::json(200, json!({})),
            Spec::json(200, next),
            Spec::json(200, json!({})),
        ])
        .await;
        with_test_origin(peer.origin.clone(),async {
            let (mut catalog,device,public) = ready(&peer,ClientSurface::Cli).await;
            let mut original = json!({"model":"fbm1.synthetic-open","stream":false,"messages":[{"role":"user","content":"中文\\\""}]});
            {
                let scope = catalog.business_requests(&device,"open").unwrap();
                let frozen = scope.prepare_completion(&original,32768).unwrap();
                let expected = serde_json::to_vec(&original).unwrap();
                original["model"] = json!("caller-mutated-after-freeze");
                original["messages"][0]["content"] = json!("must not reach wire");
                assert!(!format!("{frozen:?}").contains("中文"));
                drop(frozen.send(BUDGET).await.unwrap());
                let hits = peer.hits.lock().unwrap();
                assert_eq!(hits[2].body,expected);
                assert_eq!(hits[2].headers["accept"],"application/json");
                assert!(hits[2].headers.get("x-freebuff-catalog-protocol").is_none(),"completion hook does not add session protocol header");
                verify(&hits[2],&public,"synthetic-fetch");
            }
            catalog.refresh(Some(&device),BUDGET).await.unwrap();
            let scope = catalog.business_requests(&device,"open").unwrap();
            drop(scope.prepare_completion(&json!({"model":"fbm1.next","stream":true}),32768).unwrap().send(BUDGET).await.unwrap());
            let hits = peer.hits.lock().unwrap();
            verify(&hits[4],&public,fetch_id.unwrap_or(""));
            assert_eq!(hits[4].headers.get("x-freebuff-catalog-fetch").map(|h|h.to_str().unwrap()),fetch_id.filter(|s|!s.is_empty()));
            assert_eq!(hits[4].headers["accept"],"text/event-stream");
        }).await;
        peer.close().await;
    }
}

#[tokio::test]
async fn wrong_model_and_encoding_expansion_are_rejected_before_network() {
    let peer = Peer::start(vec![
        Spec::catalog(),
        Spec::json(200, json!({"keyId":"device"})),
    ])
    .await;
    with_test_origin(peer.origin.clone(),async {
        let (catalog,device,_) = ready(&peer,ClientSurface::Cli).await;
        let scope = catalog.business_requests(&device,"open").unwrap();
        for model in ["open","fbm1.other","base3-free-catalog",""] {
            assert_eq!(scope.prepare_completion(&json!({"model":model}),32768).err(),Some(BusinessError::InvalidInput));
        }
        let body = json!({"model":"fbm1.synthetic-open","messages":[{"role":"user","content":"\\\\\\\"中文"}]});
        let expected = serde_json::to_vec(&body).unwrap();
        assert_eq!(scope.prepare_completion(&body,expected.len()-1).err(),Some(BusinessError::RequestTooLarge));
        assert!(scope.prepare_completion(&body,expected.len()).is_ok());
        for limit in [0,50*1024*1024+1] {
            assert_eq!(scope.prepare_completion(&body,limit).err(),Some(BusinessError::InvalidInput));
        }
        assert_eq!(scope.inspect(Duration::ZERO).await.err(),Some(BusinessError::InvalidInput));
        assert_eq!(scope.prepare_completion(&body,32768).unwrap().send(Duration::ZERO).await.err(),Some(BusinessError::InvalidInput));
        assert_eq!(scope.prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks).unwrap().send(Duration::ZERO).await.err(),Some(BusinessError::InvalidInput));
        assert_eq!(peer.count(),2);
    }).await;
    peer.close().await;
}

#[tokio::test]
async fn full_heartbeat_and_explicit_end_pin_the_same_host_generated_attempt() {
    let peer = Peer::start(vec![
        Spec::catalog(),
        Spec::json(200, json!({"keyId":"device"})),
        Spec::json(200, json!({"status":"none"})),
        Spec::json(200, json!({"status":"ended","freebucksRefundPending":true})),
    ])
    .await;
    with_test_origin(peer.origin.clone(), async {
        let (catalog, device, public) = ready(&peer, ClientSurface::Cli).await;
        let scope = catalog.business_requests(&device, "open").unwrap();
        let admission = scope
            .prepare_cli_admission(SessionChargeConsent::MaySpendFreebucks)
            .unwrap();
        let claim = admission.claim().clone();
        assert!(uuid::Uuid::parse_str(claim.attempt_id()).is_ok());
        assert_eq!(claim.instance_id(), format!("cli:{}", claim.attempt_id()));
        drop(admission);
        scope.heartbeat(&claim, BUDGET).await.unwrap();
        let reply = scope.end(&claim, BUDGET).await.unwrap();
        assert_eq!(
            reply.body["freebucksRefundPending"], true,
            "does not invent a refund receipt"
        );
        let hits = peer.hits.lock().unwrap();
        for hit in &hits[2..] {
            verify(hit, &public, "synthetic-fetch");
            assert_eq!(hit.headers["x-freebuff-instance-id"], claim.instance_id());
            assert_eq!(hit.headers["x-freebuff-multi-session"], "1");
            assert_eq!(hit.headers["x-freebuff-purchase-continuity"], "1");
            assert!(hit.headers.get("x-freebuff-model").is_none());
            assert!(hit.headers.get("x-freebuff-wallet-spend-limit").is_none());
        }
        assert_eq!(hits[2].headers["x-freebuff-heartbeat"], "1");
        assert_eq!(
            hits[2].headers["x-freebuff-include-unused-rate-limits"],
            "1"
        );
        assert!(hits[2]
            .headers
            .get("x-freebuff-desktop-attempt-id")
            .is_none());
        assert_eq!(
            hits[3].headers["x-freebuff-desktop-attempt-id"],
            claim.attempt_id()
        );
        assert!(hits[3].headers.get("x-freebuff-heartbeat").is_none());
        assert_eq!(hits.len(), 4, "drop of an unsent admission never purchases");
    })
    .await;
    peer.close().await;
}
