//! Backward-compatible flat policy wire format; compiled plans own pre-hashed, secret-safe bindings.
use super::{policy::validate_profiles, HealthProfile, ProbePolicy};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, Default, PartialEq)]
pub struct SiteHealthConfig {
    pub policy: ProbePolicy,
    pub profiles: Vec<HealthProfile>,
}
impl std::ops::Deref for SiteHealthConfig {
    type Target = ProbePolicy;
    fn deref(&self) -> &ProbePolicy {
        &self.policy
    }
}
impl std::ops::DerefMut for SiteHealthConfig {
    fn deref_mut(&mut self) -> &mut ProbePolicy {
        &mut self.policy
    }
}
impl Serialize for SiteHealthConfig {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut value = serde_json::to_value(&self.policy).map_err(serde::ser::Error::custom)?;
        if !self.profiles.is_empty() {
            value["profiles"] =
                serde_json::to_value(&self.profiles).map_err(serde::ser::Error::custom)?;
        }
        value.serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for SiteHealthConfig {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        // Explicit split avoids serde flatten/deny_unknown_fields silently accepting misspellings.
        let mut value = serde_json::Value::deserialize(deserializer)?;
        let fields = value
            .as_object_mut()
            .ok_or_else(|| serde::de::Error::custom("Expected site health object"))?;
        let profiles = fields
            .remove("profiles")
            .map(serde_json::from_value)
            .transpose()
            .map_err(serde::de::Error::custom)?
            .unwrap_or_default();
        let policy = serde_json::from_value(value).map_err(serde::de::Error::custom)?;
        Ok(Self { policy, profiles })
    }
}
impl SiteHealthConfig {
    pub fn validate(&self, base: &str) -> anyhow::Result<()> {
        self.policy.validate(base)?;
        validate_profiles(&self.profiles, base)
    }
    pub(crate) fn plan_count(&self) -> usize {
        self.profiles.len().max(1)
    }
    pub(crate) fn compile(&self, base: &str) -> CompiledSiteHealth {
        let plans = if self.profiles.is_empty() {
            vec![CompiledPlan {
                id: None,
                account_group_ids: Vec::new(),
                policy: self.policy.clone(),
                binding: binding(self, base, None),
            }]
        } else {
            self.profiles
                .iter()
                .map(|p| CompiledPlan {
                    id: Some(p.id.clone()),
                    account_group_ids: p.account_group_ids.clone(),
                    policy: p.policy.clone(),
                    binding: binding(p, base, Some(&p.id)),
                })
                .collect()
        };
        CompiledSiteHealth {
            config: self.clone(),
            plans,
        }
    }
    pub(crate) fn redacted(&self) -> Self {
        let mut clean = self.clone();
        clean.auth.token = None;
        for profile in &mut clean.profiles {
            profile.policy.auth.token = None;
        }
        clean
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CompiledSiteHealth {
    pub config: SiteHealthConfig,
    pub plans: Vec<CompiledPlan>,
}
#[derive(Clone, Debug)]
pub(crate) struct CompiledPlan {
    /// None is the legacy whole-provider plan, a separate secret identity from explicit plans.
    pub id: Option<String>,
    pub account_group_ids: Vec<String>,
    pub policy: ProbePolicy,
    pub binding: String,
}

pub(super) fn binding(value: &impl Serialize, base: &str, id: Option<&str>) -> String {
    use sha2::{Digest, Sha256};
    use std::sync::OnceLock;
    static SALT: OnceLock<uuid::Uuid> = OnceLock::new();
    // Public snapshot bindings must not be an offline guessing oracle for monitoring passwords.
    let mut hash = Sha256::new();
    hash.update(SALT.get_or_init(uuid::Uuid::new_v4).as_bytes());
    let mut canonical = serde_json::to_value((base, id, value)).expect("validated health policy");
    // serde_json preserve_order may be unified in by another workspace crate.
    canonical.sort_all_objects();
    hash.update(canonical.to_string().as_bytes());
    hex::encode(hash.finalize())
}
