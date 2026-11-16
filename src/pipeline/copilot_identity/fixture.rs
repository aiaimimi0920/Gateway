//! Real local-key storage and synthetic route identities; routing still follows feature eligibility.
use super::*;
use crate::{auth::adapter::AuthRequest, db::UpsertAccessKeyInput, local_runtime::LocalRuntime};
#[path = "../../../tests/console_contract_support/mod.rs"]
pub(super) mod support;

pub(super) struct Fixture {
    pub console: support::ConsoleStateFixture,
    pub local: LocalRuntime,
    pub ctx: PipelineContext,
    pub candidate: RouteCandidate,
    pub key: String,
    root: std::path::PathBuf,
}

pub(super) fn input(metadata: serde_json::Value) -> UpsertAccessKeyInput {
    UpsertAccessKeyInput {
        owner_type: "user".into(),
        owner_id: "synthetic".into(),
        resolved_project_id: "local".into(),
        resolved_tenant_id: "local".into(),
        key_kind: "normal".into(),
        public_key_prefix: "sk-gw".into(),
        display_name: "Copilot identity test".into(),
        expires_at: None,
        bundle_ids: vec![],
        metadata: Some(metadata),
    }
}

impl Fixture {
    pub async fn new() -> Self {
        Self::with_runtime(false).await
    }

    pub async fn with_runtime(with_runtime: bool) -> Self {
        let document = serde_json::from_value(json!({"providers":[{
            "id":"copilot", "base_url":"", "auth_mode":"copilot_sdk",
            "explicit_credentials_only":true,"supported_models":["sdk-model"],
            "credentials":[{"id":"account", "enabled":true,
            "copilot_github_token":"synthetic-identity-secret"}]}],
            "aliases":{"public-alias":"sdk-model"},
            "account_groups":[{"id":"sdk-group","name":"SDK", "provider_credential_ids":["account"]}]
        })).unwrap();
        let mut console = support::ConsoleStateFixture::new(document, with_runtime);
        let root = std::env::temp_dir().join(format!(
            "gateway-local-copilot-identity-{}",
            uuid::Uuid::new_v4()
        ));
        let local = LocalRuntime::open(&root).await.unwrap();
        let key = local.save_access_key(None, input(json!({
            "scopes":["chat.completions"],"models":["public-alias"],"providerIds":["copilot"]
        }))).await.unwrap();
        let session = local
            .authenticate_access_key(&AuthRequest {
                authorization: Some(format!("Bearer {}", key.token.as_deref().unwrap())),
                api_key: None,
                path: "/v1/chat/completions".into(),
                method: "POST".into(),
            })
            .await
            .unwrap()
            .unwrap();
        Arc::get_mut(&mut console.state).unwrap().local_runtime = Some(local.clone());
        let mut ctx = PipelineContext::new(
            crate::protocol::openai::normalize_chat_completions(
                json!({"model":"public-alias","messages":[{"role":"user","content":"hello"}]}),
            )
            .unwrap(),
            None,
        );
        ctx.session = Some(session);
        ctx.cash_route_snapshot = Some(console.state.route_config.snapshot());
        // Construct a trusted candidate for identity-unit testing only; production routing stays empty.
        let generic = crate::routing::config::RouteConfigStore::from_document(
            serde_json::from_value(json!({"providers":[{"id":"copilot",
                "base_url":"https://example.invalid", "supported_models":["sdk-model"],
                "credentials":[{"id":"account","api_key":"generic-fixture"}]}]}))
            .unwrap(),
        )
        .unwrap();
        let mut candidate = generic
            .snapshot()
            .resolve_candidates(Some("sdk-model"))
            .remove(0);
        candidate.payload = console.state.route_config.snapshot().get_providers()[0]
            .credential_pool[0]
            .payload
            .clone();
        Self {
            console,
            local,
            ctx,
            candidate,
            key: key.id,
            root,
        }
    }

    pub fn bind(&self) -> CopilotIdentity<'_> {
        CopilotIdentity::bind(&self.console.state, &self.ctx, &self.candidate, "sdk-model").unwrap()
    }

    pub fn replace(&self, change: impl FnOnce(&mut crate::routing::config::RouteConfigYaml)) {
        let mut document = self
            .console
            .state
            .route_config
            .snapshot()
            .document()
            .clone();
        change(&mut document);
        self.console
            .state
            .route_config
            .replace_document(document)
            .unwrap();
    }

    pub async fn close(self) {
        self.console
            .state
            .shutdown
            .request("synthetic identity test completed");
        self.local.close().await;
        crate::local_runtime::test_support::remove_test_root(&self.root).await;
    }
}
