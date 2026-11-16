//! An actual public 401 wakes the long-interval durable worker; only the next request uses the new token.
use super::super::test_support::{Peer, Reply};
use super::*;
#[tokio::test]
async fn lobsterai_public_401_wakes_durable_worker_without_hidden_retry() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for stream in [false, true] {
                let peer = Peer::start(vec![
                    Reply::raw(401, KEY.as_bytes().to_vec()),
                    Reply::json(json!({"code":0,"data":{"accessToken":"new-access"}})),
                    Reply::raw(200, native::reply("chat", false, false, true))
                        .header("content-type", "application/json"),
                ])
                .await;
                let fixture = Box::pin(Fixture::with_http(
                    peer.builder()
                        .retry(rquest::retry::Policy::never())
                        .build()
                        .unwrap(),
                    peer.origin.clone(),
                ))
                .await;
                let initial = fixture.state.route_config.snapshot();
                let owner = initial
                    .oauth_refresh_target("lobsterai-public", "exact")
                    .unwrap()
                    .1
                    .refresh_config
                    .clone()
                    .unwrap();
                let generation = owner.lobster_dispatch_generation().unwrap();
                let revision = crate::console::RouteConfigRedisRevision::new(
                    initial.revision().clone(),
                    initial.document().clone(),
                )
                .unwrap();
                let grant = crate::console::refresh_grants::GrantRequest::from_revision(
                    &revision,
                    "lobsterai-public",
                    "exact",
                )
                .unwrap();
                let persistence = crate::console::persistence::RouteConfigPersistence::new(
                    &fixture.state.config.console.state_dir,
                    &fixture.state.config.console.routes_file,
                )
                .unwrap();
                let receipt = persistence
                    .state_root()
                    .join("refresh-grants")
                    .join(grant.family_id())
                    .join(format!("{}.json", grant.material_id()));
                super::super::transport::TEST_ORIGIN
                    .scope(peer.origin.clone(), async {
                        let response = fixture.open(endpoint, body(endpoint, stream)).await;
                        assert_ne!(response.status(), StatusCode::OK);
                        assert!(!drain(response.into_body()).await.contains(KEY));
                        assert_ne!(fixture.audits(1).await[0].status, "completed");
                        assert_eq!(fixture.remaining().await, 2);
                        tokio::time::timeout(Duration::from_secs(5), async {
                            loop {
                                let snapshot = fixture.state.route_config.snapshot();
                                let credential = &snapshot.document().providers[0].credentials[0];
                                if credential.api_key.as_deref() == Some("new-access") {
                                    assert!(
                                        credential.expires_at.is_none()
                                            && credential.token_expires_in_secs.is_none()
                                    );
                                    if let Ok(text) = std::fs::read_to_string(&receipt) {
                                        let record =
                                            crate::console::refresh_grants::GrantRecord::from_json(
                                                &text,
                                            )
                                            .unwrap();
                                        if record.phase()
                                            == crate::console::refresh_grants::GrantPhase::Published
                                        {
                                            assert!(record.receipt().is_some());
                                            break;
                                        }
                                    }
                                }
                                tokio::task::yield_now().await;
                            }
                        })
                        .await
                        .unwrap();
                        assert!(!owner.reject_lobster_access(generation, KEY, 401));
                        owner.dispatch_readiness().unwrap();
                        let response = fixture.open(endpoint, body(endpoint, stream)).await;
                        let status = response.status();
                        let text = drain(response.into_body()).await;
                        assert_eq!(status, StatusCode::OK, "{text}");
                        assert_eq!(
                            fixture
                                .audits(2)
                                .await
                                .iter()
                                .filter(|r| r.status == "completed")
                                .count(),
                            1
                        );
                        assert_eq!(fixture.remaining().await, 1);
                    })
                    .await;
                peer.inspect(|s| {
                    assert_eq!(s.len(), 3);
                    assert_eq!(s[0].path, "/api/proxy/v1/chat/completions");
                    assert_eq!(s[0].headers["authorization"], format!("Bearer {KEY}"));
                    assert_eq!(s[1].path, "/api/auth/refresh");
                    assert_eq!(s[1].body["refreshToken"], "synthetic-refresh");
                    assert_eq!(s[2].headers["authorization"], "Bearer new-access");
                });
                fixture.finish().await;
            }
        }
    })
    .await;
}
#[tokio::test]
async fn lobsterai_public_non_401_does_not_refresh_or_replay() {
    Box::pin(async {
        for status in [403, 429, 500] {
            let peer = Peer::start(vec![Reply::raw(status, KEY.as_bytes().to_vec())]).await;
            let fixture = Box::pin(Fixture::with_http(
                peer.builder().build().unwrap(),
                peer.origin.clone(),
            ))
            .await;
            super::super::transport::TEST_ORIGIN
                .scope(peer.origin.clone(), async {
                    let response = fixture
                        .open("chat/completions", body("chat/completions", false))
                        .await;
                    assert_ne!(response.status(), StatusCode::OK);
                    assert!(!drain(response.into_body()).await.contains(KEY));
                    assert_ne!(fixture.audits(1).await[0].status, "completed");
                    assert_eq!(fixture.remaining().await, 2);
                    let snapshot = fixture.state.route_config.snapshot();
                    let owner = snapshot
                        .oauth_refresh_target("lobsterai-public", "exact")
                        .unwrap()
                        .1
                        .refresh_config
                        .as_ref()
                        .unwrap();
                    assert!(owner.begin_refresh(Duration::ZERO, KEY).is_none());
                })
                .await;
            fixture.finish().await;
            peer.inspect(|s| assert_eq!(s.len(), 1));
        }
    })
    .await;
}
