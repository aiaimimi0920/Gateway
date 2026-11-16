//! Explicit browser-session save reads the account directory; it never generates or marks models verified.
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
            .ok_or("lobsterai_login_save_rejected")?;
        let tokens = super::stored(credential).map_err(|_| "lobsterai_login_save_rejected")?;
        let catalog = crate::protocol::lobsterai::ModelCatalog::new(
            tokens
                .model_catalog()
                .await
                .map_err(|_| "lobsterai_catalog_read_failed")?,
        )
        .map_err(|_| "lobsterai_catalog_invalid")?;
        let previous: std::collections::HashSet<_> = credential
            .lobsterai_catalog
            .as_ref()
            .map(|c| c.models().iter().map(|m| m.model_id.clone()).collect())
            .unwrap_or_default();
        let mut selected: Vec<_> = credential
            .supported_models
            .iter()
            .filter(|id| !previous.contains(*id))
            .cloned()
            .collect();
        for model in catalog.models() {
            if !selected.contains(&model.model_id) {
                selected.push(model.model_id.clone());
            }
        }
        if let Some(selection) = &mut credential.model_selection {
            selection.fetched = catalog
                .models()
                .iter()
                .map(|m| m.model_id.clone())
                .collect();
            selected = selection
                .models()
                .map_err(|_| "lobsterai_catalog_invalid")?;
        }
        credential.lobsterai_catalog = Some(catalog);
        credential.supported_models = selected;
        crate::console::document::validate_route_document(self.document.clone())
            .map_err(|_| "lobsterai_catalog_invalid")?;
        Ok(())
    }
}
