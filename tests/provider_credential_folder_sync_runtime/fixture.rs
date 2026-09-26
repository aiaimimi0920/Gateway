use std::fs;
use std::future::Future;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use neuro_gateway::console::{ConsoleConfig, ConsoleConfigValues};
use neuro_gateway::provider_credential_folder_sync::{
    start_folder_sync_task, ProviderCredentialFolderSyncStatusView,
};
use neuro_gateway::redis::keys;
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::state::AppState;
use redis::AsyncCommands;

pub struct Fixture {
    pub state: Arc<AppState>,
    pub root: PathBuf,
    directory: Arc<Directory>,
    task: Option<tokio::task::JoinHandle<()>>,
}

struct Directory(PathBuf);

impl Fixture {
    pub async fn new(enabled: bool, watch: bool, configured: bool) -> Self {
        let run_id = std::env::var("GATEWAY_FOLDER_STARTUP_TEST_RUN_ID")
            .expect("dedicated Redis run guard required");
        assert!(run_id.len() == 32 && run_id.bytes().all(|b| b.is_ascii_hexdigit()));
        let redis_url = std::env::var("GATEWAY_FOLDER_STARTUP_TEST_REDIS_URL")
            .expect("dedicated Redis URL required");
        let base = std::env::temp_dir().join(format!(
            "gateway-folder-startup-{run_id}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir(&base).unwrap();
        let directory = Arc::new(Directory(base));
        let root = directory.0.join("credentials");
        let console = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(directory.0.join("state")),
            routes_file: Some(directory.0.join("routes.yaml")),
            ..ConsoleConfigValues::default()
        })
        .unwrap();
        let mut config = crate::config::test_config(console);
        config.redis_url = redis_url;
        config.provider_credential_folder_sync_enabled = enabled;
        config.provider_credential_folder_sync_watch_enabled = watch;
        config.provider_credential_folder_sync_root_dir =
            configured.then(|| root.to_string_lossy().into_owned());
        // An immediate activation cannot be mistaken for the periodic fallback.
        config.provider_credential_folder_sync_interval_secs = 3600;
        let state = crate::support::build_test_app_state(config, RouteConfigStore::new(), None);
        let mut connection = state.redis_pool.get().await.unwrap();
        let guard: Option<String> = connection
            .get("gw:folder-startup:contract_run")
            .await
            .unwrap();
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
            let _: usize = connection.del(key).await.unwrap();
        }
        drop(connection);
        Self {
            state,
            root,
            directory,
            task: None,
        }
    }

    pub fn start(&mut self) {
        self.start_future(start_folder_sync_task(self.state.clone()));
    }

    pub fn take_task(&mut self) -> tokio::task::JoinHandle<()> {
        self.task.take().expect("folder sync task started")
    }

    pub fn start_future(&mut self, future: impl Future<Output = ()> + Send + 'static) {
        assert!(self.task.is_none());
        let directory = self.directory.clone();
        self.task = Some(tokio::spawn(async move {
            // The directory outlives watcher destruction even on a test panic.
            let _directory = directory;
            future.await;
        }));
    }

    pub async fn status(&self) -> Option<ProviderCredentialFolderSyncStatusView> {
        let mut connection = self.state.redis_pool.get().await.unwrap();
        let raw: Option<String> = connection
            .get(keys::provider_credential_folder_sync_status_key())
            .await
            .unwrap();
        raw.map(|raw| serde_json::from_str(&raw).unwrap())
    }

    pub async fn wait_disabled_status(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if self
                    .status()
                    .await
                    .is_some_and(|s| !s.enabled && !s.watch_running)
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("disabled startup status");
        assert!(
            !self.root.exists(),
            "disabled startup must not create the sync root"
        );
    }

    pub async fn wait_root(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            while !self.root.is_dir() {
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("runtime enable must create the sync root without a restart");
    }

    pub async fn wait_enabled_status_without_watchers(&self) {
        tokio::time::timeout(Duration::from_secs(3), async {
            loop {
                if self
                    .status()
                    .await
                    .is_some_and(|s| s.enabled && !s.watch_running)
                {
                    return;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await
        .expect("enabled runtime status persisted without watchers");
    }

    pub async fn finish(mut self) {
        let task = self.task.take().unwrap();
        task.abort();
        let result = tokio::time::timeout(Duration::from_secs(3), task)
            .await
            .unwrap();
        assert!(
            result.is_err_and(|error| error.is_cancelled()),
            "runtime remains owned until cancellation"
        );
        assert_eq!(Arc::strong_count(&self.state), 1, "task released AppState");
        let base = self.directory.0.clone();
        drop(self);
        assert!(
            !base.exists(),
            "owned temporary directory removed after task cleanup"
        );
    }

    pub async fn finish_gracefully(mut self) {
        self.state
            .shutdown
            .request("folder watcher shutdown regression");
        let mut task = self.task.take().unwrap();
        let stopped = tokio::time::timeout(Duration::from_secs(3), &mut task).await;
        if stopped.is_err() {
            // Rescue the old task before reporting failure and removing its directory.
            task.abort();
            let _ = task.await;
        }
        assert!(
            matches!(stopped, Ok(Ok(()))),
            "shutdown request must finish the folder task"
        );
        assert_eq!(
            Arc::strong_count(&self.state),
            1,
            "graceful shutdown released AppState"
        );
        let base = self.directory.0.clone();
        drop(self);
        assert!(
            !base.exists(),
            "directory removed after graceful task cleanup"
        );
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        let temp = std::env::temp_dir();
        assert!(self.0.parent() == Some(temp.as_path()));
        assert!(self
            .0
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gateway-folder-startup-"));
        if let Err(error) = fs::remove_dir_all(&self.0) {
            eprintln!("folder startup fixture cleanup failed: {error}");
        }
    }
}
