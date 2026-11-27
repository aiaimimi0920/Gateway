//! Model catalog, server HTTP/2 route config, and parameterized selector verification.
use super::{
    json_wire::{parse_json, proto_field, wire_fields},
    protobuf::fields,
    transport::{CheckedClient, ENDPOINTS},
    ProbeError,
};
use serde_json::{Map, Value};
use std::collections::HashSet;

const AVAILABLE_MODELS_REQ: [u8; 4] = [0x28, 0x01, 0x38, 0x01];
const HTTP2_NAMES: [&str; 5] = [
    "UNSPECIFIED",
    "FORCE_ALL_DISABLED",
    "FORCE_ALL_ENABLED",
    "FORCE_BIDI_DISABLED",
    "FORCE_BIDI_ENABLED",
];
const UPDATE_LEVELS: [&str; 4] = ["UNSPECIFIED", "NONE", "RECOMMENDED", "REQUIRED"];

#[derive(Debug, Clone)]
pub(crate) struct CatalogSummary {
    pub(crate) default_model: Option<String>,
    pub(crate) default_listed: bool,
    pub(crate) selector_safe: bool,
}

#[derive(Debug, Clone)]
struct ModelDetails {
    id: String,
    unsafe_variant: bool,
}

pub(crate) fn valid_model_id(id: &str) -> bool {
    let mut bytes = id.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    id.len() <= 160
        && first.is_ascii_alphanumeric()
        && bytes.all(|b| b.is_ascii_alphanumeric() || b"._:/+-".contains(&b))
}

pub(crate) async fn read_agent_catalog(
    token: &str,
    client: &CheckedClient<'_>,
) -> Result<CatalogSummary, ProbeError> {
    let available = client
        .post_connect_json(ENDPOINTS.models, token, "{}")
        .await?;
    let available_map = parse_json(available.status, &available.content_type, &available.body)?;
    let selected = client
        .post_connect_json(ENDPOINTS.default_model, token, "{}")
        .await?;
    let selected_map = parse_json(selected.status, &selected.content_type, &selected.body)?;
    catalog_summary(&available_map, &selected_map)
}

pub(crate) fn require_account_model(catalog: &CatalogSummary) -> Result<String, ProbeError> {
    let Some(model) = catalog
        .default_model
        .as_deref()
        .filter(|id| valid_model_id(id))
    else {
        return Err(ProbeError::new("account_default_model_not_verified"));
    };
    if !catalog.default_listed || !catalog.selector_safe {
        return Err(ProbeError::new("account_default_model_not_verified"));
    }
    Ok(model.to_owned())
}

fn catalog_summary(
    available: &Map<String, Value>,
    selected: &Map<String, Value>,
) -> Result<CatalogSummary, ProbeError> {
    let avail_fields = wire_fields(available, &["models"])?;
    let list = match avail_fields.get("models") {
        None => &[][..],
        Some(Value::Array(items)) => items.as_slice(),
        _ => return Err(ProbeError::new("invalid_model_catalog")),
    };
    let mut parsed_models = Vec::with_capacity(list.len());
    for entry in list {
        parsed_models.push(parse_model_details(entry)?);
    }
    let sel_fields = wire_fields(selected, &["model"])?;
    let chosen = match sel_fields.get("model") {
        None => None,
        Some(entry) => Some(parse_model_details(entry)?),
    };
    let Some(chosen) = chosen else {
        return Ok(CatalogSummary {
            default_model: None,
            default_listed: false,
            selector_safe: false,
        });
    };
    let matches: Vec<&ModelDetails> = parsed_models.iter().filter(|m| m.id == chosen.id).collect();
    let selector_safe =
        matches.len() == 1 && !chosen.unsafe_variant && matches.iter().all(|m| !m.unsafe_variant);
    Ok(CatalogSummary {
        default_listed: !matches.is_empty(),
        selector_safe,
        default_model: Some(chosen.id),
    })
}

