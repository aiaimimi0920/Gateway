use super::{
    fixture::{self, Fixture, KEYS},
    support,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use neuro_gateway::http::router::build_router;
use serde_json::json;
use std::sync::atomic::Ordering;
use tower::ServiceExt;

#[tokio::test]
async fn management_toggle_and_delete_preserve_cash_history() {
    let mut f = Fixture::new().await;
    f.price(1000, 2000, "0.8").await;
    let key = f.key(100_000).await;
    let id = key["id"].as_str().unwrap();
    let path = format!("{KEYS}/{id}/enabled");
    assert_eq!(
        fixture::relay(&f.state, &key, fixture::request(false), None)
            .await
            .0,
        StatusCode::OK
    );
    let ledger = f.finalized(&key, 1).await;
    let balance = f.balance(&key).await;
    for token in ["invalid", key["token"].as_str().unwrap()] {
        assert!(
            !fixture::api(&f.state, &path, token, Some(json!({"enabled":false})), None)
                .await
                .0
                .is_success()
        );
    }
    assert!(!f
        .manage(&path, Some(json!({"enabled":"false"})))
        .await
        .0
        .is_success());
    assert_eq!(
        f.manage(&path, Some(json!({"enabled":false}))).await.0,
        StatusCode::OK
    );
    assert_eq!(
        fixture::relay(&f.state, &key, fixture::request(false), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(f.control.calls.load(Ordering::SeqCst), 1);
    f.restart().await;
    assert_eq!(
        fixture::relay(&f.state, &key, fixture::request(false), None)
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        f.manage(&path, Some(json!({"enabled":true}))).await.0,
        StatusCode::OK
    );
    assert_eq!(f.balance(&key).await, balance);
    assert_eq!(f.ledger(&key).await, ledger);
    assert_eq!(
        fixture::relay(&f.state, &key, fixture::request(false), None)
            .await
            .0,
        StatusCode::OK
    );
    let ledger = f.finalized(&key, 2).await;
    for token in ["invalid", support::MANAGEMENT_TOKEN] {
        let request = Request::delete(format!("{KEYS}/{id}"))
            .header("authorization", format!("Bearer {token}"))
            .header("x-management-token", token)
            .body(Body::empty())
            .unwrap();
        let response = build_router(f.state.clone())
            .oneshot(request)
            .await
            .unwrap();
        assert_eq!(
            response.status().is_success(),
            token == support::MANAGEMENT_TOKEN
        );
    }
    assert_eq!(f.ledger(&key).await, ledger);
    assert!(
        !fixture::relay(&f.state, &key, fixture::request(false), None)
            .await
            .0
            .is_success()
    );
    assert!(!f
        .manage(&path, Some(json!({"enabled":true})))
        .await
        .0
        .is_success());
    let (_, _, catalog) = f.manage("/v1/internal/gateway/access/catalog", None).await;
    assert!(catalog["accessKeys"].as_array().unwrap().is_empty());
    f.finish().await;
}
