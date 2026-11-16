//! Persist only explicit account-owned business authority; catalog handles are never configuration.
use super::catalog::{BusinessOrigin, ClientSurface};
use crate::{error::GatewayError, routing::candidate::ProviderAccountPayload};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

pub(crate) const CONFIG_KEY: &str = "freebuffBusiness";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BusinessTarget {
    pub origin: BusinessOrigin,
    pub client: ClientSurface,
}

pub(crate) fn is_config_key(key: &str) -> bool {
    super::normalize_lookup_key(key) == "freebuffbusiness"
}

pub(crate) fn read(
    extra: Option<&HashMap<String, Value>>,
) -> Result<Option<BusinessTarget>, GatewayError> {
    let Some(extra) = extra else { return Ok(None) };
    // Runtime option aliases must not create an ignored target or a second, inherited authority.
    if extra
        .keys()
        .any(|key| is_config_key(key) && key != CONFIG_KEY)
    {
        return Err(invalid());
    }
    extra
        .get(CONFIG_KEY)
        .map(|value| serde_json::from_value(value.clone()).map_err(|_| invalid()))
        .transpose()
}

pub(crate) fn invalid() -> GatewayError {
    GatewayError::bad_request(
        "FreeBuff business target requires one explicit account-owned site and client.",
    )
    .with_code("freebuff_business_target_invalid")
}

/// A persisted catalog key must not reach the legacy agent/session wire or silently buy admission.
/// The catalog-aware execution owner will replace this boundary when consent and signed sending exist.
pub(crate) fn reject_legacy_execution(
    payload: &ProviderAccountPayload,
) -> Result<(), GatewayError> {
    if read(payload.extra_body.as_ref())?.is_some() {
        return Err(GatewayError::conflict("FreeBuff catalog business execution requires explicit admission and signed sending; the legacy executor cannot use this selection.")
            .with_code("freebuff_business_admission_required")
            .with_provider("freebuff_compatible"));
    }
    Ok(())
}
