use super::{
    CredentialAutomationDriver, CredentialAutomationDriverTransport,
    CredentialPoolAutomationConfig, DriverRegistry, DriverRegistryDocument,
};
use crate::routing::config::ProviderConfigYaml;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use url::{Host, Url};
impl DriverRegistry {
    pub(super) fn load(config: &CredentialPoolAutomationConfig) -> anyhow::Result<Self> {
        let Some(path) = config.driver_config_path.as_deref() else {
            return Ok(Self::default());
        };
        let bytes = std::fs::read(path).map_err(|error| {
            anyhow::anyhow!(
                "failed to read credential automation driver registry '{}': {error}",
                path.display()
            )
        })?;
        let document: DriverRegistryDocument = serde_json::from_slice(&bytes).map_err(|error| {
            anyhow::anyhow!(
                "failed to parse credential automation driver registry '{}': {error}",
                path.display()
            )
        })?;
        let mut drivers = BTreeMap::new();
        for mut driver in document.drivers {
            driver.id = normalize_required_id(&driver.id, "driver id")?;
            driver.provider_ids = driver
                .provider_ids
                .into_iter()
                .map(|provider_id| normalize_required_id(&provider_id, "provider id"))
                .collect::<anyhow::Result<Vec<_>>>()?;
            if driver.provider_ids.is_empty() {
                anyhow::bail!(
                    "credential automation driver '{}' has no provider_ids",
                    driver.id
                );
            }
            validate_driver(config, &driver)?;
            if drivers.insert(driver.id.clone(), driver).is_some() {
                anyhow::bail!("duplicate credential automation driver id");
            }
        }
        Ok(Self { drivers })
    }

    pub(super) fn resolve<'a>(
        &'a self,
        provider: &ProviderConfigYaml,
    ) -> Result<Option<&'a CredentialAutomationDriver>, String> {
        if let Some(explicit_id) = provider
            .credential_automation_driver_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            let Some(driver) = self.drivers.get(explicit_id) else {
                return Err(format!(
                    "configured driver '{explicit_id}' is not registered"
                ));
            };
            if !driver.supports_provider(&provider.id) {
                return Err(format!(
                    "driver '{}' is not allowlisted for provider '{}'",
                    driver.id, provider.id
                ));
            }
            return Ok(Some(driver));
        }

        let mut matches = self
            .drivers
            .values()
            .filter(|driver| driver.supports_provider(&provider.id));
        let first = matches.next();
        if matches.next().is_some() {
            return Err(format!(
                "provider '{}' matches multiple drivers; set credential_automation_driver_id",
                provider.id
            ));
        }
        Ok(first)
    }
}

pub(super) fn normalize_required_id(value: &str, label: &str) -> anyhow::Result<String> {
    let normalized = value.trim();
    if normalized.is_empty()
        || !normalized
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        anyhow::bail!("{label} must contain only ASCII letters, digits, '-' or '_'");
    }
    Ok(normalized.to_string())
}

pub(super) fn validate_driver(
    config: &CredentialPoolAutomationConfig,
    driver: &CredentialAutomationDriver,
) -> anyhow::Result<()> {
    match &driver.transport {
        CredentialAutomationDriverTransport::Script { script } => {
            resolve_allowlisted_script_path(config, script)?;
        }
        CredentialAutomationDriverTransport::Http {
            endpoint,
            secret_env,
        } => {
            let url = Url::parse(endpoint).map_err(|error| {
                anyhow::anyhow!("driver '{}' endpoint is invalid: {error}", driver.id)
            })?;
            let is_loopback_http = url.scheme() == "http"
                && match url.host() {
                    Some(Host::Ipv4(address)) => address.is_loopback(),
                    Some(Host::Ipv6(address)) => address.is_loopback(),
                    Some(Host::Domain(_)) | None => false,
                };
            if url.scheme() != "https" && !is_loopback_http {
                anyhow::bail!(
                    "driver '{}' endpoint must use HTTPS (HTTP is allowed only for loopback)",
                    driver.id
                );
            }
            if let Some(secret_env) = secret_env {
                normalize_required_id(secret_env, "secret_env")?;
            }
        }
    }
    Ok(())
}

pub(super) fn resolve_allowlisted_script_path(
    config: &CredentialPoolAutomationConfig,
    script: &str,
) -> anyhow::Result<PathBuf> {
    let root = config.script_root.as_deref().ok_or_else(|| {
        anyhow::anyhow!("script driver requires GATEWAY_CREDENTIAL_POOL_AUTOMATION_SCRIPT_ROOT")
    })?;
    let relative = Path::new(script);
    if relative.is_absolute()
        || relative
            .components()
            .any(|part| matches!(part, std::path::Component::ParentDir))
    {
        anyhow::bail!("script driver path must be a relative path without '..'");
    }
    let canonical_root = root.canonicalize().map_err(|error| {
        anyhow::anyhow!(
            "failed to canonicalize script root '{}': {error}",
            root.display()
        )
    })?;
    let canonical_script = canonical_root
        .join(relative)
        .canonicalize()
        .map_err(|error| {
            anyhow::anyhow!(
                "failed to canonicalize automation script '{}': {error}",
                relative.display()
            )
        })?;
    if !canonical_script.starts_with(&canonical_root) || !canonical_script.is_file() {
        anyhow::bail!("automation script is outside the allowlisted root or is not a file");
    }
    Ok(canonical_script)
}
