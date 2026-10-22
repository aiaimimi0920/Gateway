//! Read-only startup policy shared by the API runtime and desktop launcher.
//! Missing settings retain legacy behavior; malformed settings fail closed.
use serde::Deserialize;
use std::{
    fs::File,
    io::{self, Read},
    path::{Path, PathBuf},
};

const MAX_CONFIG_BYTES: u64 = 64 * 1024;
pub const CONFIG_FILE_NAME: &str = "gateway.json";

#[derive(Clone, Debug, Deserialize)]
#[serde(default)]
pub struct StartupConfig {
    pub enable_management_ui: bool,
    pub routing_policy: Option<serde_json::Value>,
}

impl Default for StartupConfig {
    fn default() -> Self {
        Self {
            enable_management_ui: true,
            routing_policy: None,
        }
    }
}

impl StartupConfig {
    pub fn config_path() -> io::Result<PathBuf> {
        let path = match crate::env_path("GATEWAY_CONFIG_FILE") {
            Some(path) => path,
            None => crate::selected_root()?.join(CONFIG_FILE_NAME),
        };
        if !path.is_absolute() {
            return Err(io::Error::other(
                "GATEWAY_CONFIG_FILE must be an absolute path",
            ));
        }
        Ok(path)
    }

    pub fn load() -> io::Result<Self> {
        Self::from_path(&Self::config_path()?)
    }

    pub fn from_path(path: &Path) -> io::Result<Self> {
        let file = match File::open(path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(error),
        };
        // Bound the actual read, not only metadata: concurrent writes must not bypass it.
        let mut bytes = Vec::new();
        file.take(MAX_CONFIG_BYTES + 1).read_to_end(&mut bytes)?;
        if bytes.len() as u64 > MAX_CONFIG_BYTES {
            return Err(io::Error::other(
                "Gateway startup configuration exceeds 64 KiB",
            ));
        }
        serde_json::from_slice(&bytes).map_err(|error| {
            io::Error::new(
                io::ErrorKind::InvalidData,
                format!("Invalid Gateway startup configuration: {error}"),
            )
        })
    }

    pub fn management_ui_enabled(&self, compiled_with_ui: bool) -> bool {
        compiled_with_ui && self.enable_management_ui
    }
}