fn parse_model_details(entry: &Value) -> Result<ModelDetails, ProbeError> {
    let invalid = || ProbeError::new("invalid_model_details");
    let obj = entry.as_object().ok_or_else(invalid)?;
    let fields = wire_fields(
        obj,
        &[
            "modelId",
            "thinkingDetails",
            "displayModelId",
            "displayName",
            "displayNameShort",
            "aliases",
            "maxMode",
            "apiKeyCredentials",
            "azureCredentials",
            "bedrockCredentials",
        ],
    )?;
    let id = fields
        .get("modelId")
        .and_then(|v| v.as_str())
        .filter(|s| valid_model_id(s))
        .ok_or_else(invalid)?;
    for key in ["displayModelId", "displayName", "displayNameShort"] {
        if let Some(val) = fields.get(key) {
            if !val.is_string() {
                return Err(invalid());
            }
        }
    }
    if let Some(val) = fields.get("aliases") {
        if !val
            .as_array()
            .is_some_and(|arr| arr.iter().all(Value::is_string))
        {
            return Err(invalid());
        }
    }
    let max_mode = match fields.get("maxMode") {
        None => false,
        Some(val) => val.as_bool().ok_or_else(invalid)?,
    };
    let credential_variant = [
        "apiKeyCredentials",
        "azureCredentials",
        "bedrockCredentials",
    ]
    .iter()
    .any(|k| fields.contains_key(k));
    let thinking_variant = fields.contains_key("thinkingDetails");
    Ok(ModelDetails {
        id: id.to_owned(),
        unsafe_variant: credential_variant || thinking_variant || max_mode,
    })
}

pub(crate) async fn verify_server_route(
    token: &str,
    client: &CheckedClient<'_>,
) -> Result<(), ProbeError> {
    let resp = client
        .post_connect_json(ENDPOINTS.server_config, token, "{}")
        .await?;
    let map = parse_json(resp.status, &resp.content_type, &resp.body)?;
    let invalid = || ProbeError::new("invalid_server_config");

    let h2_name = match proto_field(&map, "http2Config", "invalid_server_config")? {
        None => "UNSPECIFIED",
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|idx| usize::try_from(idx).ok())
            .and_then(|idx| HTTP2_NAMES.get(idx).copied())
            .ok_or_else(invalid)?,
        Some(Value::String(s)) => {
            let trimmed = s.strip_prefix("HTTP2_CONFIG_").unwrap_or(s.as_str());
            HTTP2_NAMES
                .iter()
                .copied()
                .find(|&item| item == trimmed)
                .ok_or_else(invalid)?
        }
        _ => return Err(invalid()),
    };
    let _ = proto_field(&map, "configVersion", "invalid_server_config")?;

    let routes = match proto_field(&map, "agentUrlConfig", "invalid_server_config")? {
        None => None,
        Some(Value::Object(obj)) => Some(obj),
        _ => return Err(invalid()),
    };
    validate_client_version_status(proto_field(
        &map,
        "clientVersionStatus",
        "invalid_server_config",
    )?)?;

    let bidi_disabled = matches!(h2_name, "FORCE_ALL_DISABLED" | "FORCE_BIDI_DISABLED");
    let Some(routes) = routes else {
        return Err(ProbeError::new("route_unconfirmed"));
    };
    let agent_ok = is_current_run_origin(proto_field(routes, "agentUrl", "invalid_server_config")?);
    let agentn_ok =
        is_current_run_origin(proto_field(routes, "agentnUrl", "invalid_server_config")?);
    let loop_ok = is_current_run_origin(proto_field(
        routes,
        "localLoopInferenceUrl",
        "invalid_server_config",
    )?);
    if bidi_disabled || !agent_ok || !agentn_ok || !loop_ok {
        return Err(ProbeError::new("route_unconfirmed"));
    }
    Ok(())
}

fn is_current_run_origin(value: Option<&Value>) -> bool {
    matches!(
        value.and_then(Value::as_str),
        Some("https://agentn.global.api5.cursor.sh" | "https://agentn.global.api5.cursor.sh/")
    )
}

