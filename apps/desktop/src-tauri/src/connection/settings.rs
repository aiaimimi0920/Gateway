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
    let bytes = serde_json::to_vec_pretty(settings).map_err(|error| error.to_string())?;
    std::fs::write(path, bytes)
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
}
