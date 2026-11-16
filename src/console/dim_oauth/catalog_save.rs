//! Explicit OAuth save reads the account directory; it never generates or marks models verified.
use super::{persistence::PreparedSave, Binding};

impl PreparedSave {
    pub(super) async fn load_catalog(&mut self, binding: &Binding) -> Result<(), &'static str> {
        let credential = self
            .document
            .providers
            .iter_mut()
            .find(|p| p.id == binding.provider)
            .and_then(|p| {
                p.credentials
                    .iter_mut()
                    .find(|c| c.id.as_deref() == Some(self.credential_id.as_str()))
            })
            .ok_or("dim_login_save_rejected")?;
        let context = credential
            .dim_oauth
            .as_ref()
            .ok_or("dim_login_save_rejected")?;
        let http = rquest::Client::builder()
            .build()
            .map_err(|_| "dim_catalog_transport_unavailable")?;
        let catalog = context
            .model_catalog(
                &http,
                credential
                    .api_key
                    .as_deref()
                    .ok_or("dim_login_save_rejected")?,
            )
            .await
            .map_err(|_| "dim_catalog_read_failed")?;
        let previous: std::collections::HashSet<_> = credential
            .dim_catalog
            .as_ref()
            .map(|c| c.models().iter().map(|m| m.id.clone()).collect())
            .unwrap_or_default();
        let mut selected: Vec<_> = credential
            .supported_models
            .iter()
            .filter(|id| !previous.contains(*id))
            .cloned()
            .collect();
        for model in catalog.models() {
            if !selected.contains(&model.id) {
                selected.push(model.id.clone());
            }
        }
        if let Some(selection) = &mut credential.model_selection {
            selection.fetched = catalog.models().iter().map(|m| m.id.clone()).collect();
            selected = selection.models().map_err(|_| "dim_catalog_invalid")?;
        }
        credential.dim_catalog = Some(catalog);
        credential.supported_models = selected;
        crate::console::document::validate_route_document(self.document.clone())
            .map_err(|_| "dim_catalog_invalid")?;
        Ok(())
    }
}
