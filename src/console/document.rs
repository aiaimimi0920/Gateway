//! Canonical route documents and stable validation entry points.

mod validation;
use validation::collect_document_diagnostics;

use std::error::Error;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::routing::config::{compile_route_document, RouteConfigInner, RouteConfigYaml};

pub(crate) const MAX_CANONICAL_DOCUMENT_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Error,
    Warning,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RouteConfigDiagnostic {
    pub code: String,
    pub severity: DiagnosticSeverity,
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct RouteConfigDiagnostics {
    pub diagnostics: Vec<RouteConfigDiagnostic>,
}

impl RouteConfigDiagnostics {
    pub fn requires_repair(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
    }

    fn push_error(
        &mut self,
        code: impl Into<String>,
        path: impl Into<String>,
        message: impl Into<String>,
    ) {
        self.diagnostics.push(RouteConfigDiagnostic {
            code: code.into(),
            severity: DiagnosticSeverity::Error,
            path: path.into(),
            message: message.into(),
        });
    }

    fn sort(&mut self) {
        self.diagnostics.sort_by(|left, right| {
            left.path
                .cmp(&right.path)
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
        });
    }
}

impl fmt::Display for RouteConfigDiagnostics {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.diagnostics.is_empty() {
            return formatter.write_str("route configuration is valid");
        }
        for (index, diagnostic) in self.diagnostics.iter().enumerate() {
            if index > 0 {
                formatter.write_str("; ")?;
            }
            write!(
                formatter,
                "{} at {}: {}",
                diagnostic.code, diagnostic.path, diagnostic.message
            )?;
        }
        Ok(())
    }
}

impl Error for RouteConfigDiagnostics {}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RouteConfigInspection {
    pub diagnostics: RouteConfigDiagnostics,
    pub requires_repair: bool,
}

#[derive(Clone, Eq, PartialEq)]
pub struct CanonicalRouteDocument {
    canonical_json: Vec<u8>,
    canonical_yaml: Vec<u8>,
    document_digest: String,
    yaml_digest: String,
}

impl fmt::Debug for CanonicalRouteDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CanonicalRouteDocument")
            .field("canonical_json_len", &self.canonical_json.len())
            .field("canonical_yaml_len", &self.canonical_yaml.len())
            .field("document_digest", &self.document_digest)
            .field("yaml_digest", &self.yaml_digest)
            .finish()
    }
}

impl CanonicalRouteDocument {
    pub fn canonical_json(&self) -> &[u8] {
        &self.canonical_json
    }

    pub fn canonical_yaml(&self) -> &[u8] {
        &self.canonical_yaml
    }

    pub fn document_digest(&self) -> &str {
        &self.document_digest
    }

    pub fn yaml_digest(&self) -> &str {
        &self.yaml_digest
    }
}

#[derive(Debug, thiserror::Error)]
pub enum CanonicalDocumentError {
    #[error("failed to serialize route configuration as JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("failed to serialize route configuration as YAML: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error(
        "canonical document size limit exceeded for {format}: {actual_bytes} bytes exceeds {max_bytes} bytes"
    )]
    TooLarge {
        format: &'static str,
        actual_bytes: usize,
        max_bytes: usize,
    },
}

#[derive(Clone)]
pub struct ValidatedRouteDocument {
    document: RouteConfigYaml,
    canonical_json: Vec<u8>,
    canonical_yaml: Vec<u8>,
    document_digest: String,
    yaml_digest: String,
    diagnostics: RouteConfigDiagnostics,
    compiled: Arc<RouteConfigInner>,
}

impl fmt::Debug for ValidatedRouteDocument {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidatedRouteDocument")
            .field("provider_count", &self.document.providers.len())
            .field("route_count", &self.document.model_routes.len())
            .field("document_digest", &self.document_digest)
            .field("yaml_digest", &self.yaml_digest)
            .field("diagnostics", &self.diagnostics)
            .finish()
    }
}

impl ValidatedRouteDocument {
    pub fn document(&self) -> &RouteConfigYaml {
        &self.document
    }

    pub fn canonical_json(&self) -> &[u8] {
        &self.canonical_json
    }

    pub fn canonical_yaml(&self) -> &[u8] {
        &self.canonical_yaml
    }

    pub fn document_digest(&self) -> &str {
        &self.document_digest
    }

    pub fn yaml_digest(&self) -> &str {
        &self.yaml_digest
    }

