//! Versioned payload and plaintext validation. Only ciphertext may leave the backend.
use base64::{engine::general_purpose::STANDARD, Engine};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Archive {
    pub version: u32,
    pub created_at: String,
    pub files: BTreeMap<String, String>,
}

impl Archive {
    pub fn encode(files: BTreeMap<String, Vec<u8>>) -> Result<Vec<u8>, String> {
        gateway_local_data::restore::validate_files(&files).map_err(|e| e.to_string())?;
        serde_json::to_vec(&Self {
            version: 1,
            created_at: time::OffsetDateTime::now_utc()
                .format(&time::format_description::well_known::Rfc3339)
                .map_err(|_| "Cannot format backup date")?,
            files: files
                .into_iter()
                .map(|(name, bytes)| (name, STANDARD.encode(bytes)))
                .collect(),
        })
        .map_err(|_| "Cannot encode backup".into())
    }
    pub fn decode(bytes: &[u8]) -> Result<(String, BTreeMap<String, Vec<u8>>), String> {
        let archive: Self = serde_json::from_slice(bytes).map_err(|_| "Invalid backup payload")?;
        if archive.version != 1 || archive.files.len() > 4096 {
            return Err("Unsupported backup version or file count".into());
        }
        let files = archive
            .files
            .into_iter()
            .map(|(name, bytes)| {
                STANDARD
                    .decode(bytes)
                    .map(|value| (name, value))
                    .map_err(|_| "Invalid backup file encoding".to_owned())
            })
            .collect::<Result<BTreeMap<_, _>, _>>()?;
        gateway_local_data::restore::validate_files(&files).map_err(|e| e.to_string())?;
        let document: crate::routing::config::RouteConfigYaml =
            serde_yaml::from_slice(&files["local/routes.yaml"])
                .map_err(|_| "Invalid backup route configuration")?;
        crate::console::document::validate_route_document(document)
            .map_err(|_| "Backup route validation failed")?;
        let seal: String = serde_json::from_slice(&files["local/state/access-key-seal.json"])
            .map_err(|_| "Invalid backup credential seal")?;
        if STANDARD
            .decode(seal)
            .map_err(|_| "Invalid backup credential seal")?
            .len()
            != 32
        {
            return Err("Invalid backup credential seal".into());
        }
        let admin = files
            .get("local/state/console/admin.json")
            .ok_or("Backup is missing administrator keys")?;
        let admin: serde_json::Value =
            serde_json::from_slice(admin).map_err(|_| "Invalid backup administrator keys")?;
        if admin["version"] != 2 || !admin["keys"].is_array() {
            return Err("Invalid backup administrator keys".into());
        }
        Ok((archive.created_at, files))
    }
}
