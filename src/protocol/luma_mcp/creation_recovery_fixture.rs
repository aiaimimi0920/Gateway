//! Real local configuration commits and explicit refresh routes against a disposable OAuth peer.
use super::*;
use crate::routing::config::RouteConfigYaml;

pub(super) async fn commit(f: &Fixture, edit: impl FnOnce(&mut RouteConfigYaml)) {
    let snapshot = f.state.route_config.snapshot();
    let mut document = snapshot.document().clone();
    edit(&mut document);
    f.state
        .route_config_runtime
        .as_ref()
        .unwrap()
        .commit_document(snapshot.revision().id(), document, None)
        .await
        .unwrap();
}

pub(super) async fn offline(f: &Fixture) {
    commit(f, |document| {
        let c = &mut document.providers[0].credentials[0];
        c.refresh_token = Some("synthetic-refresh".into());
        let mut context = serde_json::to_value(c.luma_oauth.as_ref().unwrap()).unwrap();
        context["offline_access"] = json!(true);
        c.luma_oauth = Some(serde_json::from_value(context).unwrap());
    })
    .await;
}

pub(super) async fn restart(f: &mut Fixture) {
    browser_fixture::close(&f.state).await;
    f.state = browser_fixture::state(&f.root).await;
    f.grant = support::grant_console_secret_access(&f.state).await;
}

pub(super) async fn refresh(f: &Fixture, index: u32) {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;
    let snapshot = f.state.route_config.snapshot();
    let old_refresh = snapshot.document().providers[0].credentials[0]
        .refresh_token
        .clone()
        .unwrap();
    let count = Arc::new(AtomicUsize::new(0));
    let seen = count.clone();
    let app = axum::Router::new().route("/token", axum::routing::post(
        move |axum::Form(form): axum::Form<std::collections::HashMap<String, String>>| {
            assert_eq!(form["refresh_token"], old_refresh);
            assert_eq!(form["grant_type"], "refresh_token");
            seen.fetch_add(1, Ordering::SeqCst);
            async move { axum::Json(json!({"access_token":format!("rotation-{index}"),
                "refresh_token":format!("refresh-{index}"),"token_type":"Bearer","expires_in":3600})) }
        }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}/token", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    crate::protocol::luma_oauth::test_support::ENDPOINT.scope(endpoint, async {
        let path = "/v1/internal/gateway/console/luma-refresh-sessions";
        let response = support::send_console_json(&f.state, Method::POST, path,
            json!({"providerId":"luma-mcp","credentialId":"saved-mcp","expectedRevision":snapshot.revision().id()}),
            Some(&f.grant), None).await;
        let status = response.status();
        let body = support::parse_json(response).await;
        assert_eq!(status, 200, "refresh {index}: {body}");
        let path = format!("{path}/{}", body["session"]["id"].as_str().unwrap());
        let response = support::send_console_json(&f.state, Method::POST, &format!("{path}/import"),
            json!({"expectedRevision":snapshot.revision().id()}), Some(&f.grant), None).await;
        assert_eq!(response.status(), 200);
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let response = support::send_console_json(&f.state, Method::GET, &path, Value::Null, None, None).await;
                assert_eq!(response.status(), 200);
                let body = support::parse_json(response).await;
                match body["session"]["status"].as_str().unwrap() {
                    "succeeded" => break,
                    "importing" => tokio::time::sleep(Duration::from_millis(10)).await,
                    _ => panic!("refresh did not complete: {body}"),
                }
            }
        }).await.unwrap();
        // Runtime shutdown owns registry closure; the next explicit rotation still needs it.
    }).await;
    assert_eq!(count.load(Ordering::SeqCst), 1);
    server.abort();
    assert!(server.await.unwrap_err().is_cancelled());
}

pub(super) async fn unpublished_successor(f: &Fixture) {
    use crate::console::{
        refresh_grants::{GrantReceipt, GrantRequest, GrantReservation},
        RouteConfigRedisRevision,
    };
    let runtime = f.state.route_config_runtime.as_ref().unwrap();
    let snapshot = f.state.route_config.snapshot();
    let original =
        RouteConfigRedisRevision::new(snapshot.revision().clone(), snapshot.document().clone())
            .unwrap();
    let request = GrantRequest::from_revision(&original, "luma-mcp", "saved-mcp").unwrap();
    let sending = match runtime
        .reserve_refresh_grant(&original, &request)
        .await
        .unwrap()
    {
        GrantReservation::Reserved(record) => record,
        other => panic!("expected reservation, got {other:?}"),
    };
    let tokens = crate::protocol::luma_oauth::ReceivedRefresh::capture(
        br#"{"access_token":"unpublished-access","refresh_token":"unpublished-refresh","token_type":"Bearer","expires_in":3600}"#,
        time::OffsetDateTime::now_utc(), request.material.luma_oauth.as_ref().unwrap(), &request.material.refresh).unwrap();
    let result = request.result_with_luma(&tokens).unwrap();
    let receipt = runtime
        .store_refresh_grant_receipt(&sending, &GrantReceipt::LumaOauth { tokens })
        .await
        .unwrap();
    runtime
        .store_refresh_grant_result(&receipt, &result)
        .await
        .unwrap();
    commit(f, |document| {
        let c = &mut document.providers[0].credentials[0];
        c.api_key = Some(result.material.access.clone());
        c.refresh_token = Some(result.material.refresh.clone());
        c.luma_oauth = result.material.luma_oauth.clone();
    })
    .await;
}