fn validate_client_version_status(status: Option<&Value>) -> Result<(), ProbeError> {
    let invalid = || ProbeError::new("invalid_server_config");
    let empty = Map::new();
    let obj = match status {
        None => &empty,
        Some(Value::Object(map)) => map,
        _ => return Err(invalid()),
    };
    let level = match proto_field(obj, "updateLevel", "invalid_server_config")? {
        None => "UNSPECIFIED",
        Some(Value::Number(n)) => n
            .as_u64()
            .and_then(|idx| usize::try_from(idx).ok())
            .and_then(|idx| UPDATE_LEVELS.get(idx).copied())
            .ok_or_else(invalid)?,
        Some(Value::String(s)) => {
            let trimmed = s.strip_prefix("CLIENT_UPDATE_LEVEL_").unwrap_or(s.as_str());
            UPDATE_LEVELS
                .iter()
                .copied()
                .find(|&item| item == trimmed)
                .ok_or_else(invalid)?
        }
        _ => return Err(invalid()),
    };
    let _ = level;
    for (name, max_bytes) in [
        ("currentClientVersion", 256usize),
        ("minSupportedClientVersion", 256),
        ("minAllowedClientVersion", 256),
        ("message", 4096),
    ] {
        if let Some(raw) = proto_field(obj, name, "invalid_server_config")? {
            if !raw.as_str().is_some_and(|s| s.len() <= max_bytes) {
                return Err(invalid());
            }
        }
    }
    Ok(())
}

pub(crate) async fn verify_model_selection(
    token: &str,
    agent_catalog: &CatalogSummary,
    client: &CheckedClient<'_>,
) -> Result<(), ProbeError> {
    let (cli_avail_res, cli_sel_res, param_res) = tokio::join!(
        client.post_connect_json(ENDPOINTS.cli_models, token, "{}"),
        client.post_connect_json(ENDPOINTS.cli_default_model, token, "{}"),
        client.post_connect_proto(ENDPOINTS.parameter_models, token, &AVAILABLE_MODELS_REQ),
    );
    let cli_avail_resp = cli_avail_res?;
    let cli_sel_resp = cli_sel_res?;
    let param_resp = param_res?;

    let cli_avail = parse_json(
        cli_avail_resp.status,
        &cli_avail_resp.content_type,
        &cli_avail_resp.body,
    )?;
    let cli_sel = parse_json(
        cli_sel_resp.status,
        &cli_sel_resp.content_type,
        &cli_sel_resp.body,
    )?;
    let cli = catalog_summary(&cli_avail, &cli_sel)?;
    let parameterized = parse_parameter_models(
        param_resp.status,
        &param_resp.content_type,
        &param_resp.body,
    )?;

    if !cli.selector_safe || agent_catalog.default_model != cli.default_model {
        return Err(ProbeError::new("selection_unconfirmed"));
    }
    let Some(target) = cli.default_model.as_deref() else {
        return Err(ProbeError::new("selection_unconfirmed"));
    };

    let mut needs_mapping = false;
    let mut name_matches = 0usize;
    let mut supports_agent: Option<bool> = None;
    let mut requires_retention: Option<bool> = None;
    let mut param_defs = 0usize;
    let mut variants = 0usize;
    let mut variant_param_values = 0usize;
    let mut variant_max_mode = false;

    for entry in &parameterized {
        let exact = entry.name == target;
        let alias = entry.legacy_slugs.iter().any(|s| s == target);
        if alias && !exact {
            needs_mapping = true;
        }
        if exact {
            name_matches += 1;
            supports_agent = entry.supports_agent;
            requires_retention = entry.requires_data_retention;
        }
        if exact || alias {
            param_defs += entry.param_def_count;
            variants += entry.variant_count;
            variant_param_values += entry.variant_param_values;
            variant_max_mode |= entry.variant_max_mode;
        }
    }

    if needs_mapping
        || name_matches != 1
        || param_defs != 0
        || variants != 1
        || variant_param_values != 0
        || variant_max_mode
        || supports_agent == Some(false)
        || requires_retention == Some(true)
    {
        return Err(ProbeError::new("selection_unconfirmed"));
    }
    Ok(())
}

