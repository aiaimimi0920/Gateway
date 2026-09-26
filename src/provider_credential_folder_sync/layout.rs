//! Interpret folder material descriptors and construct existing export layouts.
//! Filesystem keys use the shared containment owner's lossless validation.
use super::canonicalization::{
    canonicalize_credential_material_kind, canonicalize_folder_service_provider_slug,
    canonicalize_folder_surface_slug, sanitize_file_component,
};
use super::classification::{
    derive_provider_family_slug, derive_provider_surface_slug, derive_service_provider_slug,
};
use super::payload_metadata::derive_credential_material_kind;
use crate::db;
use crate::error::GatewayError;
use serde_json::Value;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct FolderImportPathDescriptor {
    pub(super) service_provider_slug: Option<String>,
    pub(super) provider_surface_slug: String,
    pub(super) credential_material_kind: Option<String>,
}

pub(super) fn resolve_import_path_descriptor(
    relative: &str,
    raw_payload: &Value,
) -> Option<FolderImportPathDescriptor> {
    let path_descriptor = describe_folder_import_path(relative)?;
    let raw_map = raw_payload.as_object()?;

    let service_provider_slug = raw_map
        .get("serviceProviderKey")
        .or_else(|| raw_map.get("service_provider_key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(canonicalize_folder_service_provider_slug)
        .or(path_descriptor.service_provider_slug);

    let provider_surface_slug = raw_map
        .get("providerSurfaceKey")
        .or_else(|| raw_map.get("provider_surface_key"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(canonicalize_folder_surface_slug)
        .unwrap_or(path_descriptor.provider_surface_slug);

    let credential_material_kind = raw_map
        .get("credentialMaterialKind")
        .or_else(|| raw_map.get("credential_material_kind"))
        .or_else(|| raw_map.get("materialKind"))
        .or_else(|| raw_map.get("material_kind"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(canonicalize_credential_material_kind)
        .or(path_descriptor.credential_material_kind);

    Some(FolderImportPathDescriptor {
        service_provider_slug,
        provider_surface_slug,
        credential_material_kind,
    })
}

pub(super) fn describe_folder_import_path(relative: &str) -> Option<FolderImportPathDescriptor> {
    let normalized = normalize_source_path_key(relative)?;
    let segments = normalized.split('/').collect::<Vec<_>>();
    if segments.len() < 2 {
        return None;
    }
    if segments.len() >= 4 {
        return Some(FolderImportPathDescriptor {
            service_provider_slug: Some(canonicalize_folder_service_provider_slug(segments[0])),
            provider_surface_slug: canonicalize_folder_surface_slug(segments[1]),
            credential_material_kind: Some(canonicalize_credential_material_kind(segments[2])),
        });
    }
    if segments.len() == 3 {
        return Some(FolderImportPathDescriptor {
            service_provider_slug: Some(canonicalize_folder_service_provider_slug(segments[0])),
            provider_surface_slug: canonicalize_folder_surface_slug(segments[1]),
            credential_material_kind: None,
        });
    }
    Some(FolderImportPathDescriptor {
        service_provider_slug: None,
        provider_surface_slug: canonicalize_folder_surface_slug(segments[0]),
        credential_material_kind: None,
    })
}

pub(super) fn default_folder_sync_relative_path(
    provider_account: &db::GatewayProviderAccountView,
    credential: &db::GatewayProviderCredentialView,
) -> String {
    if provider_account
        .service_provider_key
        .eq_ignore_ascii_case("azure_openai_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("anthropic_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("aws_bedrock_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("cohere_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("qwen_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("gemini_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("chataibot_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("suno_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("udio_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("lumalabs_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("xai_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("perplexity_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("freebuff_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("xfyun_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("producer_platform")
        || provider_account
            .service_provider_key
            .eq_ignore_ascii_case("kiro_platform")
    {
        return format!(
            "{}/{}/{}/{}.json",
            derive_service_provider_slug(provider_account),
            derive_provider_surface_slug(provider_account),
            material_kind_folder_slug(derive_credential_material_kind(
                provider_account,
                Some(&credential.payload),
            )),
            sanitize_file_component(&credential.id)
        );
    }

    format!(
        "{}/{}.json",
        derive_provider_family_slug(provider_account),
        sanitize_file_component(&credential.id)
    )
}

fn material_kind_folder_slug(kind: String) -> String {
    kind.replace('_', "-")
}

pub(super) fn credential_label_from_path(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| value.to_string())
        .unwrap_or_else(|| "Imported Credential".to_string())
}

pub(super) fn normalize_source_path_key(source_path: &str) -> Option<String> {
    let segments = source_path
        .split(['/', '\\'])
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.is_empty() {
        return None;
    }
    Some(segments.join("/"))
}

pub(super) fn normalize_relative_path(
    root_dir: &Path,
    file_path: &Path,
) -> Result<String, GatewayError> {
    let relative = file_path.strip_prefix(root_dir).map_err(|error| {
        GatewayError::server_error(format!(
            "derive relative provider credential path {}: {error}",
            file_path.display()
        ))
    })?;
    super::paths::relative_key(relative)
}
