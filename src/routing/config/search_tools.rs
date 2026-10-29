//! Virtual operations reuse ordinary model/credential admission, never a separate pool.
use super::*;

pub(super) fn is_generic(model: &str) -> bool {
    matches!(model, "search" | "fetch")
}

pub(super) fn is_retired(model: &str) -> bool {
    matches!(
        model,
        "linkup-balance" | "search-balance" | "search-api-balance"
    )
}

pub(super) fn add_operation_mappings(provider: &mut CompiledProvider) {
    if provider.payload.canonical_adapter() != "search_api_compatible" {
        return;
    }
    let Some(default) = provider.payload.default_model.as_deref() else {
        return;
    };
    let Some(prefix) = default
        .strip_suffix("-search")
        .or_else(|| default.strip_suffix("-fetch"))
    else {
        return;
    };
    for (operation, supported) in [
        ("search", provider.payload.search_path.is_some()),
        ("fetch", provider.payload.fetch_path.is_some()),
    ] {
        let target = format!("{prefix}-{operation}");
        if supported
            && (provider.supported_models.is_empty()
                || provider
                    .supported_models
                    .iter()
                    .any(|m| m == &target || m == operation))
            && !provider.model_map.contains_key(operation)
            && !provider.model_map_targets.contains_key(operation)
        {
            // A one-target mapping retains strict credential model checks, including
            // when every account supporting this operation is disabled or outside a group.
            provider
                .model_map_targets
                .insert(operation.into(), vec![target]);
        }
    }
}

pub(super) fn allows_model(provider: &CompiledProvider, model: Option<&str>) -> bool {
    let Some(model) = model else { return true };
    !is_retired(model)
        && (!is_generic(model)
            || (provider.payload.canonical_adapter() == "search_api_compatible"
                && match model {
                    "search" => provider.payload.search_path.is_some(),
                    _ => provider.payload.fetch_path.is_some(),
                }
                && (provider.model_map.contains_key(model)
                    || provider.model_map_targets.contains_key(model))))
}
