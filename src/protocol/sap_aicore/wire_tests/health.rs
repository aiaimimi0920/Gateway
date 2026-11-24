//! Public requests prove phase-correct evidence, exact health identity and one send.
use super::*;
mod admission;
mod fallback;
mod stream;

fn metric(name: &str, class: Option<&str>) -> u64 {
    let labels = format!(
        "provider=\"sap_aicore-public\",model=\"deployment1\"{}",
        class
            .map(|c| format!(",failure_class=\"{c}\""))
            .unwrap_or_default()
    );
    let prefix = format!("{name}{{{labels}}} ");
    crate::metrics::request::global_gateway_metrics()
        .render_prometheus()
        .lines()
        .find_map(|line| line.strip_prefix(&prefix))
        .map(|v| v.parse().unwrap())
        .unwrap_or(0)
}

async fn states(fixture: &Fixture) -> Vec<db::GatewayProviderCredentialModelStateView> {
    fixture
        .state
        .local_runtime
        .as_ref()
        .unwrap()
        .list_model_states(db::CredentialModelStateFilters::default())
        .await
        .unwrap()
}

#[tokio::test]
async fn sap_aicore_http_errors_preserve_status_health_and_single_send() {
    Box::pin(async {
        for stream in [false, true] {
            for status in [400, 401, 403, 429, 503] {
                let peer = peer::Peer::new("chat", false, false, status).await;
                *peer.retry_after.lock().unwrap() = Some("120".into());
                let fixture = Box::pin(Fixture::new(&peer)).await;
                super::super::test_support::origin(peer.origin.clone(), async {
                    let metrics_before = metric("gateway_provider_requests_total", None);
                    let class = if status == 429 {
                        "rate_limited"
                    } else if status == 503 {
                        "provider_transient"
                    } else {
                        "unknown"
                    };
                    let errors_before = metric("gateway_provider_errors_total", Some(class));
                    let response = fixture
                        .open("chat/completions", body("chat/completions", stream))
                        .await;
                    assert_eq!(response.status().as_u16(), status);
                    let text = drain(response.into_body()).await;
                    assert!(!text.contains("synthetic-private-error") && !text.contains(KEY));
                    let audit = fixture.audits(1).await;
                    assert_identity(&audit[0]);
                    assert_eq!(audit[0].upstream_status, Some(i32::from(status)));
                    assert_eq!(
                        audit[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
                        1
                    );
                    assert_eq!(peer.calls.lock().unwrap().len(), 1);
                    assert_eq!(
                        metric("gateway_provider_requests_total", None),
                        metrics_before + 1
                    );
                    if matches!(status, 429 | 503) {
                        assert_eq!(
                            metric("gateway_provider_errors_total", Some(class)),
                            errors_before + 1
                        );
                    }
                    assert_eq!(fixture.remaining().await, 2);
                    let health = states(&fixture).await;
                    if matches!(status, 429 | 503) {
                        assert_eq!(health.len(), 1);
                        let exact = &health[0];
                        assert_eq!(exact.provider_credential_ref.as_deref(), Some("exact"));
                        assert_eq!(exact.model, UPSTREAM_MODEL);
                        assert_eq!(exact.failure_count, 1, "never double-penalize");
                        assert_eq!(exact.last_upstream_status, Some(i32::from(status)));
                        assert_eq!(
                            exact.failure_class.as_deref(),
                            Some(if status == 429 {
                                "rate_limited"
                            } else {
                                "provider_transient"
                            })
                        );
                        if status == 429 {
                            let until = time::OffsetDateTime::parse(
                                exact.cooldown_until.as_deref().unwrap(),
                                &time::format_description::well_known::Rfc3339,
                            )
                            .unwrap();
                            assert!(
                                until - time::OffsetDateTime::now_utc()
                                    > time::Duration::seconds(100)
                            );
                        }
                    } else {
                        assert!(health.is_empty(), "{status}: {health:?}");
                    }
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}

#[tokio::test]
async fn sap_aicore_oauth_and_planner_failures_have_zero_inference_attribution() {
    Box::pin(async {
        for stream in [false, true] {
            for oauth in [false, true] {
                let peer = peer::Peer::new("chat", false, false, 200).await;
                if oauth {
                    *peer.token.lock().unwrap() = (401, b"private-service-secret".to_vec());
                }
                let fixture = Box::pin(Fixture::new(&peer)).await;
                let mut request = body("chat/completions", stream);
                if !oauth {
                    request["reasoning"] = json!({"effort":"high"});
                }
                super::super::test_support::origin(peer.origin.clone(), async {
                    let metrics_before = metric("gateway_provider_requests_total", None);
                    let response = fixture.open("chat/completions", request).await;
                    assert!(!response.status().is_success());
                    let text = drain(response.into_body()).await;
                    assert!(!text.contains("private-service-secret"));
                    let rows = fixture.audits(1).await;
                    assert_eq!(
                        rows[0].route_trace.as_ref().unwrap()["routeAttemptCount"],
                        0
                    );
                    assert!(rows[0].provider_account_id.is_none());
                    assert!(rows[0].upstream_status.is_none());
                    assert!(rows[0].route_trace.as_ref().unwrap()["realCredentialRef"].is_null());
                    assert!(states(&fixture).await.is_empty());
                    assert!(peer.calls.lock().unwrap().is_empty());
                    assert_eq!(peer.oauth.lock().unwrap().len(), usize::from(oauth));
                    assert_eq!(
                        metric("gateway_provider_requests_total", None),
                        metrics_before
                    );
                    assert_eq!(fixture.remaining().await, 2);
                })
                .await;
                fixture.finish().await;
                peer.stop().await;
            }
        }
    })
    .await;
}
