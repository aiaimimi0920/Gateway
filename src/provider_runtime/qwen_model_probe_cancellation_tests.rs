//! Actual management future cancellation must release transport and persist one neutral audit.
use super::*;
use crate::{db, local_runtime::LocalRuntime};

#[test]
fn qwen_probe_outer_cancellation_finalizes_exact_account_without_health_or_usage() {
    run(cancel);
}
async fn cancel() {
    tokio::time::timeout(Duration::from_secs(15), async {
        for plan in [false, true] {
            let server = wire::Server::start().await;
            let mut fixture = fixture();
            let path = std::env::temp_dir().join(format!("qwen-probe-cancel-{}",uuid::Uuid::new_v4()));
            let local = LocalRuntime::open(&path).await.unwrap();
            let state = Arc::get_mut(&mut fixture.state).unwrap();
            state.local_runtime = Some(local.clone());
            state.upstream_client.http = server.client();
            let grant = grant_console_secret_access(&fixture.state).await;
            let body = if plan {
                json!({"testPlan":{"automaticEnabled":false,"intervalMinutes":15,
                    "modelSelection":"selected","models":["probe-alias"],"cases":[{"id":"hold",
                    "name":"Hold","prompt":"hold-body","expectedAnswer":"OK","difficulty":1,"enabled":true},
                    {"id":"after-hold","name":"After","prompt":"OK","expectedAnswer":"OK","difficulty":1,"enabled":true}]},
                    "scope":{"kind":"account","id":"other"},"credentialIds":["other"]})
            } else { json!({"prompt":"hold-body","model":"probe-alias","credentialIds":["other"]}) };
            let mut request = Box::pin(with_transport(&server.origin, send_console_json(
                &fixture.state,Method::POST,ENDPOINT,body,Some(&grant),None)));
            // Poll the real router until both SQL and the actual HTTP handler confirm the in-flight phase.
            tokio::time::timeout(Duration::from_millis(800),async {
                loop {
                    tokio::select! {
                        _ = &mut request => panic!("Held upstream completed before cancellation"),
                        _ = tokio::time::sleep(Duration::from_millis(5)) => {}
                    }
                    let rows = local.list_audits(&db::RequestAuditFilters::default()).await.unwrap();
                    if server.calls().len() == 1 && rows.len() == 1 && rows[0].status == "running" { break; }
                }
            }).await.unwrap();
            assert_eq!(server.closed(),0);
            drop(request);
            let rows = tokio::time::timeout(Duration::from_secs(3),async {
                loop {
                    let rows = local.list_audits(&db::RequestAuditFilters::default()).await.unwrap();
                    if rows.len() == 1 && rows[0].status == "cancelled" { break rows; }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            }).await.expect("Dropped probe left its audit running");
            let row = &rows[0];
            assert_eq!(row.provider_account_id.as_deref(),Some("qwen-probe"));
            assert_eq!(row.route_trace.as_ref().unwrap()["realCredentialRef"],"other");
            assert_eq!(row.resolved_model.as_deref(),Some("qwen-synthetic"));
            assert!(row.total_tokens.is_none() && row.prompt_tokens.is_none());
            assert!(row.completed_at.is_some());
            assert!(!row.error_summary.as_deref().unwrap_or_default().contains("synthetic-other-key"));
            assert!(local.list_model_states(db::CredentialModelStateFilters::default()).await.unwrap().is_empty());
            tokio::time::timeout(Duration::from_secs(3),async {
                while server.closed() == 0 { tokio::time::sleep(Duration::from_millis(5)).await; }
            }).await.unwrap();
            let calls = server.calls();
            assert_eq!(calls.len(),1,"No retries, fallback accounts or continued plan cases");
            assert_eq!(calls[0].authorization,"Bearer synthetic-other-key");
            assert!(calls[0].uri.contains("workspace-b.ap-southeast-1.maas.aliyuncs.com"));
            assert_eq!(calls[0].body["model"],"qwen-synthetic");
            server.stop().await;
            drop(fixture);
            tokio::time::timeout(Duration::from_secs(3),local.close()).await.unwrap();
        }
    }).await.unwrap();
}
