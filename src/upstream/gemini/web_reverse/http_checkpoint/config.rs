//! Explicit local-only opt-in; keys never enter route documents, Debug or diagnostics.
use base64::{engine::general_purpose::STANDARD, Engine};
use std::path::PathBuf;

use super::{error, Config};
use crate::error::GatewayError;

impl Config {
    pub(super) fn from_values(
        driver: &str,
        directory: &str,
        encoded: &str,
    ) -> Result<Option<Self>, GatewayError> {
        match driver.trim().to_ascii_lowercase().as_str() {
            "disabled" => return Ok(None),
            "local" => (),
            _ => return Err(error("config_invalid")),
        }
        let bytes = STANDARD
            .decode(encoded)
            .map_err(|_| error("config_invalid"))?;
        let key: [u8; 32] = bytes.try_into().map_err(|_| error("config_invalid"))?;
        let directory = PathBuf::from(directory);
        if STANDARD.encode(key) != encoded
            || !directory.is_absolute()
            || directory.as_os_str().to_string_lossy().starts_with("\\\\")
            || !local_drive(&directory)
        {
            return Err(error("config_invalid"));
        }
        Ok(Some(Self { directory, key }))
    }

    pub(super) fn from_env() -> Result<Option<Self>, GatewayError> {
        let read = |name: &str, fallback: &str| match std::env::var(name) {
            Ok(value) => Ok(value),
            Err(std::env::VarError::NotPresent) => Ok(fallback.to_owned()),
            Err(_) => Err(error("config_invalid")),
        };
        let driver = read("GEMINI_WEB_HTTP_AUTH_CHECKPOINT_DRIVER", "disabled")?;
        if driver.trim().eq_ignore_ascii_case("disabled") {
            return Ok(None);
        }
        Self::from_values(
            &driver,
            &read("GEMINI_WEB_HTTP_AUTH_CHECKPOINT_DIR", "")?,
            &read("GEMINI_WEB_HTTP_AUTH_CHECKPOINT_KEY", "")?,
        )
    }
}

fn local_drive(directory: &std::path::Path) -> bool {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let Some(Component::Prefix(prefix)) = directory.components().next() else {
            return false;
        };
        let Prefix::Disk(letter) = prefix.kind() else {
            return false;
        };
        let root = [letter as u16, b':' as u16, b'\\' as u16, 0];
        // The stack buffer is a valid NUL-terminated drive root for this synchronous API.
        let kind = unsafe { windows_sys::Win32::Storage::FileSystem::GetDriveTypeW(root.as_ptr()) };
        matches!(kind, 2 | 3 | 6) // removable, fixed or RAM; never mapped network drives.
    }
    #[cfg(not(windows))]
    {
        let _ = directory;
        true
    }
}
