use crate::config::Config;
use crate::state::ProviderCredentialFolderSyncRuntime;

use super::{folder_sync_root_available, ProviderCredentialFolderSyncStatusView};

// Only folder-sync configuration crosses the admitted operation boundary.
#[derive(Clone)]
pub(in crate::provider_credential_folder_sync) struct FolderSyncStatusConfig {
    pub enabled: bool,
    root_dir: Option<String>,
    interval_seconds: Option<u64>,
    watch_enabled: bool,
    watch_debounce_millis: Option<u64>,
    pub import_enabled: bool,
    pub export_enabled: bool,
    pub delete_missing: bool,
}

impl FolderSyncStatusConfig {
    pub fn new(config: &Config, enabled: bool) -> Self {
        let configured = folder_sync_root_available(config);
        let watch_enabled = configured && config.provider_credential_folder_sync_watch_enabled;
        Self {
            enabled: configured && enabled,
            root_dir: config.provider_credential_folder_sync_root_dir.clone(),
            interval_seconds: configured
                .then_some(config.provider_credential_folder_sync_interval_secs),
            watch_enabled,
            watch_debounce_millis: watch_enabled
                .then_some(config.provider_credential_folder_sync_watch_debounce_millis),
            import_enabled: config.provider_credential_folder_sync_import_enabled,
            export_enabled: config.provider_credential_folder_sync_export_enabled,
            delete_missing: config.provider_credential_folder_sync_delete_missing,
        }
    }

    pub fn apply_current(
        &self,
        status: &mut ProviderCredentialFolderSyncStatusView,
        runtime: &ProviderCredentialFolderSyncRuntime,
    ) {
        let mut current = self.clone();
        // A run can outlive a disable; project fresh enablement on every CAS attempt.
        current.enabled = self.interval_seconds.is_some() && runtime.enabled();
        current.apply(status);
    }

    pub fn apply(self, status: &mut ProviderCredentialFolderSyncStatusView) {
        status.enabled = self.enabled;
        self.apply_observer(status);
    }

    pub fn apply_observer(&self, status: &mut ProviderCredentialFolderSyncStatusView) {
        self.apply_static(status);
        if !self.watch_enabled {
            status.watch_running = false;
        }
    }

    pub fn apply_shared_enabled(
        &self,
        status: &mut ProviderCredentialFolderSyncStatusView,
        enabled: bool,
    ) {
        // The Redis override is the cross-process management value; local runtime
        // state is only a fallback when that key has not been initialized.
        status.enabled = self.interval_seconds.is_some() && enabled;
        self.apply_observer(status);
    }

    pub fn apply_static(&self, status: &mut ProviderCredentialFolderSyncStatusView) {
        status.root_dir = self.root_dir.clone();
        status.interval_seconds = self.interval_seconds;
        status.watch_enabled = self.watch_enabled;
        status.watch_debounce_millis = self.watch_debounce_millis;
        status.import_enabled = self.import_enabled;
        status.export_enabled = self.export_enabled;
        status.delete_missing = self.delete_missing;
    }
}

#[cfg(test)]
mod tests {
    use super::{FolderSyncStatusConfig, ProviderCredentialFolderSyncStatusView};

    fn config(enabled: bool, watch_enabled: bool) -> FolderSyncStatusConfig {
        FolderSyncStatusConfig {
            enabled,
            root_dir: Some("C:/folder-sync".into()),
            interval_seconds: Some(30),
            watch_enabled,
            watch_debounce_millis: Some(750),
            import_enabled: true,
            export_enabled: false,
            delete_missing: true,
        }
    }

    #[test]
    fn static_projection_preserves_runtime_owned_fields() {
        let mut status = ProviderCredentialFolderSyncStatusView {
            enabled: true,
            watch_running: true,
            ..Default::default()
        };

        config(false, true).apply_static(&mut status);

        assert!(status.enabled);
        assert!(status.watch_running);
        assert_eq!(status.root_dir.as_deref(), Some("C:/folder-sync"));
        assert_eq!(status.interval_seconds, Some(30));
    }

    #[test]
    fn full_projection_owns_enabled_and_disabled_watch_state() {
        let mut status = ProviderCredentialFolderSyncStatusView {
            enabled: true,
            watch_running: true,
            ..Default::default()
        };

        config(false, false).apply(&mut status);

        assert!(!status.enabled);
        assert!(!status.watch_running);
    }

    #[test]
    fn observer_projection_preserves_enabled_but_clears_disabled_watch_state() {
        let mut status = ProviderCredentialFolderSyncStatusView {
            enabled: true,
            watch_running: true,
            ..Default::default()
        };

        config(false, false).apply_observer(&mut status);

        assert!(status.enabled);
        assert!(!status.watch_running);
    }

    #[test]
    fn shared_enabled_projection_uses_authoritative_override() {
        let mut status = ProviderCredentialFolderSyncStatusView {
            enabled: true,
            watch_running: true,
            ..Default::default()
        };

        config(true, true).apply_shared_enabled(&mut status, false);

        assert!(!status.enabled);
        assert!(status.watch_running);
    }
}
