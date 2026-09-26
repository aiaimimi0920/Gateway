//! Index provider accounts and preserve ranking/tie-breaking for folder imports.
use super::canonicalization::{
    canonicalize_folder_family_slug, canonicalize_folder_surface_slug, sanitize_file_component,
    service_surface_lookup_key,
};
use super::classification::ProviderAccountDescriptor;
use super::classification::{
    derive_provider_family_slug, derive_provider_surface_slug, derive_service_provider_slug,
};
use super::layout::FolderImportPathDescriptor;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub(super) struct FolderSyncProviderMaps {
    legacy_family_map: HashMap<String, Vec<String>>,
    surface_map: HashMap<String, Vec<String>>,
    service_surface_map: HashMap<String, Vec<String>>,
}

pub(super) fn build_provider_maps<A: ProviderAccountDescriptor>(
    provider_accounts: &[A],
) -> FolderSyncProviderMaps {
    FolderSyncProviderMaps {
        legacy_family_map: build_provider_legacy_family_map(provider_accounts),
        surface_map: build_provider_surface_map(provider_accounts),
        service_surface_map: build_provider_service_surface_map(provider_accounts),
    }
}

fn build_provider_legacy_family_map<A: ProviderAccountDescriptor>(
    provider_accounts: &[A],
) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut grouped: HashMap<String, Vec<&A>> = HashMap::new();
    for account in provider_accounts {
        grouped
            .entry(derive_provider_family_slug(account))
            .or_default()
            .push(account);
    }
    for (family, mut accounts) in grouped {
        accounts.sort_by(|left, right| {
            provider_family_selection_score(*right, family.as_str())
                .cmp(&provider_family_selection_score(*left, family.as_str()))
                .then_with(|| left.label().cmp(right.label()))
        });
        map.insert(
            family,
            accounts
                .into_iter()
                .map(|account| account.id().to_string())
                .collect::<Vec<_>>(),
        );
    }
    map
}

fn build_provider_surface_map<A: ProviderAccountDescriptor>(
    provider_accounts: &[A],
) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut grouped: HashMap<String, Vec<&A>> = HashMap::new();
    for account in provider_accounts {
        grouped
            .entry(derive_provider_surface_slug(account))
            .or_default()
            .push(account);
    }
    for (surface, mut accounts) in grouped {
        accounts.sort_by(|left, right| {
            provider_surface_selection_score(*right, surface.as_str())
                .cmp(&provider_surface_selection_score(*left, surface.as_str()))
                .then_with(|| left.label().cmp(right.label()))
        });
        map.insert(
            surface,
            accounts
                .into_iter()
                .map(|account| account.id().to_string())
                .collect::<Vec<_>>(),
        );
    }
    map
}

fn build_provider_service_surface_map<A: ProviderAccountDescriptor>(
    provider_accounts: &[A],
) -> HashMap<String, Vec<String>> {
    let mut map: HashMap<String, Vec<String>> = HashMap::new();
    let mut grouped: HashMap<String, Vec<&A>> = HashMap::new();
    for account in provider_accounts {
        grouped
            .entry(service_surface_lookup_key(
                derive_service_provider_slug(account).as_str(),
                derive_provider_surface_slug(account).as_str(),
            ))
            .or_default()
            .push(account);
    }
    for (key, mut accounts) in grouped {
        let (_, surface) = key.split_once("::").unwrap_or((key.as_str(), key.as_str()));
        accounts.sort_by(|left, right| {
            provider_surface_selection_score(*right, surface)
                .cmp(&provider_surface_selection_score(*left, surface))
                .then_with(|| left.label().cmp(right.label()))
        });
        map.insert(
            key,
            accounts
                .into_iter()
                .map(|account| account.id().to_string())
                .collect::<Vec<_>>(),
        );
    }
    map
}

fn select_provider_account_for_family(
    family_map: &HashMap<String, Vec<String>>,
    family_slug: &str,
) -> Option<String> {
    let family_slug = canonicalize_folder_family_slug(family_slug);
    family_map
        .get(family_slug.as_str())
        .and_then(|items| items.first())
        .cloned()
}

