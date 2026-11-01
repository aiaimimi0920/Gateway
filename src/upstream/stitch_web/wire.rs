//! Stitch Nemo RPC framing. Product operations are not Gemini/Flow protocol aliases.
use super::failure;
use crate::error::GatewayError;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::LazyLock;

pub(super) const LIST: &str = "A7f2qf";
pub(super) const CREATE: &str = "cabgj";
pub(super) const GENERATE: &str = "IEkn6e";
pub(super) const POLL: &str = "uYEY6";
pub(super) const SCREENS: &str = "ErneX";

pub(super) struct Bootstrap {
    pub at: String,
    pub sid: String,
    pub build: String,
}

pub(super) fn bootstrap(html: &str) -> Result<Bootstrap, GatewayError> {
    static FIELDS: LazyLock<[Regex; 3]> = LazyLock::new(|| {
        ["SNlM0e", "FdrFJe", "cfb2h"].map(|key| {
            Regex::new(&format!(r#""{key}"\s*:\s*("(?:[^"\\]|\\.){{1,4096}}")"#)).unwrap()
        })
    });
    let read = |index: usize| {
        FIELDS[index]
            .captures(html)
            .and_then(|m| serde_json::from_str::<String>(&m[1]).ok())
            .filter(|v| !v.is_empty())
            .ok_or_else(|| {
                failure(
                    "stitch_web_bootstrap_invalid",
                    "Stitch 页面缺少登录或运行参数。",
                )
            })
    };
    Ok(Bootstrap {
        at: read(0)?,
        sid: read(1)?,
        build: read(2)?,
    })
}

pub(super) fn form(rpc: &str, params: &Value, at: &str) -> String {
    let packed = json!([[[rpc, params.to_string(), null, "generic"]]]).to_string();
    url::form_urlencoded::Serializer::new(String::new())
        .append_pair("f.req", &packed)
        .append_pair("at", at)
        .finish()
}

pub(super) fn decode(text: &str, rpc: &str) -> Result<Value, GatewayError> {
    let invalid = || {
        failure(
            "stitch_web_rpc_invalid",
            "Stitch RPC 响应无效，未将 HTTP 200 当作生成成功。",
        )
    };
    let mut result = None;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('['))
    {
        let frame: Value = serde_json::from_str(line).map_err(|_| invalid())?;
        for row in frame.as_array().ok_or_else(invalid)? {
            if row.get(0).and_then(Value::as_str) == Some("er") {
                return Err(failure(
                    "stitch_web_rpc_rejected",
                    "Stitch 拒绝了请求；不会自动重放生成。",
                ));
            }
            if row.get(0).and_then(Value::as_str) != Some("wrb.fr")
                || row.get(1).and_then(Value::as_str) != Some(rpc)
            {
                continue;
            }
            if result.is_some() || row.get(5).is_some_and(|v| !v.is_null()) {
                return Err(invalid());
            }
            result = Some(
                serde_json::from_str::<Value>(
                    row.get(2).and_then(Value::as_str).ok_or_else(invalid)?,
                )
                .map_err(|_| invalid())?,
            );
        }
    }
    let result = result.filter(|v| !v.is_null()).ok_or_else(invalid)?;
    let mut pending = vec![&result];
    let mut visited = 0;
    while let Some(value) = pending.pop() {
        visited += 1;
        if visited > 8192 {
            return Err(invalid());
        }
        match value {
            Value::String(s) if s.starts_with("PUBLIC_ERROR_") || s.starts_with("ERROR_") => {
                return Err(failure(
                    "stitch_web_rpc_rejected",
                    "Stitch 返回了产品错误；未记作成功。",
                ))
            }
            Value::Array(a) => pending.extend(a),
            Value::Object(o) => pending.extend(o.values()),
            _ => {}
        }
    }
    Ok(result)
}

pub(super) fn read_only_refresh(text: &str, rpc: &str, previous: &str) -> Option<String> {
    if rpc != LIST {
        return None;
    }
    for line in text.lines().map(str::trim).filter(|s| s.starts_with('[')) {
        let root: Value = serde_json::from_str(line).ok()?;
        let mut pending = vec![&root];
        let mut visited = 0;
        while let Some(value) = pending.pop() {
            visited += 1;
            if visited > 8192 {
                return None;
            }
            if let Some(a) = value.as_array() {
                if a.first().and_then(Value::as_str) == Some("xsrf") {
                    return a
                        .get(1)
                        .and_then(Value::as_str)
                        .filter(|s| !s.is_empty() && s.len() <= 4096 && *s != previous)
                        .map(str::to_owned);
                }
                pending.extend(a);
            }
        }
    }
    None
}

pub(super) fn project(value: &Value) -> Result<String, GatewayError> {
    resource(value.get(0).and_then(Value::as_str), false)
}

pub(super) fn session(value: &Value) -> Result<String, GatewayError> {
    resource(value.get(0).and_then(Value::as_str), true)
}

fn resource(value: Option<&str>, session: bool) -> Result<String, GatewayError> {
    let value = value.unwrap_or_default();
    let parts: Vec<_> = value.split('/').collect();
    let valid_id = |s: &str| {
        !s.is_empty()
            && s.len() <= 128
            && s.bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    };
    if (parts.len() == if session { 4 } else { 2 })
        && parts[0] == "projects"
        && valid_id(parts[1])
        && (!session || parts[2] == "sessions" && valid_id(parts[3]))
    {
        return Ok(value.into());
    }
    Err(failure(
        "stitch_web_resource_invalid",
        "Stitch 未返回有效的项目或生成会话标识。",
    ))
}

pub(super) fn generate_params(project: &str, prompt: &str) -> Value {
    json!([
        project,
        [
            null,
            null,
            null,
            [prompt, null, 1, null, [], null, null, 1, [], 2],
            null,
            null,
            1
        ]
    ])
}