struct ParamModel {
    name: String,
    legacy_slugs: Vec<String>,
    supports_agent: Option<bool>,
    requires_data_retention: Option<bool>,
    param_def_count: usize,
    variant_count: usize,
    variant_param_values: usize,
    variant_max_mode: bool,
}

fn parse_parameter_models(
    status: u16,
    content_type: &str,
    body: &[u8],
) -> Result<Vec<ParamModel>, ProbeError> {
    if status != 200 {
        return Err(ProbeError::with_status("upstream_http_error", status));
    }
    if content_type != "application/proto" {
        return Err(ProbeError::new("unexpected_protobuf_content_type"));
    }
    let invalid = || ProbeError::new("invalid_selection_metadata");
    if body.len() > 512 * 1024 {
        return Err(invalid());
    }
    let mut models = Vec::new();
    for top in fields(body)? {
        if top.tag != 2 {
            continue;
        }
        if top.wire != 2 {
            return Err(invalid());
        }
        let mut seen_singletons = HashSet::new();
        let mut name = String::new();
        let mut legacy_slugs = Vec::new();
        let mut supports_agent = None;
        let mut requires_data_retention = None;
        let mut param_def_count = 0usize;
        let mut variant_count = 0usize;
        let mut variant_param_values = 0usize;
        let mut variant_max_mode = false;

        for f in fields(top.bytes)? {
            if matches!(f.tag, 1 | 5 | 46) && !seen_singletons.insert(f.tag) {
                return Err(invalid());
            }
            match f.tag {
                1 => {
                    if f.wire != 2 || f.bytes.len() > 4096 {
                        return Err(invalid());
                    }
                    name = std::str::from_utf8(f.bytes)
                        .map_err(|_| invalid())?
                        .to_owned();
                }
                5 => {
                    if f.wire != 0 || f.varint > 1 {
                        return Err(invalid());
                    }
                    supports_agent = Some(f.varint == 1);
                }
                46 => {
                    if f.wire != 0 || f.varint > 1 {
                        return Err(invalid());
                    }
                    requires_data_retention = Some(f.varint == 1);
                }
                36 => {
                    if f.wire != 2 || f.bytes.len() > 4096 {
                        return Err(invalid());
                    }
                    legacy_slugs.push(
                        std::str::from_utf8(f.bytes)
                            .map_err(|_| invalid())?
                            .to_owned(),
                    );
                }
                29 => {
                    if f.wire != 2 {
                        return Err(invalid());
                    }
                    let _ = fields(f.bytes)?;
                    param_def_count += 1;
                }
                30 => {
                    if f.wire != 2 {
                        return Err(invalid());
                    }
                    variant_count += 1;
                    let mut seen_max = false;
                    for vf in fields(f.bytes)? {
                        match vf.tag {
                            1 => {
                                if vf.wire != 2 {
                                    return Err(invalid());
                                }
                                let mut pv_seen = HashSet::new();
                                for pf in fields(vf.bytes)? {
                                    if matches!(pf.tag, 1 | 2) {
                                        if !pv_seen.insert(pf.tag)
                                            || pf.wire != 2
                                            || pf.bytes.len() > 4096
                                            || std::str::from_utf8(pf.bytes).is_err()
                                        {
                                            return Err(invalid());
                                        }
                                    }
                                }
                                variant_param_values += 1;
                            }
                            3 => {
                                if seen_max || vf.wire != 0 || vf.varint > 1 {
                                    return Err(invalid());
                                }
                                seen_max = true;
                                variant_max_mode |= vf.varint == 1;
                            }
                            _ => {}
                        }
                    }
                }
                _ => {}
            }
        }
        models.push(ParamModel {
            name,
            legacy_slugs,
            supports_agent,
            requires_data_retention,
            param_def_count,
            variant_count,
            variant_param_values,
            variant_max_mode,
        });
    }
    Ok(models)
}
