//! Shared bounded test-plan contract. Connectivity and answer quality remain independent.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
#[cfg(test)]
mod tests;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialTestPolicy {
    #[serde(default)]
    pub automatic_enabled: bool,
    #[serde(default = "default_interval")]
    pub interval_minutes: u64,
    pub model_selection: ModelSelection,
    #[serde(default)]
    pub models: Vec<String>,
    pub cases: Vec<CredentialTestCase>,
}

fn default_interval() -> u64 {
    60
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum ModelSelection {
    All,
    Selected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CredentialTestCase {
    pub id: String,
    pub name: String,
    pub prompt: String,
    #[serde(default)]
    pub expected_answer: Option<String>,
    pub difficulty: u8,
    pub enabled: bool,
}

pub fn exact_model(model: &str) -> bool {
    !model.is_empty()
        && model.len() <= 256
        && model.trim() == model
        && !model.contains(['*', '?'])
        && !model.chars().any(char::is_control)
}

impl CredentialTestPolicy {
    pub fn validate(&self) -> Result<(), &'static str> {
        if !(1..=10_080).contains(&self.interval_minutes) {
            return Err("Test interval must be 1..10080 minutes.");
        }
        if self.models.len() > 128
            || self.models.iter().any(|model| !exact_model(model))
            || self.models.iter().collect::<HashSet<_>>().len() != self.models.len()
            || (self.model_selection == ModelSelection::Selected && self.models.is_empty())
        {
            return Err("Choose all configured models or 1..128 unique exact model names.");
        }
        if self.cases.is_empty()
            || self.cases.len() > 16
            || !self.cases.iter().any(|case| case.enabled)
            || self
                .cases
                .iter()
                .map(|case| &case.id)
                .collect::<HashSet<_>>()
                .len()
                != self.cases.len()
        {
            return Err("A test set must contain 1..16 unique cases with at least one selected.");
        }
        for case in &self.cases {
            if !exact_model(&case.id)
                || case.name.trim().is_empty()
                || case.name.len() > 256
                || case.prompt.trim().is_empty()
                || case.prompt.len() > 8192
                || !(1..=3).contains(&case.difficulty)
                || case
                    .expected_answer
                    .as_ref()
                    .is_some_and(|answer| answer.trim().is_empty() || answer.len() > 4096)
            {
                return Err("Invalid test case: bounded ID/name/prompt/answer and difficulty 1..3 required.");
            }
        }
        if self.model_selection == ModelSelection::Selected
            && self.models.len() * self.cases.iter().filter(|case| case.enabled).count() > 128
        {
            return Err("At most 128 model/case combinations per account are allowed.");
        }
        Ok(())
    }
}
