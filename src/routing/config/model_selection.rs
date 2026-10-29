//! Explicit editable model ownership; directory/probe evidence stays independent.
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ModelSelection {
    pub fetched: Vec<String>,
    pub manual: Vec<String>,
}

impl ModelSelection {
    pub fn models(&self) -> anyhow::Result<Vec<String>> {
        let mut seen = HashSet::new();
        let mut models = Vec::new();
        for list in [&self.manual, &self.fetched] {
            anyhow::ensure!(
                list.len() <= 2048,
                "Model selection exceeds 2048 IDs per source."
            );
            for id in list {
                anyhow::ensure!(
                    !id.is_empty()
                        && id.trim() == id
                        && id.len() <= 256
                        && !id.chars().any(|c| c.is_control() || matches!(c, '*' | '?')),
                    "Invalid model selection ID."
                );
                if seen.insert(id.as_str()) {
                    models.push(id.clone());
                }
            }
        }
        Ok(models)
    }
}
