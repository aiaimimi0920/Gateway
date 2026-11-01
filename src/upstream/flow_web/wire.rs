//! Current flow.google.com batchexecute control RPC, not the legacy Labs bearer API.
use super::failure;
use crate::error::GatewayError;
use regex::Regex;
use serde_json::{json, Value};
use std::sync::LazyLock;

pub(super) const CREDITS_RPC: &str = "nzlxg";
pub(super) struct Bootstrap {
    pub build: String,
    pub sid: String,
    pub at: Option<String>,
}

pub(super) fn bootstrap(html: &str) -> Result<Bootstrap, GatewayError> {
    static BUILD: LazyLock<Regex> = LazyLock::new(|| {
        Regex::new(r"boq_labs-ai-sandbox-frontend_[A-Za-z0-9_.-]{1,256}").unwrap()
    });
    static SID: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""FdrFJe"\s*:\s*"(-?[0-9]{1,32})""#).unwrap());
    static AT: LazyLock<Regex> =
        LazyLock::new(|| Regex::new(r#""SNlM0e"\s*:\s*("(?:[^"\\]|\\.){1,4096}")"#).unwrap());
    let invalid = || {
        failure(
            "flow_web_bootstrap_invalid",
            "Flow HTTP 页面缺少必要运行参数，请重新登录或检查网页协议变化。",
        )
    };
    Ok(Bootstrap {
        build: BUILD.find(html).ok_or_else(invalid)?.as_str().into(),
        sid: SID.captures(html).ok_or_else(invalid)?[1].into(),
        at: AT
            .captures(html)
            .and_then(|m| serde_json::from_str::<String>(&m[1]).ok()),
    })
}

pub(super) fn credits_form(at: Option<&str>) -> String {
    let packed = json!([[[CREDITS_RPC, "[]", null, "generic"]]]).to_string();
    let mut form = url::form_urlencoded::Serializer::new(String::new());
    form.append_pair("f.req", &packed);
    if let Some(at) = at {
        form.append_pair("at", at);
    }
    form.finish()
}

pub(super) fn credits(text: &str) -> Result<u64, GatewayError> {
    let invalid = || {
        failure(
            "flow_web_rpc_invalid",
            "Flow HTTP 额度响应格式无效；未将缺失值当作零额度。",
        )
    };
    if text.len() > 1024 * 1024 {
        return Err(invalid());
    }
    let mut payload = None;
    for line in text
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('['))
    {
        let frame: Value = serde_json::from_str(line).map_err(|_| invalid())?;
        let rows = frame.as_array().ok_or_else(invalid)?;
        for row in rows {
            if row.get(0).and_then(Value::as_str) == Some("er") {
                return Err(failure(
                    "flow_web_rpc_rejected",
                    "Flow HTTP 只读 RPC 被拒绝，请检查登录状态。",
                ));
            }
            if row.get(0).and_then(Value::as_str) != Some("wrb.fr")
                || row.get(1).and_then(Value::as_str) != Some(CREDITS_RPC)
            {
                continue;
            }
            if payload.is_some() || row.get(5).is_some_and(|v| !v.is_null()) {
                return Err(invalid());
            }
            payload = Some(
                serde_json::from_str::<Value>(
                    row.get(2).and_then(Value::as_str).ok_or_else(invalid)?,
                )
                .map_err(|_| invalid())?,
            );
        }
    }
    let payload = payload.ok_or_else(invalid)?;
    let mut pending = vec![&payload];
    let mut visited = 0;
    while let Some(value) = pending.pop() {
        visited += 1;
        if visited > 4096 {
            return Err(invalid());
        }
        match value {
            Value::String(s) if s.starts_with("PUBLIC_ERROR_") || s.starts_with("ERROR_") => {
                return Err(invalid())
            }
            Value::Array(items) => pending.extend(items),
            Value::Object(items) => pending.extend(items.values()),
            _ => {}
        }
    }
    payload
        .as_array()
        .and_then(|items| items.first())
        .and_then(Value::as_u64)
        .ok_or_else(invalid)
}
