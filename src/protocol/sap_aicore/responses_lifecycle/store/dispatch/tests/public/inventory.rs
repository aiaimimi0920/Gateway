//! The unresolved-work inventory is operator-only, paginated, and never sends.
use super::*;
const MANAGEMENT_TOKEN: &str = "console-management-contract-token";

const PATH: &str = "/v1/internal/gateway/console/sap-aicore-responses/pending";

async fn inventory(fixture: &Fixture, token: Option<&str>, path: &str) -> (StatusCode, Value) {
    let mut request = Request::get(path);
    if let Some(token) = token {
        request = request.header("x-management-token", token);
    }
    let response = Box::pin(
        crate::http::router::build_router(fixture.state.clone())
            .oneshot(request.body(Body::empty()).unwrap()),
    )
    .await
    .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    read(response).await
}

#[tokio::test]
async fn sap_aicore_stored_http_inventory_requires_persistent_operator_and_pages() {
    Box::pin(async {
        let wire = Arc::new(Wire::default());
        let (_, fixture, server) = prepare(wire.clone()).await;
        let pool = &fixture.state.local_runtime.as_ref().unwrap().pool;
        for n in 0..101 {
            let id = format!("resp_gw_{n:032x}");
            sqlx::query(
                "INSERT INTO gateway_sap_response_receipts
                (id,owner_tag,binding_tag,provider_ref,credential_ref,phase)
                VALUES ($1,$2,$3,'synthetic-provider','synthetic-credential','submitting')",
            )
            .bind(&id)
            .bind("a".repeat(64))
            .bind("b".repeat(64))
            .execute(pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO gateway_sap_response_route_pins
                (response_id,public_model,source_key_ref) VALUES ($1,$2,'private-source-key')",
            )
            .bind(&id)
            .bind(MODEL)
            .execute(pool)
            .await
            .unwrap();
            sqlx::query(
                "INSERT INTO gateway_sap_response_mutations
                (response_id,operation,confirmed) VALUES ($1,'cancel',0)",
            )
            .bind(&id)
            .execute(pool)
            .await
            .unwrap();
        }
        assert_eq!(
            inventory(&fixture, None, PATH).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(
            inventory(&fixture, Some(&fixture.token), PATH).await.0,
            StatusCode::UNAUTHORIZED
        );
        let (status, page) = inventory(&fixture, Some(MANAGEMENT_TOKEN), PATH).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        let items = page["items"].as_array().unwrap();
        assert_eq!(items.len(), 100);
        assert_eq!(items[0]["upstreamKnown"], false);
        assert_eq!(items[0]["mutationPending"], true);
        assert!(!page.to_string().contains("private-source-key"));
        assert!(items[0].get("ownerTag").is_none());
        let cursor = items.last().unwrap()["id"].as_str().unwrap();
        let (status, next) = inventory(
            &fixture,
            Some(MANAGEMENT_TOKEN),
            &format!("{PATH}?after={cursor}"),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{next}");
        assert_eq!(next["items"].as_array().unwrap().len(), 1);
        assert_eq!(
            inventory(
                &fixture,
                Some(MANAGEMENT_TOKEN),
                &format!("{PATH}?after=resp_private")
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        fixture
            .state
            .console_auth
            .set_management_key_slot(
                &crate::console::ConsoleRequestContext::loopback(),
                MANAGEMENT_TOKEN,
                0,
                false,
                None,
            )
            .unwrap();
        let marker = fixture
            .state
            .console_auth
            .passwordless_token()
            .unwrap()
            .unwrap();
        assert_eq!(
            inventory(&fixture, Some(&marker), PATH).await.0,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(fixture.remaining().await, 2);
        assert!(wire.requests.lock().unwrap().is_empty());
        fixture.finish().await;
        server.abort();
        let _ = server.await;
    })
    .await;
}
