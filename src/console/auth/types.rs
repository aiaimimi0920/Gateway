//! Console authentication context, actor, wire views and stored records.

use serde::{Deserialize, Serialize};
use std::net::IpAddr;
use time::OffsetDateTime;

#[derive(Clone, Debug)]
pub struct ConsoleRequestContext {
    client_ip: IpAddr,
    origin_key: String,
    loopback: bool,
}

impl ConsoleRequestContext {
    pub fn loopback() -> Self {
        Self {
            client_ip: IpAddr::from([127, 0, 0, 1]),
            origin_key: "local".to_string(),
            loopback: true,
        }
    }

    pub fn new(client_ip: IpAddr, origin_key: String) -> Self {
        let loopback = client_ip.is_loopback();
        Self {
            client_ip,
            origin_key,
            loopback,
        }
    }

    pub fn client_ip(&self) -> IpAddr {
        self.client_ip
    }

    pub fn origin_key(&self) -> &str {
        &self.origin_key
    }

    pub fn is_loopback(&self) -> bool {
        self.loopback
    }
}

#[derive(Clone, Debug)]
pub struct AuthenticatedConsoleActor {
    pub(super) token_fingerprint: String,
    pub(super) secret_access_granted: bool,
}

impl AuthenticatedConsoleActor {
    pub fn token_fingerprint(&self) -> &str {
        &self.token_fingerprint
    }

    pub fn secret_access_granted(&self) -> bool {
        self.secret_access_granted
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleBootstrapStatus {
    pub needs_bootstrap: bool,
    pub management_configured: bool,
    pub environment_override: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConsoleSessionView {
    pub role: &'static str,
    pub capabilities: Vec<String>,
    pub active_revision: Option<String>,
    pub secret_access_granted: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretGrantView {
    pub grant: String,
    pub expires_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AdminRecord {
    pub(super) version: u32,
    pub(super) token_hash: String,
    #[serde(with = "time::serde::rfc3339")]
    pub(super) created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub(super) updated_at: OffsetDateTime,
}

#[derive(Clone, Debug)]
pub(super) struct SecretGrantRecord {
    pub(super) token_fingerprint: String,
    pub(super) origin_key: String,
    pub(super) client_ip: IpAddr,
    pub(super) expires_at: OffsetDateTime,
}
