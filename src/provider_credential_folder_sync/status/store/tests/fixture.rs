use std::time::Duration;

use deadpool_redis::Pool;
use redis::{AsyncCommands, Commands};

use crate::provider_credential_folder_sync::ProviderCredentialFolderSyncStatusView;
use crate::redis::keys;

pub(super) struct Fixture {
    pub pool: Pool,
    writer: redis::Connection,
}

impl Fixture {
    pub async fn new() -> Self {
        let run_id = std::env::var("GATEWAY_FOLDER_STARTUP_TEST_RUN_ID")
            .expect("dedicated Redis run guard required");
        assert!(run_id.len() == 32 && run_id.bytes().all(|b| b.is_ascii_hexdigit()));
        let url = std::env::var("GATEWAY_FOLDER_STARTUP_TEST_REDIS_URL")
            .expect("dedicated Redis URL required");
        let pool = crate::redis::pool::create_pool(&url).unwrap();
        let mut conn = pool.get().await.unwrap();
        let guard: Option<String> = conn.get("gw:folder-startup:contract_run").await.unwrap();
        assert_eq!(
            guard.as_deref(),
            Some(run_id.as_str()),
            "isolated Redis guard"
        );
        for key in [
            keys::provider_credential_folder_sync_status_key(),
            keys::provider_credential_folder_sync_enabled_key(),
            keys::legacy_provider_credential_folder_sync_status_key().to_string(),
            keys::legacy_provider_credential_folder_sync_enabled_key().to_string(),
        ] {
            let _: usize = conn.del(key).await.unwrap();
        }
        drop(conn);
        let writer = redis::Client::open(url).unwrap().get_connection().unwrap();
        writer
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        writer
            .set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        Self { pool, writer }
    }

    pub fn replace(&mut self, status: &ProviderCredentialFolderSyncStatusView) {
        self.replace_raw(&serde_json::to_string(status).unwrap());
    }

    pub fn replace_raw(&mut self, raw: &str) {
        // A separate real Redis connection forces a competing commit between the
        // tested update's GET and SET/CAS. Only fixture I/O is synchronous/bounded.
        let _: () = self
            .writer
            .set(keys::provider_credential_folder_sync_status_key(), raw)
            .unwrap();
    }

    pub fn replace_enabled(&mut self, enabled: bool) {
        let _: () = self
            .writer
            .set(
                keys::provider_credential_folder_sync_enabled_key(),
                serde_json::to_string(&enabled).unwrap(),
            )
            .unwrap();
    }

    pub fn replace_legacy(&mut self, status: &ProviderCredentialFolderSyncStatusView) {
        let raw = serde_json::to_string(status).unwrap();
        let _: () = self
            .writer
            .set(
                keys::legacy_provider_credential_folder_sync_status_key(),
                raw,
            )
            .unwrap();
    }

    pub fn replace_legacy_enabled(&mut self, enabled: bool) {
        let _: () = self
            .writer
            .set(
                keys::legacy_provider_credential_folder_sync_enabled_key(),
                serde_json::to_string(&enabled).unwrap(),
            )
            .unwrap();
    }

    pub async fn raw(&self) -> String {
        let mut conn = self.pool.get().await.unwrap();
        conn.get(keys::provider_credential_folder_sync_status_key())
            .await
            .unwrap()
    }

    pub async fn enabled_raw(&self) -> Option<String> {
        let mut conn = self.pool.get().await.unwrap();
        conn.get(keys::provider_credential_folder_sync_enabled_key())
            .await
            .unwrap()
    }

    pub async fn status(&self) -> ProviderCredentialFolderSyncStatusView {
        serde_json::from_str(&self.raw().await).unwrap()
    }
}
