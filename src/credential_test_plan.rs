//! Named plans own their matrix independently; legacy inherited policies remain separate.
use crate::{
    credential_test_policy::{exact_model, CredentialTestPolicy},
    routing::config::ProviderConfigYaml,
};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(tag = "kind", rename_all = "lowercase", deny_unknown_fields)]
pub enum TestScope {
    Pool,
    Subpool { id: String },
    Account { id: String },
}

impl TestScope {
    pub fn valid(&self) -> bool {
        match self {
            Self::Pool => true,
            Self::Subpool { id } | Self::Account { id } => exact_model(id),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialTestPlan {
    pub id: String,
    pub name: String,
    pub scopes: Vec<TestScope>,
    pub policy: CredentialTestPolicy,
}

impl CredentialTestPlan {
    pub fn validate(&self, provider: &ProviderConfigYaml) -> Result<(), &'static str> {
        if !exact_model(&self.id)
            || self.name.trim().is_empty()
            || self.name.len() > 256
            || self.name.chars().any(char::is_control)
        {
            return Err("A test plan requires a bounded stable ID and name.");
        }
        if self.scopes.is_empty()
            || self.scopes.len() > 128
            || self.scopes.iter().collect::<HashSet<_>>().len() != self.scopes.len()
            || self.scopes.iter().any(|scope| {
                !scope.valid()
                    || std::mem::discriminant(scope) != std::mem::discriminant(&self.scopes[0])
            })
            || (self.scopes.len() > 1 && matches!(self.scopes[0], TestScope::Pool))
        {
            return Err("Select one pool or 1..128 distinct subpools or accounts for a test plan.");
        }
        for scope in &self.scopes {
            let exists = match scope {
                TestScope::Pool => true,
                TestScope::Account { id } => {
                    if provider.credentials.is_empty() {
                        *id == format!("{}::default", provider.id)
                    } else {
                        provider
                            .credentials
                            .iter()
                            .enumerate()
                            .any(|(index, credential)| {
                                credential
                                    .id
                                    .as_deref()
                                    .map(str::to_string)
                                    .unwrap_or_else(|| format!("{}-cred-{index}", provider.id))
                                    == *id
                            })
                    }
                }
                TestScope::Subpool { id } => provider.credentials.iter().any(|credential| {
                    credential.credential_identity_category_id.as_deref() == Some(id)
                }),
            };
            if !exists {
                return Err("Test plan scope is not present in this provider pool.");
            }
        }
        let accounts = if provider.credentials.is_empty() {
            1
        } else {
            provider
                .credentials
                .iter()
                .enumerate()
                .filter(|(index, credential)| {
                    self.scopes.iter().any(|scope| match scope {
                        TestScope::Pool => true,
                        TestScope::Subpool { id } => {
                            credential.credential_identity_category_id.as_deref() == Some(id)
                        }
                        TestScope::Account { id } => {
                            credential
                                .id
                                .clone()
                                .unwrap_or_else(|| format!("{}-cred-{index}", provider.id))
                                == *id
                        }
                    })
                })
                .count()
        };
        if accounts > 128 {
            return Err("Select a test plan scope containing at most 128 accounts.");
        }
        self.policy.validate()
    }
}