pub(super) fn select_provider_account_for_path(
    provider_maps: &FolderSyncProviderMaps,
    descriptor: &FolderImportPathDescriptor,
) -> Option<String> {
    if let Some(service_provider_slug) = descriptor.service_provider_slug.as_deref() {
        let service_surface_key = service_surface_lookup_key(
            service_provider_slug,
            descriptor.provider_surface_slug.as_str(),
        );
        if let Some(provider_account_id) = provider_maps
            .service_surface_map
            .get(service_surface_key.as_str())
            .and_then(|items| items.first())
            .cloned()
        {
            return Some(provider_account_id);
        }
    }

    provider_maps
        .surface_map
        .get(descriptor.provider_surface_slug.as_str())
        .and_then(|items| items.first())
        .cloned()
        .or_else(|| {
            select_provider_account_for_family(
                &provider_maps.legacy_family_map,
                descriptor.provider_surface_slug.as_str(),
            )
        })
}

fn provider_family_selection_score<A: ProviderAccountDescriptor + ?Sized>(
    provider_account: &A,
    family_slug: &str,
) -> i32 {
    let family_slug = canonicalize_folder_family_slug(family_slug);
    let payload_base_url = provider_account
        .payload_base_url()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let label = provider_account.label().to_ascii_lowercase();

    match family_slug.as_str() {
        "accio" => {
            let mut score = 0;
            if provider_account
                .adapter()
                .eq_ignore_ascii_case("accio_compatible")
            {
                score += 100;
            }
            if provider_account
                .protocol_profile()
                .eq_ignore_ascii_case("accio")
            {
                score += 50;
            }
            if provider_account
                .service_provider_key()
                .eq_ignore_ascii_case("accio_platform")
            {
                score += 20;
            }
            if payload_base_url.contains("phoenix-gw.alibaba.com") {
                score += 25;
            }
            if label.contains("accio") {
                score += 10;
            }
            score
        }
        "codex" => {
            let mut score = 0;
            if payload_base_url.contains("/backend-api/codex") {
                score += 100;
            }
            if label.contains("codex") {
                score += 20;
            }
            if provider_account.protocol_family() == "openai" {
                score += 5;
            }
            score
        }
        "qwen-web-chat" => {
            let mut score = 0;
            if provider_account
                .adapter()
                .eq_ignore_ascii_case("qwen_web_compatible")
            {
                score += 100;
            }
            if provider_account
                .protocol_profile()
                .eq_ignore_ascii_case("qwen_web_chat")
            {
                score += 50;
            }
            if provider_account
                .protocol_family()
                .eq_ignore_ascii_case("qwen_web_chat")
            {
                score += 25;
            }
            if provider_account
                .service_provider_key()
                .eq_ignore_ascii_case("qwen_platform")
            {
                score += 15;
            }
            if label.contains("qwen") && label.contains("web") {
                score += 10;
            }
            if payload_base_url.contains("chat.qwen.ai") {
                score += 20;
            }
            score
        }
        _ => {
            let mut score = 0;
            if label.contains(family_slug.as_str()) {
                score += 10;
            }
            if payload_base_url.contains(family_slug.as_str()) {
                score += 10;
            }
            score
        }
    }
}

fn provider_surface_selection_score<A: ProviderAccountDescriptor + ?Sized>(
    provider_account: &A,
    surface_slug: &str,
) -> i32 {
    let surface_slug = canonicalize_folder_surface_slug(surface_slug);
    let derived_surface_slug = derive_provider_surface_slug(provider_account);
    let protocol_profile_slug = sanitize_file_component(provider_account.protocol_profile());
    let label = provider_account.label().to_ascii_lowercase();

    let mut score = 0;
    if derived_surface_slug == surface_slug {
        score += 100;
    }
    if protocol_profile_slug == surface_slug {
        score += 30;
    }
    if label.contains(surface_slug.as_str()) {
        score += 10;
    }
    score
}
