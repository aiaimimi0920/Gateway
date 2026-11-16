//! Provider declarations select the wire protocol, never a guessed model name or a probe receipt.
use super::{reply, OAuthError};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

const MAX_MODELS: usize = 2048;
const MAX_BYTES: usize = 2 * 1024 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WireProtocol {
    Chat,
    Responses,
    Messages,
    Gemini,
}
impl WireProtocol {
    fn from_declared(value: &str) -> Option<Self> {
        match value {
            "openai" => Some(Self::Chat),
            "openai-response" => Some(Self::Responses),
            "anthropic" => Some(Self::Messages),
            "gemini" => Some(Self::Gemini),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeclaredModel {
    pub id: String,
    pub name: String,
    pub protocol: WireProtocol,
    pub context_window: Option<u64>,
    pub max_output_tokens: Option<u64>,
    pub tools: Option<bool>,
    pub reasoning: Option<bool>,
    pub reasoning_efforts: Vec<String>,
    pub input_modalities: Vec<String>,
    pub output_modalities: Vec<String>,
    pub service_tiers: Vec<String>,
    pub deprecated: bool,
    pub zdr: bool,
}

pub fn parse_catalog(bytes: &[u8]) -> Result<Vec<DeclaredModel>, OAuthError> {
    let value = reply::object(bytes, MAX_BYTES)?;
    let data = value["data"]
        .as_array()
        .ok_or(OAuthError::InvalidResponse)?;
    if data.len() > MAX_MODELS {
        return Err(OAuthError::ResponseTooLarge);
    }
    let mut seen = std::collections::HashSet::new();
    let mut models = Vec::new();
    for item in data {
        let raw = object(item)?;
        let id = text(raw.get("id"), 256)?;
        if !seen.insert(id.to_owned()) {
            return Err(OAuthError::InvalidResponse);
        }
        let config = match raw.get("dim") {
            None | Some(Value::Null) => raw,
            Some(config) => object(config)?,
        };
        if config.get("id").is_some_and(|v| v.as_str() != Some(id)) {
            return Err(OAuthError::InvalidResponse);
        }
        if config.get("status").and_then(Value::as_str) == Some("unavailable") {
            continue;
        }
        if let Some(protocol) = protocol(raw, config)? {
            models.push(model(id, raw, config, protocol)?);
        }
    }
    if !data.is_empty() && models.is_empty() {
        return Err(OAuthError::NoSupportedModels);
    }
    Ok(models)
}

fn protocol(
    raw: &Map<String, Value>,
    config: &Map<String, Value>,
) -> Result<Option<WireProtocol>, OAuthError> {
    let explicit = first(config, &["endpoint_type"])
        .or_else(|| first(raw, &["endpointType", "endpoint_type"]));
    if let Some(value) = explicit {
        return Ok(WireProtocol::from_declared(text(Some(value), 128)?));
    }
    let declared = first(
        config,
        &[
            "selectableEndpointTypes",
            "selectable_endpoint_types",
            "supportedEndpointTypes",
            "supported_endpoint_types",
        ],
    );
    match declared {
        None => Ok(Some(WireProtocol::Chat)),
        Some(value) => Ok(strings(Some(value), &[])?
            .iter()
            .find_map(|v| WireProtocol::from_declared(v))),
    }
}

fn model(
    id: &str,
    raw: &Map<String, Value>,
    config: &Map<String, Value>,
    protocol: WireProtocol,
) -> Result<DeclaredModel, OAuthError> {
    let empty = Map::new();
    let limit = optional_object(config.get("limit"), &empty)?;
    let extra = optional_object(config.get("extra_capabilities"), &empty)?;
    let reason = extra.get("reasoning").or_else(|| config.get("reasoning"));
    let info = reason
        .filter(|v| v.is_object())
        .map(object)
        .transpose()?
        .unwrap_or(&empty);
    let efforts = first(info, &["effortOptions", "effort_options"]);
    let reasoning = if reason.is_some_and(Value::is_object) {
        optional_bool(info.get("supported"))?.or(efforts.map(|_| true))
    } else {
        optional_bool(reason)?
    };
    let modalities = config.get("modalities");
    let modal = if modalities.is_some_and(Value::is_array) {
        &empty
    } else {
        optional_object(modalities, &empty)?
    };
    let default_input = if config.get("vision") == Some(&Value::Bool(true)) {
        &["text", "image"][..]
    } else {
        &["text"][..]
    };
    let input = strings(
        modalities
            .filter(|v| v.is_array())
            .or_else(|| modal.get("input")),
        default_input,
    )?;
    let tier = first(config, &["serviceTier"])
        .or_else(|| first(raw, &["serviceTier", "service_tier"]))
        .or_else(|| first(extra, &["serviceTier", "service_tier"]));
    let tiers = tier
        .filter(|v| v.is_object())
        .map(object)
        .transpose()?
        .and_then(|v| v.get("options"));
    let name = first(config, &["displayName", "display_name", "name"])
        .or_else(|| first(raw, &["display_name", "name"]));
    Ok(DeclaredModel {
        id: id.into(),
        name: name
            .map(|v| text(Some(v), 1024))
            .transpose()?
            .unwrap_or(id)
            .into(),
        protocol,
        context_window: limit_number(
            first(config, &["context_window", "contextWindow"]).or_else(|| limit.get("context")),
        )?,
        max_output_tokens: limit_number(
            first(config, &["max_output_tokens", "maxOutputTokens"])
                .or_else(|| limit.get("output")),
        )?,
        tools: optional_bool(first(config, &["tool_call", "toolCalling"]))?,
        reasoning,
        reasoning_efforts: strings(efforts, &[])?,
        input_modalities: input,
        output_modalities: strings(modal.get("output"), &["text"])?,
        service_tiers: strings(tiers, &[])?
            .into_iter()
            .filter(|v| v != "default")
            .collect(),
        deprecated: config.get("status").and_then(Value::as_str) == Some("deprecated"),
        zdr: raw.get("zdr") == Some(&Value::Bool(true)),
    })
}

fn object(value: &Value) -> Result<&Map<String, Value>, OAuthError> {
    value.as_object().ok_or(OAuthError::InvalidResponse)
}
fn optional_object<'a>(
    value: Option<&'a Value>,
    empty: &'a Map<String, Value>,
) -> Result<&'a Map<String, Value>, OAuthError> {
    value.map(object).transpose().map(|v| v.unwrap_or(empty))
}
fn first<'a>(map: &'a Map<String, Value>, keys: &[&str]) -> Option<&'a Value> {
    keys.iter()
        .find_map(|k| map.get(*k).filter(|v| !v.is_null()))
}
fn text(value: Option<&Value>, max: usize) -> Result<&str, OAuthError> {
    value
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty() && v.len() <= max && !v.chars().any(char::is_control))
        .ok_or(OAuthError::InvalidResponse)
}
fn optional_bool(value: Option<&Value>) -> Result<Option<bool>, OAuthError> {
    value
        .map(|v| v.as_bool().ok_or(OAuthError::InvalidResponse))
        .transpose()
}
fn limit_number(value: Option<&Value>) -> Result<Option<u64>, OAuthError> {
    value
        .map(|v| v.as_u64().ok_or(OAuthError::InvalidResponse))
        .transpose()
        .map(|n| n.filter(|n| *n > 0))
}
fn strings(value: Option<&Value>, default: &[&str]) -> Result<Vec<String>, OAuthError> {
    let Some(value) = value else {
        return Ok(default.iter().map(|s| (*s).into()).collect());
    };
    let list = value
        .as_array()
        .filter(|a| a.len() <= 32)
        .ok_or(OAuthError::InvalidResponse)?;
    let mut out = Vec::new();
    for value in list {
        let value = text(Some(value), 128)?.to_owned();
        if !out.contains(&value) {
            out.push(value);
        }
    }
    Ok(out)
}
