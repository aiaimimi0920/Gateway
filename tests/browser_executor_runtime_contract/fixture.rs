use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use neuro_gateway::browser_executor_runtime::{
    acquire_browser_capability_lease, release_browser_capability_lease,
    upsert_browser_capability_slot, BrowserCapabilityLeaseAcquireInput,
    BrowserCapabilityLeaseReleaseInput, BrowserCapabilityLeaseView, BrowserCapabilitySlotStatus,
    BrowserCapabilitySlotUpsertInput, BrowserCapabilitySlotView,
};
use neuro_gateway::console::{ConsoleConfig, ConsoleConfigValues};
use neuro_gateway::error::GatewayError;
use neuro_gateway::redis::keys;
use neuro_gateway::routing::candidate::ProviderExecutionMode;
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::state::AppState;
use redis::AsyncCommands;

use crate::console_contract_support::test_config;
use crate::support::build_test_app_state;

pub struct LeaseFixture {
    pub state: Arc<AppState>,
    pub slot_id: String,
    pub account_id: String,
    _directory: FixtureDirectory,
}

struct FixtureDirectory(PathBuf);

impl LeaseFixture {
    pub async fn new() -> Self {
        let redis_url = std::env::var("GATEWAY_BROWSER_EXECUTOR_TEST_REDIS_URL")
            .expect("set the dedicated Redis fixture URL");
        let run_id = std::env::var("GATEWAY_BROWSER_EXECUTOR_TEST_RUN_ID")
            .expect("set the dedicated Redis fixture guard");
        assert!(!run_id.is_empty());
        let id = uuid::Uuid::new_v4().simple().to_string();
        let directory = std::env::temp_dir().join(format!("gateway-browser-lease-{id}"));
        fs::create_dir(&directory).expect("create exclusively owned fixture directory");
        let directory = FixtureDirectory(directory);
        let mut config = test_config();
        config.redis_url = redis_url;
        config.console = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(directory.0.join("state")),
            routes_file: Some(directory.0.join("routes.yaml")),
            ..ConsoleConfigValues::default()
        })
        .unwrap();
        let fixture = Self {
            state: build_test_app_state(config, RouteConfigStore::new(), None),
            slot_id: format!("slot-{id}"),
            account_id: format!("account-{id}"),
            _directory: directory,
        };
        let mut conn = fixture.state.redis_pool.get().await.unwrap();
        let guard: Option<String> = conn.get("gw:browser_executor:contract_run").await.unwrap();
        assert_eq!(
            guard.as_deref(),
            Some(run_id.as_str()),
            "isolated Redis guard"
        );
        drop(conn);
        fixture.warm_slot().await;
        fixture
    }

    pub async fn warm_slot(&self) -> BrowserCapabilitySlotView {
        upsert_browser_capability_slot(
            &self.state,
            BrowserCapabilitySlotUpsertInput {
                slot_id: self.slot_id.clone(),
                node_id: format!("node-{}", self.slot_id),
                provider_account_id: self.account_id.clone(),
                adapter: "lumalabs_compatible".to_string(),
                endpoint_kind: "images".to_string(),
                execution_mode: ProviderExecutionMode::BrowserBacked,
                status: Some(BrowserCapabilitySlotStatus::Warm),
                runtime_state_object_key: Some("fixture/runtime-state".to_string()),
                account_name: Some("Fixture account".to_string()),
                last_warm_at: Some("2026-09-12T00:00:00Z".to_string()),
                last_used_at: None,
                last_failure_at: None,
                degradation_reasons: None,
                ttl_seconds: None,
            },
        )
        .await
        .unwrap()
    }

    pub async fn acquire(
        &self,
        lease_ttl_seconds: Option<u64>,
    ) -> Result<BrowserCapabilityLeaseView, GatewayError> {
        acquire_browser_capability_lease(
            &self.state,
            BrowserCapabilityLeaseAcquireInput {
                provider_account_id: self.account_id.clone(),
                endpoint_kind: "images".to_string(),
                execution_mode: Some(ProviderExecutionMode::BrowserBacked),
                request_audit_id: Some("fixture-audit".to_string()),
                project_id: Some("fixture-project".to_string()),
                lease_ttl_seconds,
            },
        )
        .await
    }

    pub async fn release(
        &self,
        lease: &BrowserCapabilityLeaseView,
        reason: Option<&str>,
    ) -> Result<BrowserCapabilityLeaseView, GatewayError> {
        release_browser_capability_lease(
            &self.state,
            BrowserCapabilityLeaseReleaseInput {
                lease_id: lease.lease_id.clone(),
                release_reason: reason.map(str::to_string),
            },
        )
        .await
    }

    pub async fn slot_raw(&self) -> String {
        let mut conn = self.state.redis_pool.get().await.unwrap();
        conn.get(keys::browser_executor_slot_key(&self.slot_id))
            .await
            .unwrap()
    }

    pub async fn slot(&self) -> BrowserCapabilitySlotView {
        serde_json::from_str(&self.slot_raw().await).unwrap()
    }

    pub async fn lock(&self) -> Option<String> {
        let mut conn = self.state.redis_pool.get().await.unwrap();
        conn.get(keys::browser_executor_slot_lease_lock_key(&self.slot_id))
            .await
            .unwrap()
    }

    pub async fn expire_lock(&self) {
        let mut conn = self.state.redis_pool.get().await.unwrap();
        let _: usize = conn
            .del(keys::browser_executor_slot_lease_lock_key(&self.slot_id))
            .await
            .unwrap();
    }
}

impl Drop for FixtureDirectory {
    fn drop(&mut self) {
        // This private path was created exclusively by this fixture.
        let _ = fs::remove_dir_all(&self.0);
    }
}
