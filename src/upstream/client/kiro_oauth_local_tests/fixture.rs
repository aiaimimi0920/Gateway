//! One real local runtime and its synthetic OIDC owner; all writes use normal management APIs.
use super::*;
use crate::config::Config;

pub(super) struct Fixture {
    pub server: LocalServer,
    pub oidc: issuer::Mock,
    pub config: Config,
    pub credential: String,
    pub key_id: String,
    pub token: String,
    pub grant: String,
}
impl Fixture {
    pub async fn new(upstream: &wire::Server) -> Self {
        let root = std::env::temp_dir().join(format!("kiro-oauth-local-{}", uuid::Uuid::new_v4()));
        let config = config(&root);
        let oidc = issuer::Mock::new(vec![
            issuer::registration(),
            issuer::device(),
            issuer::tokens(true, true),
        ])
        .await;
        let server =
            LocalServer::start_with_oidc(config.clone(), upstream, Some(oidc.origin.clone()), None)
                .await;
        let grant = server.grant().await;
        let (status, saved) = server.manage("PUT", ROUTE,
            json!({"document":document(),"secretPatches":[],"expectedRevision":server.revision()}), None).await;
        assert_eq!(status, 200, "{saved}");
        assert_eq!(
            oidc.count(),
            0,
            "Creating an empty pool never authorizes or generates"
        );
        let credential = login(&server, &grant, None).await;
        assert_eq!(oidc.count(), 3);
        let (status, created) = server
            .manage(
                "POST",
                "/v1/internal/gateway/access/keys",
                json!({"ownerType":"user","ownerId":"oauth-fixture","resolvedProjectId":"local",
                "resolvedTenantId":"local","keyKind":"normal","publicKeyPrefix":"sk-gw",
                "displayName":"Kiro OAuth local","metadata":{"models":[ALIAS]}}),
                None,
            )
            .await;
        assert_eq!(status, 200, "{created}");
        let key_id = created["id"].as_str().unwrap().to_owned();
        let token = created["token"].as_str().unwrap().to_owned();
        let (status, balance) = server
            .manage(
                "POST",
                &format!("/v1/internal/gateway/access/keys/{key_id}/balances/adjust"),
                json!({"balanceMode":"message_prepaid","totalMessages":10,"remainingMessages":10}),
                None,
            )
            .await;
        assert_eq!(status, 200, "{balance}");
        Self {
            server,
            oidc,
            config,
            credential,
            key_id,
            token,
            grant,
        }
    }
    pub async fn document(&self) -> Value {
        let (status, value) = self.server.manage("GET", ROUTE, Value::Null, None).await;
        assert_eq!(status, 200, "{value}");
        assert_redacted(&value);
        value["routeConfig"]["document"].clone()
    }
    pub async fn save(
        &self,
        document: Value,
        operation: &str,
        grant: Option<&str>,
    ) -> (u16, Value) {
        let patches:Vec<_>=["api_key","refresh_token","kiro_oauth/client_secret"].iter()
            .map(|field|json!({"path":format!("/providers/0/credentials/0/{field}"),"operation":operation})).collect();
        self.server.manage("PUT",ROUTE,json!({"document":document,"secretPatches":patches,"expectedRevision":self.server.revision()}),grant).await
    }
    pub async fn audits(&self, count: usize) -> Vec<db::GatewayRequestAuditView> {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let rows = self
                    .server
                    .state
                    .local_runtime
                    .as_ref()
                    .unwrap()
                    .list_audits(&db::RequestAuditFilters::default())
                    .await
                    .unwrap();
                if rows.len() == count
                    && rows
                        .iter()
                        .all(|r| !matches!(r.status.as_str(), "started" | "running"))
                {
                    return rows;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap()
    }
    pub fn assert_identity(&self, rows: &[db::GatewayRequestAuditView], status: &str) {
        for row in rows {
            assert_eq!(row.status, status);
            assert_eq!(row.provider_account_id.as_deref(), Some(PROVIDER));
            assert_eq!(row.requested_model.as_deref(), Some(ALIAS));
            assert_eq!(row.resolved_model.as_deref(), Some(MODEL));
            assert_eq!(
                row.route_trace.as_ref().unwrap()["realCredentialRef"],
                self.credential
            );
            assert_redacted(&serde_json::to_value(row).unwrap());
        }
    }
    pub async fn stop(self) {
        self.server.stop().await;
        self.oidc.stop().await;
    }
}
pub(super) async fn login(server: &LocalServer, grant: &str, credential: Option<&str>) -> String {
    let mut body = json!({"providerId":PROVIDER,"expectedRevision":server.revision(),
        "directory":{"authMethod":"builder_id"},"scopes":["codewhisperer:conversations"],"machineId":"a".repeat(64)});
    if let Some(id) = credential {
        body["credentialId"] = json!(id);
    }
    let (status, created) = server.manage("POST", AUTH, body, Some(grant)).await;
    assert_eq!(status, 200, "{created}");
    let session = created["session"].clone();
    let id = session["id"].as_str().unwrap();
    wait(server, id, "waiting_user").await;
    for (action, expected) in [("poll", "ready"), ("import", "succeeded")] {
        let (status, value) = server
            .manage(
                "POST",
                &format!("{AUTH}/{id}/{action}"),
                json!({"expectedRevision":session["expectedRevision"]}),
                Some(grant),
            )
            .await;
        assert_eq!(status, 200, "{value}");
        assert_redacted(&value);
        wait(server, id, expected).await;
    }
    let view = wait(server, id, "succeeded").await;
    view["targetCredentialId"].as_str().unwrap().to_owned()
}
async fn wait(server: &LocalServer, id: &str, expected: &str) -> Value {
    tokio::time::timeout(Duration::from_secs(15), async {
        loop {
            let (status, value) = server
                .manage("GET", &format!("{AUTH}/{id}"), Value::Null, None)
                .await;
            assert_eq!(status, 200, "{value}");
            assert_redacted(&value);
            let session = &value["session"];
            if session["status"] == expected {
                return session.clone();
            }
            assert!(
                !matches!(
                    session["status"].as_str(),
                    Some("failed" | "cancelled" | "expired" | "save_unconfirmed")
                ),
                "{value}"
            );
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap()
}
pub(super) fn assert_redacted(value: &Value) {
    let text = value.to_string();
    for secret in [
        "synthetic-console-access",
        "synthetic-console-refresh",
        "synthetic-console-secret",
        "synthetic-device-secret",
        "private-upstream-detail",
    ] {
        assert!(
            !text.contains(secret),
            "Secret leaked to management/public audit"
        );
    }
}
