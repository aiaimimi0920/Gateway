use super::*;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn copilot_sender_public_disabled_credentials_do_not_charge_or_attempt() {
    let f = configured().await;
    f.replace(|doc| doc.providers[0].credentials[0].enabled = Some(false));
    let token = f.local.access_key_secret(&f.key).await.unwrap();
    let before = balance_snapshot(&f).await;
    let router = crate::http::router::build_router(f.console.state.clone());
    for path in ["/v1/chat/completions", "/v1/responses"] {
        for stream in [false, true] {
            let mut body = if path.ends_with("responses") {
                json!({"model":"public-alias", "input":"hello"})
            } else {
                json!({"model":"public-alias", "messages":[{"role":"user","content":"hello"}]})
            };
            body["stream"] = json!(stream);
            let response = Box::pin(
                router.clone().oneshot(
                    Request::post(path)
                        .header("authorization", format!("Bearer {token}"))
                        .header("content-type", "application/json")
                        .body(Body::from(body.to_string()))
                        .unwrap(),
                ),
            )
            .await
            .unwrap();
            let status = response.status();
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            // Disabled compiled accounts also leave this restricted Key with no permitted provider.
            assert_eq!(status, StatusCode::FORBIDDEN, "{value}");
            assert!(value.get("error").is_some());
            assert!(!value.to_string().contains("synthetic-token"));
        }
    }
    let rows = f
        .local
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 4);
    for row in rows {
        assert_eq!(row.route_trace.as_ref().unwrap()["routeAttemptCount"], 0);
        assert!(row.upstream_status.is_none());
        assert!(row.total_tokens.is_none());
        assert_ne!(row.status, "completed");
    }
    assert_eq!(balance_snapshot(&f).await, before);
    drop(router);
    finish(f).await;
}

#[tokio::test]
async fn copilot_sender_real_eligibility_rejects_before_consuming_attempt() {
    let f = configured().await;
    f.replace(|doc| doc.providers[0].credentials[0].enabled = Some(false));
    let gate = super::super::super::stage_rate_limit::provider_attempt_gate(
        &f.ctx,
        &f.console.state,
        "copilot",
    )
    .unwrap()
    .with_eligibility(
        super::super::super::stage_rate_limit::eligibility::EligibilityCheck::new(
            &f.console.state,
            &f.candidate,
            "synthetic-model",
        ),
    );
    // The outer sender admits before entering its execute_observed wrapper.
    let result = gate.admit().await;
    assert!(result.is_err());
    assert_eq!(f.ctx.route_attempt_count(), 0);
    finish(f).await;
}
