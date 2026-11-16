//! Live inbound device policy. Sharing never bypasses model or administrator authentication.
mod local_addresses;
mod persistence;

use crate::{console::ConsoleConfig, error::GatewayError};
use axum::{
    extract::{ConnectInfo, Request, State},
    middleware::Next,
    response::{IntoResponse, Response},
};
use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use std::{
    net::{IpAddr, SocketAddr},
    sync::Arc,
};

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Settings {
    pub enabled: bool,
}

pub(crate) struct Runtime {
    config: ConsoleConfig,
    settings: RwLock<Result<Settings, String>>,
    write_lock: Mutex<()>,
}

impl Runtime {
    pub fn new(config: &ConsoleConfig) -> Self {
        Self {
            config: config.clone(),
            settings: RwLock::new(persistence::load(config)),
            write_lock: Mutex::new(()),
        }
    }

    pub fn settings(&self) -> Result<Settings, GatewayError> {
        self.settings
            .read()
            .clone()
            .map_err(|_| GatewayError::service_unavailable("Cannot read LAN sharing settings"))
    }

    pub fn save(&self, settings: Settings) -> Result<Settings, GatewayError> {
        // Serialize disk publication and the in-memory switch so concurrent saves cannot diverge.
        let _guard = self.write_lock.lock();
        persistence::save(&self.config, settings)
            .map_err(|_| GatewayError::service_unavailable("Cannot save LAN sharing settings"))?;
        *self.settings.write() = Ok(settings);
        Ok(settings)
    }
}

pub(crate) fn is_local_device(mut peer: SocketAddr) -> bool {
    if let IpAddr::V6(ip) = peer.ip() {
        if let Some(ip) = ip.to_ipv4_mapped() {
            peer = SocketAddr::new(IpAddr::V4(ip), 0);
        }
    }
    let ip = peer.ip();
    if ip.is_loopback() {
        return true;
    }
    if ip.is_unspecified() || ip.is_multicast() || matches!(ip, IpAddr::V4(ip) if ip.is_broadcast())
    {
        return false;
    }
    local_addresses::contains(ip)
}

pub(crate) async fn enforce(
    State(runtime): State<Arc<Runtime>>,
    request: Request,
    next: Next,
) -> Response {
    match runtime.settings() {
        Ok(settings) if settings.enabled => return next.run(request).await,
        policy => {
            if request
                .extensions()
                .get::<ConnectInfo<SocketAddr>>()
                .is_some_and(|peer| is_local_device(peer.0))
            {
                return next.run(request).await;
            }
            if let Err(error) = policy {
                return error.into_response();
            }
        }
    }
    let mut error = GatewayError::unauthorized(
        "LAN sharing is disabled; access is limited to the Gateway host",
    )
    .with_code("gateway_lan_sharing_disabled");
    error.http_status = Some(403);
    error.into_response()
}
