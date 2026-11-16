//! Final payload gates cover startup and Console commits after discovery/flow compilation.
use super::{CompiledCredential, ProviderAccountPayload, ProviderConfigYaml};
use crate::{
    credential_test_plan::TestScope, credential_test_policy::CredentialTestPolicy,
    provider_model_id::valid_for_payload,
};

fn policy(
    policy: Option<&CredentialTestPolicy>,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    if let Some(policy) = policy {
        policy
            .validate_for_payload(payload)
            .map_err(anyhow::Error::msg)?;
    }
    Ok(())
}

// Legacy short declarations keep their old grammar. Extended identities are always checked.
fn extended<'a>(
    models: impl IntoIterator<Item = &'a String>,
    payload: &ProviderAccountPayload,
) -> anyhow::Result<()> {
    for model in models {
        anyhow::ensure!(
            model.len() <= 256 || valid_for_payload(model, payload),
            "Extended model identity is invalid for the effective provider product."
        );
    }
    Ok(())
}

pub(super) fn validate(
    cfg: &ProviderConfigYaml,
    base: &ProviderAccountPayload,
    credentials: &[CompiledCredential],
) -> anyhow::Result<()> {
    // Even inactive owners remain validated; turning off a schedule must not grant new syntax.
    policy(cfg.default_account_test_policy.as_ref(), base)?;
    for plan in &cfg.test_plans {
        plan.validate(cfg).map_err(anyhow::Error::msg)?;
    }
    let payloads: Vec<_> = if credentials.is_empty() {
        vec![base]
    } else {
        credentials
            .iter()
            .map(|credential| &credential.payload)
            .collect()
    };
    for payload in &payloads {
        policy(cfg.test_policy.as_ref(), payload)?;
        extended(
            cfg.model_map
                .keys()
                .chain(cfg.model_map.values())
                .chain(cfg.model_map_targets.keys())
                .chain(cfg.model_map_targets.values().flatten()),
            payload,
        )?;
    }
    if credentials.is_empty() {
        extended(&cfg.supported_models, base)?;
        extended(base.default_model.iter(), base)?;
        for policy_value in cfg.subpool_test_policies.values() {
            policy(Some(policy_value), base)?;
        }
        for plan in &cfg.test_plans {
            policy(Some(&plan.policy), base)?;
        }
        return Ok(());
    }
    for (index, (raw, credential)) in cfg.credentials.iter().zip(credentials).enumerate() {
        let payload = &credential.payload;
        policy(raw.test_policy.as_ref(), payload)?;
        if let Some(category) = raw.credential_identity_category_id.as_ref() {
            policy(cfg.subpool_test_policies.get(category), payload)?;
        }
        let models =
            if credential.supported_models.is_empty() && !credential.explicit_model_selection {
                &cfg.supported_models
            } else {
                &credential.supported_models
            };
        extended(models, payload)?;
        extended(payload.default_model.iter(), payload)?;
        let id = super::effective_credential_id(&cfg.id, index, raw);
        for plan in &cfg.test_plans {
            if plan.scopes.iter().any(|scope| match scope {
                TestScope::Pool => true,
                TestScope::Account { id: selected } => selected == id.as_ref(),
                TestScope::Subpool { id: selected } => {
                    raw.credential_identity_category_id.as_ref() == Some(selected)
                }
            }) {
                policy(Some(&plan.policy), payload)?;
            }
        }
    }
    // An empty subpool may acquire accounts later; validate it against its current product.
    for (category, policy_value) in &cfg.subpool_test_policies {
        if !cfg
            .credentials
            .iter()
            .any(|credential| credential.credential_identity_category_id.as_ref() == Some(category))
        {
            policy(Some(policy_value), base)?;
        }
    }
    Ok(())
}
