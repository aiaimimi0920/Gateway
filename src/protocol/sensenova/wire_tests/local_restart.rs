//! A real Local management commit is used by a new runtime, not just the in-memory snapshot.
use super::*;
use axum::http::Method;
use support::{grant_console_secret_access, parse_json, send_console_json};

const CONFIG: &str = "/v1/internal/gateway/console/route-config";
const CATALOG: &str = "/v1/internal/gateway/console/sensenova-model-catalog";
const PROBE: &str = "/v1/internal/gateway/console/providers/sensenova-public/probe";

#[test]
fn sensenova_local_management_save_restart_and_use_all_public_routes() {
    server_fixture::run(save_and_restart);
}

async fn save_and_restart() {
    let chat = peer::Peer::new("chat", false, false, 200).await;
    let mut f = Fixture::with_document(
        &chat,
        json!({"providers":[],"model_routes":[]}),
        ENDPOINTS.len(),
    )
    .await;
    let grant = grant_console_secret_access(&f.state).await;
    let original = f.state.route_config.snapshot().revision().id().to_owned();
    let mut document = json!({"providers":[{"id":"sensenova-public","preset":super::super::PRESET,
        "base_url":super::super::BASE,"explicit_credentials_only":true,
        "supported_models":[MODEL],"model_map":{MODEL:UPSTREAM_MODEL},"request_wait":{"mode":"disabled"},
        "credentials":[{"id":"exact","account_name":"Isolated account","supported_models":[MODEL]}]}],"model_routes":[]});
    commit(&f, &grant, &original, &document, "replace").await;

    let saved_before_catalog = f.state.route_config.snapshot().revision().id().to_owned();
    let (catalog_url, task) = support::spawn_probe_server(support::ProbeServerState {
        accepted_authorizations: vec![format!("Bearer {KEY}")],
        status: StatusCode::OK,
        body: json!({"data":[{"id":UPSTREAM_MODEL,"output_modalities":["text"]},
            {"id":"sensenova-u-image","output_modalities":["image"]}]})
        .to_string(),
    })
    .await;
    let catalog =
        super::super::test_support::origin(catalog_url.trim_end_matches("/v1").into(), async {
            let response = send_console_json(
                &f.state,
                Method::POST,
                CATALOG,
                json!({"credentialId":"exact"}),
                Some(&grant),
                None,
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            parse_json(response).await
        })
        .await;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(catalog["revision"], saved_before_catalog);
    assert_eq!(catalog["catalog"]["models"], json!([UPSTREAM_MODEL]));
    assert_eq!(
        f.state.route_config.snapshot().revision().id(),
        saved_before_catalog
    );
    document["providers"][0]["credentials"][0]["model_catalog"] = catalog["catalog"].clone();
    commit(&f, &grant, &saved_before_catalog, &document, "keep").await;
    let revision = f.state.route_config.snapshot().revision().id().to_owned();
    assert_ne!(revision, saved_before_catalog);
    assert!(
        chat.calls.lock().unwrap().is_empty(),
        "save and discovery must not generate"
    );
    assert_eq!(f.remaining().await, 3);

    // Every peer is non-forwarding; rebuild against the identical persisted routes and SQLite data.
    let mut audit_count = 0;
    for (index, endpoint) in ENDPOINTS.into_iter().enumerate() {
        let peer = peer::Peer::new(family(endpoint), false, false, 200).await;
        f = f.restart(&peer).await;
        let current = f.state.route_config.snapshot();
        assert_eq!(current.revision().id(), revision);
        let restored = &current.document().providers[0].credentials[0];
        assert_eq!(restored.id.as_deref(), Some("exact"));
        assert!(restored.api_key.as_deref() == Some(KEY), "lost stored key");
        assert_eq!(
            restored.model_catalog.as_ref().unwrap().models,
            [UPSTREAM_MODEL]
        );
        drop(current);
        assert_eq!(f.remaining().await, (3 - index) as i64);
        super::super::test_support::origin(peer.origin.clone(), async {
            if index == 0 {
                let grant = grant_console_secret_access(&f.state).await;
                let response = send_console_json(
                    &f.state,
                    Method::POST,
                    PROBE,
                    json!({"credentialIds":["exact"],"model":MODEL,"prompt":"only OK"}),
                    Some(&grant),
                    None,
                )
                .await;
                let status = response.status();
                let result = parse_json(response).await;
                assert_eq!(status, StatusCode::OK, "{result}");
                assert_eq!(result["result"]["results"][0]["status"], "passed");
                assert!(!result.to_string().contains(KEY));
                assert_eq!(
                    f.remaining().await,
                    3,
                    "internal probe must not spend the caller balance"
                );
                audit_count += 1;
            }
            let response = f.open(endpoint, body(endpoint, false)).await;
            let status = response.status();
            let text = drain(response.into_body()).await;
            assert_eq!(status, StatusCode::OK, "{endpoint}: {text}");
            assert!(text.contains("合成答案"));
        })
        .await;
        audit_count += 1;
        assert_wire(&peer, if index == 0 { 2 } else { 1 }, endpoint);
        let rows = f.audits(audit_count).await;
        assert!(rows
            .iter()
            .all(|row| row.status == "completed" && row.total_tokens == Some(5)));
        let public: Vec<_> = rows
            .iter()
            .filter(|row| row.access_key_id.is_some())
            .collect();
        assert_eq!(public.len(), index + 1);
        for row in public {
            assert_identity(row);
        }
        assert_eq!(f.remaining().await, (2 - index) as i64);
        peer.stop().await;
    }
    assert_eq!(f.remaining().await, 0);
    f.finish().await;
    chat.stop().await;
}

async fn commit(f: &Fixture, grant: &str, revision: &str, document: &Value, operation: &str) {
    let mut patch = json!({"path":"/providers/0/credentials/0/api_key","operation":operation});
    if operation == "replace" {
        patch["value"] = json!(KEY);
    }
    let response = send_console_json(
        &f.state,
        Method::PUT,
        CONFIG,
        json!({"expectedRevision":revision,"document":document,"secretPatches":[patch]}),
        Some(grant),
        None,
    )
    .await;
    let status = response.status();
    let result = parse_json(response).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["committed"], true);
    assert!(!result.to_string().contains(KEY));
}
