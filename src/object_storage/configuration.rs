use std::path::PathBuf;
use std::time::Duration;

use aws_sdk_s3::config::{timeout::TimeoutConfig, Credentials, Region};
use aws_sdk_s3::Client;

use super::{GatewayObjectStorage, ObjectStorageDriver, S3_NETWORK_DEADLINE};
use crate::error::GatewayError;

impl GatewayObjectStorage {
    pub(crate) fn from_env() -> Result<Self, GatewayError> {
        let driver = std::env::var("AI_GATEWAY_OBJECT_STORAGE_DRIVER")
            .ok()
            .or_else(|| std::env::var("OBJECT_STORAGE_DRIVER").ok())
            .map(|value| value.trim().to_lowercase())
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "local".to_string());

        if driver == "s3-compatible" {
            let bucket =
                required_env(&["AI_GATEWAY_OBJECT_STORAGE_BUCKET", "OBJECT_STORAGE_BUCKET"])?;
            let endpoint = required_env(&[
                "AI_GATEWAY_OBJECT_STORAGE_ENDPOINT",
                "OBJECT_STORAGE_ENDPOINT",
            ])?;
            let access_key_id = required_env(&[
                "AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID",
                "OBJECT_STORAGE_ACCESS_KEY_ID",
            ])?;
            let secret_access_key = required_env(&[
                "AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY",
                "OBJECT_STORAGE_SECRET_ACCESS_KEY",
            ])?;
            let region =
                optional_env(&["AI_GATEWAY_OBJECT_STORAGE_REGION", "OBJECT_STORAGE_REGION"])
                    .unwrap_or_else(|| "auto".to_string());
            let force_path_style = parse_bool_env(
                optional_env(&[
                    "AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE",
                    "OBJECT_STORAGE_FORCE_PATH_STYLE",
                ])
                .as_deref(),
                false,
            );

            let credentials = Credentials::new(access_key_id, secret_access_key, None, None, "env");
            let timeout_config = TimeoutConfig::builder()
                .connect_timeout(Duration::from_secs(5))
                .read_timeout(S3_NETWORK_DEADLINE)
                .operation_attempt_timeout(S3_NETWORK_DEADLINE)
                .operation_timeout(S3_NETWORK_DEADLINE)
                .build();
            let mut builder = aws_sdk_s3::config::Builder::new()
                .region(Region::new(region))
                .credentials_provider(credentials)
                .force_path_style(force_path_style)
                .behavior_version_latest()
                .timeout_config(timeout_config);
            builder = builder.endpoint_url(endpoint);
            let client = Client::from_conf(builder.build());

            return Ok(Self {
                driver: ObjectStorageDriver::S3Compatible {
                    client,
                    bucket,
                    deadline: S3_NETWORK_DEADLINE,
                },
            });
        }

        let root = optional_env(&[
            "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR",
            "CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR",
            "OBJECT_STORAGE_LOCAL_DIR",
        ])
        .unwrap_or_else(|| ".runtime/ai-gateway-objects".to_string());

        Ok(Self {
            driver: ObjectStorageDriver::Local {
                root: PathBuf::from(root),
            },
        })
    }
}

fn optional_env(names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| std::env::var(name).ok())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn required_env(names: &[&str]) -> Result<String, GatewayError> {
    optional_env(names).ok_or_else(|| {
        GatewayError::service_unavailable(format!("缺少对象存储配置: {}", names.join(" / ")))
    })
}

fn parse_bool_env(value: Option<&str>, fallback: bool) -> bool {
    let Some(value) = value else {
        return fallback;
    };
    match value.trim().to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => true,
        "0" | "false" | "no" | "off" => false,
        _ => fallback,
    }
}
