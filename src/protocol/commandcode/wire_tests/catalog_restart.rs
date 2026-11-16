//! Management catalog -> secret-preserving CAS -> local runtime restart -> native probe and inference.
use super::*;
use axum::http::Method;
use support::{grant_console_secret_access, parse_json, send_console_json};

#[tokio::test]
async fn commandcode_catalog_save_restart_preserves_native_routes_and_account() {
    Box::pin(async {
        for family in ["messages", "responses"] {
            let peer = peer::Peer::new(family, false, false, 200).await;
            let f = Box::pin(Fixture::new(&peer)).await;
            // 默认 feature 的管理路由 future 很大；在组合测试内保持堆分配，不提高测试线程栈掩盖问题。
            let grant = Box::pin(grant_console_secret_access(&f.state)).await;
            let snapshot = f.state.route_config.snapshot();
            let original_revision = snapshot.revision().id().to_owned();
            let endpoint = format!("/{family}");
            let catalog_body = json!({"data":[{"id":UPSTREAM_MODEL,"supported_endpoints":[endpoint]}]});
            let app = axum::Router::new().route("/provider/v1/models", axum::routing::get(move |headers: axum::http::HeaderMap| {
                let catalog_body = catalog_body.clone();
                async move {
                    assert!(!headers.contains_key("authorization") && !headers.contains_key("cookie"));
                    axum::Json(catalog_body)
                }
            }));
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let origin = format!("http://{}", listener.local_addr().unwrap());
            let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
            let response = super::super::test_support::origin(origin, Box::pin(send_console_json(
                &f.state, Method::POST, "/v1/internal/gateway/console/commandcode-model-catalog",
                json!({"credentialId":"exact"}), Some(&grant), None))).await;
            let status = response.status();
            let fetched = parse_json(response).await;
            assert_eq!(status, StatusCode::OK, "{fetched}");
            assert_eq!(fetched["catalog"]["models"], json!([UPSTREAM_MODEL]));
            assert_eq!(f.state.route_config.snapshot().revision().id(), original_revision);
            let redacted = crate::console::secrets::redact_route_document(snapshot.document()).unwrap();
            let patches: Vec<_> = redacted.secrets.iter().filter(|secret| secret.configured)
                .map(|secret| json!({"path":secret.path,"operation":"keep"})).collect();
            let mut document = serde_json::to_value(redacted.document).unwrap();
            document["providers"][0]["credentials"][1]["model_catalog"] = fetched["catalog"].clone();
            document["providers"][0]["headers"] = json!({"X-Cmd-Zdr":"1"});
            document["providers"][0]["credentials"][1]["headers"] = json!({"x-cmd-zdr":"1"});
            document["providers"][0]["credentials"][1]["supported_models"] = json!([UPSTREAM_MODEL]);
            let response = Box::pin(send_console_json(&f.state, Method::PUT, "/v1/internal/gateway/console/route-config",
                json!({"expectedRevision":original_revision,"document":document,"secretPatches":patches}), Some(&grant), None)).await;
            let status = response.status();
            let saved = parse_json(response).await;
            assert_eq!(status, StatusCode::OK, "{saved}");
            assert!(!saved.to_string().contains(KEY));
            let saved_revision = f.state.route_config.snapshot().revision().id().to_owned();
            drop(snapshot);
            task.abort();
            assert!(task.await.unwrap_err().is_cancelled());
            let f = Box::pin(f.restart(&peer)).await;
            assert_eq!(f.state.route_config.snapshot().revision().id(), saved_revision);
            let restored = f.state.route_config.snapshot().select_credential_probe_target("exact").unwrap();
            assert_eq!(restored.payload.api_key, KEY);
            assert!(super::super::catalog::supports(&restored.payload, UPSTREAM_MODEL, &format!("/{family}")).unwrap());
            super::super::test_support::origin(peer.origin.clone(), Box::pin(async {
                let report = Box::pin(probe::call(&f)).await;
                assert_eq!(report.status, crate::provider_runtime::ProviderPayloadProbeStatus::Passed, "{report:?}");
                let response = f.open(family, body(family, false)).await;
                assert_eq!(response.status(), StatusCode::OK);
                assert!(drain(response.into_body()).await.contains("合成答案"));
            })).await;
            let rows = f.audits(2).await;
            assert!(rows.iter().all(|row| row.status == "completed" && row.total_tokens == Some(5)));
            assert_eq!(f.remaining().await, 1);
            let calls = peer.calls.lock().unwrap().clone();
            assert_eq!(calls.len(), 2);
            for (path, headers, _) in calls {
                assert_eq!(path, format!("/provider/v1/{family}"));
                assert_eq!(headers["authorization"], format!("Bearer {KEY}"));
                assert_eq!(headers.get_all("x-cmd-zdr").iter().count(), 1);
                assert_eq!(headers["x-cmd-zdr"], "1");
            }
            f.finish().await;
            peer.stop().await;
        }
    }).await;
}
