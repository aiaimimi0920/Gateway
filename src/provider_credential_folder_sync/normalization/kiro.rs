//! Pure Kiro folder conversion; complete registrations are credential-owned, never inferred.
use super::super::payload_metadata::apply_folder_sync_metadata;
use super::kiro_fields::{invalid, string, Fields};
use super::kiro_payload;
use crate::{
    db,
    error::GatewayError,
    protocol::kiro_oauth::{business, AuthorizationContext},
};
use serde_json::{json, Map, Value};

pub(super) fn normalize_kiro_import_payload(
    account: &db::GatewayProviderAccountView,
    raw: Value,
    kind: Option<&str>,
) -> Result<Value, GatewayError> {
    let mut fields = Fields::new(raw)?;
    let old_refresh = fields.legacy_refresh()?;
    let access = match fields.read(&["apikey", "accesstoken", "kiroaccesstoken"])? {
        None => String::new(),
        Some(Value::String(access)) if access.is_empty() || business::valid_token(&access) => {
            access
        }
        _ => return Err(invalid()),
    };
    let refresh = string(fields.read(&["refreshtoken", "kirorefreshtoken"])?)?;
    if refresh
        .as_deref()
        .is_some_and(|token| !business::valid_token(token))
    {
        return Err(invalid());
    }
    let machine = fields.machine()?;
    let context = context(&mut fields, machine.as_deref())?;
    // Only extra_body RT was consumed by the preexisting headers owner. A root RT alone
    // must not silently create a new device; preserve its old zero-ID behavior instead.
    let explicit = context
        .as_ref()
        .and_then(|context| {
            context
                .get("machine_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
        })
        .or(machine);
    let machine = match explicit {
        Some(machine) => Some(machine),
        None => {
            // Only inherit non-secret explicit device metadata, never registration or RT.
            // Account JSON is already runtime configuration, not an import source. Only
            // its two actual serde spellings may contribute previously effective identity.
            let extra = account
                .payload
                .get("extra_body")
                .or_else(|| account.payload.get("extraBody"));
            let mut inherited = Fields::new(json!({"extra_body":extra}))?;
            inherited.machine()?
        }
    }
    .or_else(|| crate::protocol::kiro::generate_machine_id(None, old_refresh.as_deref()))
    .unwrap_or_else(|| "0".repeat(64));
    let context = context.map(|mut context| {
        context.insert("machine_id".into(), json!(machine));
        context
    });
    let context = context
        .map(|context| {
            serde_json::from_value::<AuthorizationContext>(Value::Object(context))
                .map_err(|_| invalid())
        })
        .transpose()?;
    if let Some(context) = &context {
        context.validate_metadata().map_err(|_| invalid())?;
        if context.client_secret().is_some() {
            context.validate().map_err(|_| invalid())?;
        }
    }
    if access.is_empty()
        && (refresh.is_none()
            || context
                .as_ref()
                .is_none_or(|context| context.client_secret().is_none()))
    {
        return Err(invalid());
    }
    let endpoint = string(fields.read(&["refreshendpoint"])?)?;
    let client = string(fields.read(&["refreshclientid"])?)?;
    if endpoint.as_ref().is_some_and(|endpoint| {
        context
            .as_ref()
            .is_none_or(|c| endpoint != &c.token_endpoint())
    }) || client
        .as_ref()
        .is_some_and(|client| context.as_ref().is_none_or(|c| client != c.client_id()))
    {
        return Err(invalid());
    }
    let expiry = string(fields.read(&["expiresat"])?)?;
    let mut payload = kiro_payload::project(account, fields, &access, &machine, expiry, kind)?;
    if let Some(refresh) = refresh {
        payload.insert("refresh_token".into(), json!(refresh));
    }
    if let Some(context) = context {
        payload.insert("kiro_oauth".into(), json!(context));
    }
    apply_folder_sync_metadata(&mut payload, account, Some("bearer_token"));
    Ok(Value::Object(payload))
}

fn context(
    fields: &mut Fields,
    machine: Option<&str>,
) -> Result<Option<Map<String, Value>>, GatewayError> {
    let typed = fields.root(&["kirooauth"])?;
    let had_typed = typed.is_some();
    let mut result = match typed {
        None => Map::new(),
        Some(value) => {
            let context: AuthorizationContext =
                serde_json::from_value(value).map_err(|_| invalid())?;
            context.validate_metadata().map_err(|_| invalid())?;
            if context.client_secret().is_some() {
                context.validate().map_err(|_| invalid())?;
            }
            serde_json::to_value(context)
                .map_err(|_| invalid())?
                .as_object()
                .unwrap()
                .clone()
        }
    };
    for (name, aliases) in [
        ("auth_method", &["authmethod", "kiroauthmethod"][..]),
        (
            "directory_region",
            &["directoryregion", "authregion", "kiroauthregion"][..],
        ),
        ("start_url", &["starturl", "kirostarturl"][..]),
        ("client_id", &["clientid", "kiroclientid"][..]),
        ("client_secret", &["clientsecret", "kiroclientsecret"][..]),
        (
            "client_secret_expires_at",
            &["clientsecretexpiresat", "kiroclientsecretexpiresat"][..],
        ),
        ("scopes", &["scopes", "kiroscopes"][..]),
    ] {
        if let Some(value) = fields.read(aliases)? {
            if result.get(name).is_some_and(|old| old != &value) {
                return Err(invalid());
            }
            result.insert(name.into(), value);
        }
    }
    if result.is_empty() {
        return Ok(None);
    }
    if !had_typed
        && result
            .get("client_secret")
            .and_then(Value::as_str)
            .is_none()
    {
        return Err(invalid());
    }
    if let Some(machine) = machine {
        if result
            .get("machine_id")
            .is_some_and(|old| old.as_str() != Some(machine))
        {
            return Err(invalid());
        }
        result.insert("machine_id".into(), json!(machine));
    }
    // The caller inserts the preserved legacy device when no explicit value was supplied.
    Ok(Some(result))
}
