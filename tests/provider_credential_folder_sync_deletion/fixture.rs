use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use neuro_gateway::console::{ConsoleConfig, ConsoleConfigValues};
use neuro_gateway::routing::config::RouteConfigStore;
use neuro_gateway::state::AppState;

pub struct Directory {
    pub root: PathBuf,
    pub outside: PathBuf,
    base: PathBuf,
    links: Vec<PathBuf>,
}

impl Directory {
    pub fn new() -> Self {
        let base =
            std::env::temp_dir().join(format!("gateway-folder-delete-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&base).unwrap();
        let directory = Self {
            root: base.join("root"),
            outside: base.join("outside"),
            base,
            links: Vec::new(),
        };
        fs::create_dir(&directory.root).unwrap();
        fs::create_dir(&directory.outside).unwrap();
        directory
    }

    pub fn state(&self, enabled: bool) -> Arc<AppState> {
        let console = ConsoleConfig::from_values(ConsoleConfigValues {
            state_dir: Some(self.base.join("console")),
            routes_file: Some(self.base.join("console/routes.yaml")),
            ..Default::default()
        })
        .unwrap();
        let mut config = super::config::test_config(console);
        config.provider_credential_folder_sync_enabled = enabled;
        config.provider_credential_folder_sync_root_dir =
            Some(self.root.to_str().unwrap().to_owned());
        super::support::build_test_app_state(config, RouteConfigStore::new(), None)
    }

    #[cfg(any(unix, windows))]
    pub fn directory_link(&mut self, target: PathBuf) {
        let link = self.root.join("linked");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let output = std::process::Command::new("cmd")
                .args(["/C", "mklink", "/J"])
                .arg(&link)
                .arg(&target)
                .creation_flags(0x08000000)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "junction creation failed: {output:?}"
            );
        }
        self.links.push(link);
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        // Unlink fixtures before recursive cleanup, including failed assertions.
        for link in &self.links {
            #[cfg(windows)]
            let _ = fs::remove_dir(link);
            #[cfg(not(windows))]
            let _ = fs::remove_file(link);
        }
        let temp = std::env::temp_dir();
        if self.base.parent() == Some(temp.as_path())
            && self
                .base
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("gateway-folder-delete-")
        {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
}
