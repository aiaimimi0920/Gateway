//! Resolving across a policy reload must refresh the current routing owner after a real inference 401.
use super::super::test_support::{Peer, Reply};
use super::*;
#[tokio::test]
async fn lobsterai_virtual_inference_401_after_policy_reload_notifies_current_owner() {
    let gate = Arc::new(tokio::sync::Notify::new());
    let peer = Peer::start(vec![
        virtual_models::resolution("openai").gated(gate.clone()),
        Reply::raw(401, vec![]),
        Reply::json(json!({"code":0,"data":{"accessToken":"new-access"}})),
    ])
    .await;
    let fixture = Box::pin(virtual_models::fixture(&peer, "yd-fast")).await;
    let snapshot = fixture.state.route_config.snapshot();
    let owner = snapshot
        .oauth_refresh_target("lobsterai-public", "exact")
        .unwrap()
        .1
        .refresh_config
        .clone()
        .unwrap();
    let generation = owner.lobster_dispatch_generation().unwrap();
    super::super::transport::TEST_ORIGIN.scope(peer.origin.clone(),async {
        let mut pending = Box::pin(fixture.open("chat/completions",body("chat/completions",false)));
        tokio::time::timeout(Duration::from_secs(3),async {
            loop {
                let mut seen=false; peer.inspect(|calls|seen=!calls.is_empty());
                if seen {break;}
                tokio::select! { _=&mut pending => panic!("resolve must await gate"), _=tokio::task::yield_now()=>{} }
            }
        }).await.unwrap();
        let mut doc = snapshot.document().clone();
        doc.providers[0].credential_pool_auth_policy=crate::routing::config::CredentialPoolAuthPolicy::OAuthOnly;
        fixture.state.route_config_runtime.as_ref().unwrap().commit_document(snapshot.revision().id(),doc,None).await.unwrap();
        let current = fixture.state.route_config.snapshot().oauth_refresh_target("lobsterai-public","exact").unwrap().1.refresh_config.clone().unwrap();
        assert!(!Arc::ptr_eq(&owner,&current));
        assert!(!owner.reject_lobster_access(generation,KEY,401));
        gate.notify_one();
        let response=pending.await;
        assert_ne!(response.status(),StatusCode::OK);
        drain(response.into_body()).await;
        tokio::time::timeout(Duration::from_secs(5),async {
            loop {
                if fixture.state.route_config.snapshot().document().providers[0].credentials[0].api_key.as_deref()==Some("new-access") {break;}
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
    }).await;
    assert_ne!(fixture.audits(1).await[0].status, "completed");
    assert_eq!(fixture.remaining().await, 2);
    fixture.finish().await;
    peer.inspect(|calls| {
        assert_eq!(calls.len(), 3);
        assert_eq!(calls[0].path, "/api/models/yd-fast/resolve");
        assert_eq!(calls[1].path, "/api/proxy/v1/chat/completions");
        assert_eq!(calls[2].path, "/api/auth/refresh");
    });
}
