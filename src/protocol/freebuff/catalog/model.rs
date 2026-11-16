//! In-memory catalog identities are deliberately not serializable into persistent route configuration.
use super::model_policy::{optional, RootCompactionPolicy};
use super::{atom, CatalogError};
use serde::Deserialize;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ModelAccess {
    Open,
    Locked,
}
#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum DataUse {
    Service,
    Training,
}
#[derive(Deserialize)]
pub struct CatalogBadge {
    pub kind: String,
    pub label: String,
    pub tooltip: Option<String>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelRow {
    key: String,
    handle: String,
    pub display_name: String,
    pub tagline: String,
    pub warning: Option<String>,
    pub badges: Vec<CatalogBadge>,
    pub multimodal: bool,
    pub premium: bool,
    pub data_use: DataUse,
    pub access: ModelAccess,
    pub sort_order: f64,
    opens_at: Option<u64>,
    #[serde(default, deserialize_with = "optional")]
    compaction: Option<RootCompactionPolicy>,
    #[serde(default)]
    legacy_digests: Vec<String>,
}
impl ModelRow {
    pub fn key(&self) -> &str {
        &self.key
    }
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WireCatalog {
    protocol: u8,
    version: String,
    issued_at: u64,
    refresh_at: u64,
    rows: Vec<ModelRow>,
    plans_url: String,
    fetch_id: Option<String>,
}
pub struct ModelCatalog(WireCatalog);
pub struct SelectedModel<'a> {
    pub key: &'a str,
    pub handle: &'a str,
    pub fetch_id: Option<&'a str>,
    pub(super) display_name: &'a str,
    pub(super) compaction: Option<&'a RootCompactionPolicy>,
    pub(super) legacy_digests: &'a [String],
}
impl ModelCatalog {
    pub(super) fn fetch_id(&self) -> Option<&str> {
        self.0.fetch_id.as_deref().filter(|id| !id.is_empty())
    }
    pub fn parse(body: &[u8]) -> Result<Self, CatalogError> {
        if body.len() > super::transport::CATALOG_BODY_LIMIT {
            return Err(CatalogError::ResponseTooLarge);
        }
        let wire: WireCatalog =
            serde_json::from_slice(body).map_err(|_| CatalogError::InvalidResponse)?;
        let mut keys = HashSet::new();
        let mut handles = HashSet::new();
        let mut valid = wire.protocol == 1
            && text(&wire.version, 1024)
            && wire.issued_at <= 9_007_199_254_740_991
            && wire.refresh_at <= 9_007_199_254_740_991
            && text(&wire.plans_url, 8192)
            && !wire.rows.is_empty()
            && wire.rows.len() <= 512
            && wire
                .fetch_id
                .as_deref()
                .is_none_or(|id| id.is_empty() || atom(id, 4096));
        for row in &wire.rows {
            valid &= !row.key.is_empty()
                && text(&row.key, 1024)
                && !row.display_name.is_empty()
                && text(&row.display_name, 4096)
                && row.handle.starts_with("fbm1.")
                && row.handle.len() > 5
                && atom(&row.handle, 16_384)
                && text(&row.tagline, 8192)
                && row.warning.as_deref().is_none_or(|s| text(s, 8192))
                && row.opens_at.is_none_or(|v| v <= 9_007_199_254_740_991)
                && row
                    .compaction
                    .as_ref()
                    .is_none_or(RootCompactionPolicy::valid)
                && row.legacy_digests.len() <= 64
                && row.legacy_digests.iter().all(|s| text(s, 1024))
                && row.badges.len() <= 64
                && row.badges.iter().all(|b| {
                    text(&b.kind, 1024)
                        && text(&b.label, 4096)
                        && b.tooltip.as_deref().is_none_or(|s| text(s, 8192))
                })
                && keys.insert(row.key.as_str())
                && handles.insert(row.handle.as_str());
        }
        if !valid {
            return Err(CatalogError::InvalidResponse);
        }
        let catalog = Self(wire);
        if catalog.rows().next().is_none() {
            return Err(CatalogError::Unsupported);
        }
        Ok(catalog)
    }
    /// Listing and selection use server issue time, never the device's wall clock.
    pub fn rows(&self) -> impl Iterator<Item = &ModelRow> {
        self.0
            .rows
            .iter()
            .filter(|row| row.opens_at.is_none_or(|at| at <= self.0.issued_at))
    }
    pub fn select(&self, key: &str) -> Result<SelectedModel<'_>, CatalogError> {
        let row = self
            .rows()
            .find(|row| row.key == key && row.access == ModelAccess::Open)
            .ok_or(CatalogError::ModelUnavailable)?;
        Ok(SelectedModel {
            key: &row.key,
            handle: &row.handle,
            fetch_id: self.0.fetch_id.as_deref().filter(|id| !id.is_empty()),
            display_name: &row.display_name,
            compaction: row.compaction.as_ref(),
            legacy_digests: &row.legacy_digests,
        })
    }
    /// Advisory refresh time, not a claim that all handles expire at this instant.
    pub fn needs_refresh(&self, now_ms: u64) -> bool {
        now_ms >= self.0.refresh_at
    }
}
fn text(value: &str, max: usize) -> bool {
    value.len() <= max && !value.chars().any(char::is_control)
}
impl std::fmt::Debug for ModelCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FreeBuffModelCatalog")
            .field("rows", &self.0.rows.len())
            .finish_non_exhaustive()
    }
}
