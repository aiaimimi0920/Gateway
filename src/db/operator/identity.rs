//! Source profiles and operator-visible provider identity filters.

use super::*;

pub(super) fn source_profile_from_account(
    provider_account: &GatewayProviderAccountView,
) -> GatewaySourceProfileView {
    GatewaySourceProfileView {
        source_kind: provider_account
            .source_kind
            .clone()
            .unwrap_or_else(|| "unknown".to_string()),
        aggregator_api_mode: provider_account.aggregator_api_mode.clone(),
        web_reverse_access_mode: provider_account.web_reverse_access_mode.clone(),
        source_notes: provider_account.source_notes.clone(),
        derived: provider_account.source_kind.is_none(),
    }
}

pub(super) fn filter_visible_provider_accounts(
    provider_accounts: Vec<GatewayProviderAccountView>,
) -> Vec<GatewayProviderAccountView> {
    provider_accounts
        .into_iter()
        .filter(|provider| !provider_is_hidden_from_operator_inventory(provider))
        .collect()
}

pub(super) fn provider_is_hidden_from_operator_inventory(
    provider_account: &GatewayProviderAccountView,
) -> bool {
    let Some(payload) = provider_account.payload.as_object() else {
        return false;
    };

    let hidden_flag = [
        "hiddenFromOperatorInventory",
        "hiddenFromInventory",
        "internalOnly",
    ]
    .iter()
    .any(|key| payload.get(*key).and_then(Value::as_bool).unwrap_or(false));
    if hidden_flag {
        return true;
    }

    if provider_account
        .source_notes
        .as_deref()
        .map(str::to_ascii_lowercase)
        .is_some_and(|notes| {
            notes.contains("hidden_from_operator_inventory")
                || notes.contains("internal_fixture")
                || notes.contains("local_fixture")
        })
    {
        return true;
    }

    read_provider_default_model(&provider_account.payload)
        .is_some_and(|model| model == "xml-fallback-fixture")
}

pub(super) fn read_provider_default_model(payload: &Value) -> Option<String> {
    payload
        .as_object()
        .and_then(|record| record.get("defaultModel"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
}
