//! Persisted default egress policy. Connection pools adopt it only at process startup.
use serde::{Deserialize, Serialize};
use std::{
    io::{Read, Write},
    path::Path,
    sync::{Mutex, OnceLock},
};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    #[default]
    Auto,
    Off,
    Custom,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default, deny_unknown_fields)]
pub struct Policy {
    pub mode: Mode,
    pub url: String,
}

static ACTIVE: OnceLock<Policy> = OnceLock::new();
static WRITE_LOCK: Mutex<()> = Mutex::new(());
pub const LOCAL_BYPASS: &str = "localhost,127.0.0.1,::1";

impl Policy {
    pub fn normalized(mut self) -> Result<Self, String> {
        if self.mode != Mode::Custom {
            self.url.clear();
            return Ok(self);
        }
        if self.url.len() > 2048 {
            return Err("Proxy address is too long".into());
        }
        let url = url::Url::parse(self.url.trim()).map_err(|_| "Enter an HTTP(S) proxy URL")?;
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/"
            || url.port() == Some(0)
        {
            return Err(
                "Proxy requires an HTTP(S) origin without credentials, path, query or fragment"
                    .into(),
            );
        }
        self.url = url.to_string();
        Ok(self)
    }
}

pub fn active() -> &'static Policy {
    ACTIVE.get_or_init(Policy::default)
}

pub fn initialize(directory: &Path) -> Result<(), String> {
    ACTIVE
        .set(load(directory)?)
        .map_err(|_| "Proxy policy initialized after HTTP clients".into())
}

pub fn load(directory: &Path) -> Result<Policy, String> {
    let file = match std::fs::File::open(directory.join("outbound-proxy.json")) {
        Ok(file) => file,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Policy::default()),
        Err(_) => return Err("Cannot read outbound proxy settings".into()),
    };
    let mut bytes = Vec::new();
    file.take(16 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "Cannot read proxy settings")?;
    if bytes.len() > 16 * 1024 {
        return Err("Proxy settings are too large".into());
    }
    serde_json::from_slice::<Policy>(&bytes)
        .map_err(|_| "Invalid proxy settings")?
        .normalized()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Snapshot {
    #[serde(flatten)]
    pub policy: Policy,
    pub effective_mode: Mode,
    pub restart_required: bool,
}

pub fn snapshot(directory: &Path) -> Result<Snapshot, String> {
    let policy = load(directory)?;
    Ok(Snapshot {
        restart_required: &policy != active(),
        effective_mode: active().mode.clone(),
        policy,
    })
}

pub fn save(directory: &Path, policy: Policy) -> Result<Snapshot, String> {
    let policy = policy.normalized()?;
    let _guard = WRITE_LOCK
        .lock()
        .map_err(|_| "Proxy settings lock poisoned")?;
    std::fs::create_dir_all(directory).map_err(|_| "Cannot create proxy settings directory")?;
    let pending = directory.join(format!(".outbound-proxy-{}.tmp", uuid::Uuid::new_v4()));
    let result = (|| {
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&pending)
            .map_err(|_| "Cannot stage proxy settings")?;
        file.write_all(
            &serde_json::to_vec_pretty(&policy).map_err(|_| "Cannot encode proxy settings")?,
        )
        .and_then(|_| file.sync_all())
        .map_err(|_| "Cannot write proxy settings")?;
        std::fs::rename(&pending, directory.join("outbound-proxy.json"))
            .map_err(|_| "Cannot publish proxy settings")?;
        snapshot(directory)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(pending);
    }
    result
}

pub(crate) fn rquest(builder: rquest::ClientBuilder, policy: &Policy) -> rquest::ClientBuilder {
    match policy.mode {
        Mode::Auto => super::proxy_policy::configure_auto(builder),
        Mode::Off => builder.no_proxy(),
        Mode::Custom => builder.no_proxy().proxy(
            rquest::Proxy::all(&policy.url)
                .expect("validated proxy URL")
                .no_proxy(rquest::NoProxy::from_string(LOCAL_BYPASS)),
        ),
    }
}

pub(crate) fn reqwest(builder: reqwest::ClientBuilder, policy: &Policy) -> reqwest::ClientBuilder {
    match policy.mode {
        Mode::Auto => builder,
        Mode::Off => builder.no_proxy(),
        Mode::Custom => builder.no_proxy().proxy(
            reqwest::Proxy::all(&policy.url)
                .expect("validated proxy URL")
                .no_proxy(reqwest::NoProxy::from_string(LOCAL_BYPASS)),
        ),
    }
}

#[cfg(test)]
#[path = "proxy_settings_tests.rs"]
mod tests;
