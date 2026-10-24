use serde::{Deserialize, Serialize};
use std::path::Path;
use tauri::Url;

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ConnectionMode {
    #[default]
    Local,
    Existing,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionSettings {
    pub mode: ConnectionMode,
    #[serde(default)]
    pub server_url: String,
}

pub fn console_url(value: &str) -> Result<Url, String> {
    if value.len() > 2048 {
        return Err("Gateway URL is too long".into());
    }
    let mut url = Url::parse(value.trim()).map_err(|_| "Enter an HTTP(S) Gateway address")?;
    if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
        return Err("Gateway address must use HTTP or HTTPS".into());
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("Enter the server address without credentials, query or fragment".into());
    }
    if !matches!(url.path(), "/" | "/ui" | "/ui/") {
        return Err("Enter the Gateway origin or its /ui/ address".into());
    }
    url.set_path("/ui/");
    Ok(url)
}

pub fn load(path: &Path) -> Result<ConnectionSettings, String> {
    match std::fs::read(path) {
        Ok(bytes) => {
            serde_json::from_slice(&bytes).map_err(|_| "Invalid Gateway connection settings".into())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            Ok(ConnectionSettings::default())
        }
        Err(error) => Err(format!("Cannot read Gateway connection settings: {error}")),
    }
}

pub fn save(path: &Path, settings: &ConnectionSettings) -> Result<(), String> {
    // Never persist an unused remote draft: it may contain credentials rejected by remote mode.
    let normalized = ConnectionSettings {
        mode: settings.mode.clone(),
        server_url: if settings.mode == ConnectionMode::Local {
            String::new()
        } else {
            console_url(&settings.server_url)?
                .origin()
                .ascii_serialization()
        },
    };
    let bytes = serde_json::to_vec_pretty(&normalized).map_err(|error| error.to_string())?;
    let pending = path.with_extension(format!("json.pending-{}", std::process::id()));
    std::fs::write(&pending, bytes)
        .and_then(|_| std::fs::rename(pending, path))
        .map_err(|error| format!("Cannot save Gateway connection settings: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalizes_server_addresses_without_accepting_secrets_or_other_schemes() {
        assert_eq!(
            console_url(" https://gateway.example:8443/ui ")
                .unwrap()
                .as_str(),
            "https://gateway.example:8443/ui/"
        );
        assert_eq!(console_url("http://127.0.0.1:4200").unwrap().path(), "/ui/");
        for value in [
            "file:///tmp/ui",
            "javascript:alert(1)",
            "https://user:secret@example.com",
            "https://example.com/?token=x",
            "https://example.com/#token",
            "https://example.com/api",
        ] {
            assert!(console_url(value).is_err(), "{value}");
        }
    }

    #[test]
    fn saved_connections_exclude_unused_drafts_and_normalize_remote_origins() {
        let dir = std::env::temp_dir().join(format!(
            "gateway-connection-save-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir(&dir).unwrap();
        let path = dir.join("connection.json");
        save(
            &path,
            &ConnectionSettings {
                mode: ConnectionMode::Local,
                server_url: "https://user:secret@example.com/?token=private".into(),
            },
        )
        .unwrap();
        assert_eq!(load(&path).unwrap().server_url, "");
        let remote = ConnectionSettings {
            mode: ConnectionMode::Existing,
            server_url: " https://example.com/ui/ ".into(),
        };
        save(&path, &remote).unwrap();
        assert_eq!(load(&path).unwrap().server_url, "https://example.com");
        assert!(save(
            &path,
            &ConnectionSettings {
                server_url: "https://user:secret@example.com".into(),
                ..remote
            }
        )
        .is_err());
        assert_eq!(load(&path).unwrap().server_url, "https://example.com");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
