//! Legacy import is a material boundary, not a new grant, client identity or refresh scheduler.
use super::{
    parse_absolute_expiry, valid_atom, valid_cookie, OAuthClient, GATEWAY, REFRESH_ENDPOINT,
};
use crate::error::GatewayError;
use serde_json::{Map, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};

pub(crate) struct ImportMaterial {
    refresh: Option<String>,
    sgcookie: Option<String>,
    client: Option<String>,
    endpoint: Option<String>,
    expiry: Option<String>,
}

pub(crate) fn access(raw: &Map<String, Value>) -> Result<Option<String>, GatewayError> {
    let mut values = fields(raw, &["accessToken", "access_token", "apiKey", "api_key"]);
    // The old export contract recognizes body token only inside extraBody, not arbitrary root data.
    for map in extra_maps(raw) {
        values.extend(map.get("token"));
    }
    strings(values, |value| valid_atom(value, 16_384))
}

impl ImportMaterial {
    pub(crate) fn parse(
        raw: &Map<String, Value>,
        device: &str,
        base: &str,
    ) -> Result<Self, GatewayError> {
        if !valid_atom(device, 1024) {
            return Err(invalid());
        }
        Self::parse_refresh(raw, base)
    }

    /// Refresh sends no device field; keep material validation shared without inventing one.
    pub(crate) fn parse_refresh(
        raw: &Map<String, Value>,
        base: &str,
    ) -> Result<Self, GatewayError> {
        if url::Url::parse(base).ok() != url::Url::parse(GATEWAY).ok() {
            return Err(invalid());
        }
        let refresh = strings(fields(raw, &["refreshToken", "refresh_token"]), |value| {
            valid_atom(value, 16_384)
        })?;
        let sgcookie = strings(
            fields(raw, &["accioSgcookie", "accio_sgcookie", "sgcookie"]),
            valid_cookie,
        )?;
        let client = strings(
            fields(
                raw,
                &[
                    "refreshClientId",
                    "refresh_client_id",
                    "clientId",
                    "client_id",
                ],
            ),
            |value| OAuthClient::new(value).is_ok(),
        )?;
        let mut endpoint = strings(
            fields(raw, &["refreshEndpoint", "refresh_endpoint"]),
            |value| value == REFRESH_ENDPOINT,
        )?;
        // A known endpoint is not permission to invent a first-party client for old exports.
        if endpoint.is_none() && refresh.is_some() && client.is_some() {
            endpoint = Some(REFRESH_ENDPOINT.into());
        }
        let mut expiry: Option<OffsetDateTime> = None;
        for value in fields(raw, &["expiresAt", "expires_at"])
            .into_iter()
            .filter(|value| !value.is_null())
        {
            // Value has already parsed JSON numbers: fractional f64s cannot retain original nanos.
            // Accept exact integer seconds/ms or decimal strings, never round into a later deadline.
            if !(value.as_u64().is_some() || value.as_str().is_some_and(|text| text.len() <= 128)) {
                return Err(invalid());
            }
            let raw = serde_json::value::to_raw_value(value).map_err(|_| invalid())?;
            let parsed = parse_absolute_expiry(&raw).ok_or_else(invalid)?;
            if expiry.is_some_and(|previous| previous != parsed) {
                return Err(invalid());
            }
            expiry = Some(parsed);
        }
        let expiry = expiry
            .map(|at| at.format(&Rfc3339).map_err(|_| invalid()))
            .transpose()?;
        Ok(Self {
            refresh,
            sgcookie,
            client,
            endpoint,
            expiry,
        })
    }

    pub(crate) fn apply(self, payload: &mut Map<String, Value>) {
        for (key, value) in [
            ("refreshToken", self.refresh),
            ("accioSgcookie", self.sgcookie),
            ("refreshClientId", self.client),
            ("refreshEndpoint", self.endpoint),
            ("expiresAt", self.expiry),
        ] {
            if let Some(value) = value {
                payload.insert(key.into(), Value::String(value));
            }
        }
    }
}

fn extra_maps(raw: &Map<String, Value>) -> impl Iterator<Item = &Map<String, Value>> {
    ["extraBody", "extra_body"]
        .into_iter()
        .filter_map(move |key| raw.get(key).and_then(Value::as_object))
}
fn fields<'a>(raw: &'a Map<String, Value>, keys: &[&str]) -> Vec<&'a Value> {
    std::iter::once(raw)
        .chain(extra_maps(raw))
        .flat_map(|map| keys.iter().filter_map(move |key| map.get(*key)))
        .collect()
}
fn strings(
    values: Vec<&Value>,
    validate: impl Fn(&str) -> bool,
) -> Result<Option<String>, GatewayError> {
    let mut selected: Option<String> = None;
    for value in values.into_iter().filter(|value| !value.is_null()) {
        let value = value
            .as_str()
            .filter(|value| validate(value))
            .ok_or_else(invalid)?;
        if selected
            .as_deref()
            .is_some_and(|previous| previous != value)
        {
            return Err(invalid());
        }
        selected = Some(value.into());
    }
    Ok(selected)
}
fn invalid() -> GatewayError {
    GatewayError::bad_request("Accio 导入的授权材料或受众无效。")
        .with_code("accio_import_invalid_material")
}