    pub fn diagnostics(&self) -> &RouteConfigDiagnostics {
        &self.diagnostics
    }

    #[allow(dead_code)]
    pub(crate) fn compiled(&self) -> Arc<RouteConfigInner> {
        self.compiled.clone()
    }
}

pub fn inspect_route_document(document: &RouteConfigYaml) -> RouteConfigInspection {
    let mut diagnostics = collect_document_diagnostics(document);
    if let Err(error) = compile_route_document(document.clone()) {
        diagnostics.push_error(
            "route_config_compile_failed",
            "/providers",
            error.to_string(),
        );
    }
    diagnostics.sort();
    RouteConfigInspection {
        requires_repair: diagnostics.requires_repair(),
        diagnostics,
    }
}

pub fn validate_route_document(
    document: RouteConfigYaml,
) -> Result<ValidatedRouteDocument, RouteConfigDiagnostics> {
    let inspection = inspect_route_document(&document);
    if inspection.requires_repair {
        return Err(inspection.diagnostics);
    }

    let normalized = materialize_credential_ids(document);
    let compiled = compile_route_document(normalized.clone()).map_err(|error| {
        let mut diagnostics = RouteConfigDiagnostics::default();
        diagnostics.push_error(
            "route_config_compile_failed",
            "/providers",
            error.to_string(),
        );
        diagnostics
    })?;
    let canonical = canonicalize_route_document(&normalized).map_err(|error| {
        let mut diagnostics = RouteConfigDiagnostics::default();
        let code = if matches!(&error, CanonicalDocumentError::TooLarge { .. }) {
            "route_config_document_too_large"
        } else {
            "route_config_canonicalization_failed"
        };
        diagnostics.push_error(code, "", error.to_string());
        diagnostics
    })?;

    Ok(ValidatedRouteDocument {
        document: normalized,
        canonical_json: canonical.canonical_json,
        canonical_yaml: canonical.canonical_yaml,
        document_digest: canonical.document_digest,
        yaml_digest: canonical.yaml_digest,
        diagnostics: inspection.diagnostics,
        compiled: Arc::new(compiled),
    })
}

#[cfg(test)]
#[path = "document/account_groups_tests.rs"]
mod tests;

pub fn canonicalize_route_document(
    document: &RouteConfigYaml,
) -> Result<CanonicalRouteDocument, CanonicalDocumentError> {
    let value = serde_json::to_value(document)?;
    let sorted = sort_json_value(value);
    let canonical_json = serde_json::to_vec(&sorted)?;
    ensure_canonical_size("JSON", canonical_json.len())?;
    let mut yaml = serde_yaml::to_string(&sorted)?.replace("\r\n", "\n");
    if !yaml.ends_with('\n') {
        yaml.push('\n');
    }
    let canonical_yaml = yaml.into_bytes();
    ensure_canonical_size("YAML", canonical_yaml.len())?;

    Ok(CanonicalRouteDocument {
        document_digest: sha256_hex(&canonical_json),
        yaml_digest: sha256_hex(&canonical_yaml),
        canonical_json,
        canonical_yaml,
    })
}

fn ensure_canonical_size(
    format: &'static str,
    actual_bytes: usize,
) -> Result<(), CanonicalDocumentError> {
    if actual_bytes > MAX_CANONICAL_DOCUMENT_BYTES {
        return Err(CanonicalDocumentError::TooLarge {
            format,
            actual_bytes,
            max_bytes: MAX_CANONICAL_DOCUMENT_BYTES,
        });
    }
    Ok(())
}

pub(crate) fn materialize_credential_ids(mut document: RouteConfigYaml) -> RouteConfigYaml {
    for provider in &mut document.providers {
        let provider_id = provider.id.clone();
        crate::credential_pool_automation::capacity::normalize_refill_state(provider);
        for (index, credential) in provider.credentials.iter_mut().enumerate() {
            if credential.id.is_none() {
                credential.id = Some(format!("{provider_id}-cred-{index}"));
            }
        }
    }
    document
}

fn sort_json_value(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|left, right| left.0.cmp(&right.0));
            let mut sorted = Map::new();
            for (key, value) in entries {
                sorted.insert(key, sort_json_value(value));
            }
            Value::Object(sorted)
        }
        Value::Array(values) => Value::Array(values.into_iter().map(sort_json_value).collect()),
        other => other,
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

pub(crate) fn encode_pointer_segment(segment: &str) -> String {
    segment.replace('~', "~0").replace('/', "~1")
}
