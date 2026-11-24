//! Local public HTTP lifecycle and disconnect using a bounded loopback SAP peer.
use super::*;
use tower::ServiceExt;

const MODEL: &str = "public-sap_aicore";
mod admission;
mod circuit;
mod health;
mod health_boundaries;
mod inventory;
mod ordinary;
mod recovery;
mod safety;

fn create() -> Value {
    json!({"model":MODEL,"input":"hello","store":true})
}

async fn prepare(wire: Arc<Wire>) -> (String, Fixture, tokio::task::JoinHandle<()>) {
    let socket = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", socket.local_addr().unwrap());
    let app = Router::new().fallback(handler).with_state(wire);
    let server = tokio::spawn(async move { axum::serve(socket, app).await.unwrap() });
    let fixture = Box::pin(Fixture::configured_with_http(
        scoped_loopback_http(), false, BASE, KEY,
        json!({"sap_native_responses":true,"sap_stored_responses":true,"sap_api_version":"2024-10-21"}),
    )).await;
    (origin, fixture, server)
}

async fn at(fixture: &Fixture, method: &str, path: &str) -> (StatusCode, Value) {
    let response = Box::pin(
        crate::http::router::build_router(fixture.state.clone()).oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("authorization", format!("Bearer {}", fixture.token))
                .body(Body::empty())
                .unwrap(),
        ),
    )
    .await
    .unwrap();
    read(response).await
}

async fn read(response: Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    (status, serde_json::from_slice(&bytes).unwrap())
}

#[tokio::test]
async fn sap_aicore_stored_http_local_create_retrieve_delete() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (origin, fixture, server) = prepare(wire.clone()).await;
        test_support::origin(origin, async {
            let (status, response) = read(fixture.open("responses", create()).await).await;
            assert_eq!(status, StatusCode::OK, "{response}");
            let id = response["id"].as_str().unwrap();
            assert!(id.starts_with("resp_gw_"));
            assert_eq!(response["model"], MODEL);
            assert_eq!(response["usage"]["total_tokens"], 5);
            assert!(!response.to_string().contains("resp_created"));
            let path = format!("/v1/responses/{id}");
            assert_eq!(
                at(&fixture, "GET", "/v1/responses/resp_created").await.0,
                StatusCode::NOT_FOUND
            );
            let (status, loaded) = at(&fixture, "GET", &path).await;
            assert_eq!(status, StatusCode::OK, "{loaded}");
            assert_eq!(loaded["id"], id);
            assert_eq!(
                at(&fixture, "DELETE", &path).await,
                (
                    StatusCode::OK,
                    json!({"id":id,"object":"response.deleted","deleted":true}),
                )
            );
            assert_eq!(at(&fixture, "GET", &path).await.0, StatusCode::NOT_FOUND);
            assert_eq!(fixture.remaining().await, 1);
            let audits = fixture.audits(5).await;
            let attempts = |audit: &&crate::db::GatewayRequestAuditView| {
                audit.route_trace.as_ref().unwrap()["routeAttemptCount"]
                    .as_u64()
                    .unwrap()
            };
            assert_eq!(
                audits.iter().filter(|audit| attempts(audit) == 0).count(),
                2
            );
            assert!(audits
                .iter()
                .filter(|audit| attempts(audit) == 0)
                .all(|audit| audit.provider_account_id.is_none()));
            assert!(audits
                .iter()
                .filter(|audit| attempts(audit) > 0)
                .all(|audit| {
                    audit.provider_account_id.as_deref() == Some("sap_aicore-public")
                        && audit.route_trace.as_ref().is_some_and(|trace| {
                            trace["routeAttemptCount"] == 1 && trace["realCredentialRef"] == "exact"
                        })
                }));
            assert_eq!(wire.requests.lock().unwrap().len(), 3);
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_stored_http_disconnect_keeps_receipt_and_single_debit() {
    Box::pin(async {
        let started = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let wire = Arc::new(Wire {
            pause_create: Some((started.clone(), release.clone())),
            ..Wire::default()
        });
        let (origin, fixture, server) = prepare(wire.clone()).await;
        test_support::origin(origin, async {
            {
                let request = fixture.open("responses", create());
                tokio::pin!(request);
                tokio::select! {
                    _ = started.notified() => {}
                    response = &mut request => panic!("premature response: {}", response.status()),
                }
            } // Drop the HTTP handler's waiter after the mock saw the send.
            fixture.state.lifecycle.begin_drain("test");
            release.notify_one();
            tokio::time::timeout(
                std::time::Duration::from_secs(5),
                fixture.state.lifecycle.stored_response_tasks.join(),
            )
            .await
            .unwrap();
            let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
            let settled: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM gateway_sap_response_receipts
                 WHERE upstream_id = 'resp_created' AND phase = 'completed' AND settled = 1",
            )
            .fetch_one(pool)
            .await
            .unwrap();
            assert_eq!(settled, 1);
            assert_eq!(fixture.remaining().await, 1);
            assert_eq!(wire.requests.lock().unwrap().len(), 1);
            let audits = fixture.audits(1).await;
            assert!(audits[0].client_disconnected_at.is_some());
            assert_eq!(
                audits[0].route_trace.as_ref().unwrap()["callerDisconnected"],
                true
            );
        })
        .await;
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}
