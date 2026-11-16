//! Durable management disable races a real public request, never a manually constructed SDK identity.
use super::*;
use std::{path::Path, time::Duration};

async fn disable(f: &fixture::Fixture) {
    let snapshot = f.console.state.route_config.snapshot();
    let redacted = redact_route_document(snapshot.document()).unwrap();
    let mut document = redacted.document;
    document.providers[0].credentials[0].enabled = Some(false);
    let patches: Vec<_> = redacted
        .secrets
        .iter()
        .map(|s| SecretPatch::keep(&s.path))
        .collect();
    let grant = fixture::support::grant_console_secret_access(&f.console.state).await;
    let response = fixture::support::send_console_json(
        &f.console.state,
        Method::PUT,
        "/v1/internal/gateway/console/route-config",
        json!({
            "expectedRevision":snapshot.revision().id(),"document":document,"secretPatches":patches
        }),
        Some(&grant),
        None,
    )
    .await;
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    assert!(!String::from_utf8_lossy(&bytes).contains("synthetic-token"));
    assert!(f
        .console
        .state
        .route_config
        .snapshot()
        .resolve_candidates(Some("public-alias"))
        .is_empty());
}

fn observed(root: &Path, method: &str) -> bool {
    std::fs::read_dir(root)
        .unwrap()
        .filter_map(Result::ok)
        .any(|entry| {
            std::fs::read_to_string(entry.path().join("rpc.jsonl"))
                .is_ok_and(|text| text.contains(method))
        })
}

async fn revoked(control: &str, method: &str, responses: bool) {
    let f = saved().await;
    let workspace =
        std::path::PathBuf::from(std::env::var_os("GATEWAY_COPILOT_WORKSPACE_ROOT").unwrap());
    assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 0);
    let barrier = workspace.join(control);
    std::fs::write(&barrier, "synthetic management revocation barrier").unwrap();
    let (path, body) = if responses {
        (
            "/v1/responses",
            json!({"model":"public-alias","input":"ok"}),
        )
    } else {
        (
            "/v1/chat/completions",
            json!({"model":"public-alias","messages":[{"role":"user","content":"ok"}]}),
        )
    };
    let request = call(&f, path, body.clone());
    let revoke = async {
        while !observed(&workspace, method) {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(
            f.local
                .access_balance(&f.key)
                .await
                .unwrap()
                .unwrap()
                .remaining_messages,
            Some(1)
        );
        disable(&f).await;
        // The fixture owns this single barrier under the required temporary workspace.
        std::fs::remove_file(&barrier).unwrap();
    };
    let ((status, value), ()) = tokio::time::timeout(Duration::from_secs(45), async {
        tokio::join!(request, revoke)
    })
    .await
    .expect("management revocation must finish without a generation timeout");
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{value}");
    assert!(value.get("error").is_some());
    assert!(!value.to_string().contains("synthetic answer"));
    assert!(!value.to_string().contains("synthetic-token"));
    assert_eq!(
        f.local
            .access_balance(&f.key)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(2)
    );
    // Reusing the saved Key after disable cannot select the account or consume quota.
    let (status, _) = call(&f, path, body).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(
        f.local
            .access_balance(&f.key)
            .await
            .unwrap()
            .unwrap()
            .remaining_messages,
        Some(2)
    );
    let rows = f
        .local
        .list_audits(&crate::db::RequestAuditFilters::default())
        .await
        .unwrap();
    assert_eq!(rows.len(), 2);
    let mut attempts = Vec::new();
    for row in rows {
        assert_ne!(row.status, "completed");
        assert!(row.upstream_status.is_none() && row.total_tokens.is_none());
        attempts.push(
            row.route_trace.as_ref().unwrap()["routeAttemptCount"]
                .as_u64()
                .unwrap(),
        );
    }
    attempts.sort();
    assert_eq!(attempts, vec![0, 1]);
    assert_eq!(std::fs::read_dir(&workspace).unwrap().count(), 0);
    finish(f).await;
}

#[tokio::test]
#[ignore = "requires synthetic CLI and fresh probe/workspace roots; run alone"]
async fn copilot_public_revocation_during_startup_prevents_generation() {
    revoked("hold-connect", "connect", false).await;
}

#[tokio::test]
#[ignore = "requires synthetic CLI and fresh probe/workspace roots; run alone"]
async fn copilot_public_revocation_during_generation_prevents_publication() {
    revoked("hold-text", "session.sendAndWait", true).await;
}
