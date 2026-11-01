//! UI artifacts are returned as a JSON manifest, never executed or mislabeled as chat/images.
use super::{failure, ORIGIN};
use crate::error::GatewayError;
use serde_json::{json, Value};

fn dimension(value: Option<&Value>) -> Option<u64> {
    let value = value?;
    value
        .as_u64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| (1..=16384).contains(n))
}

fn asset(value: Option<&Value>, html: bool) -> Option<String> {
    let raw = value?.as_str()?;
    if raw.len() > 16384 {
        return None;
    }
    let url = url::Url::parse(raw).ok()?;
    if url.scheme() != "https"
        || !url.username().is_empty()
        || url.password().is_some()
        || url.port().is_some()
        || !(url.host_str() == Some("contribution.usercontent.google.com")
            && url.path() == "/download"
            || !html
                && matches!(
                    url.host_str(),
                    Some("lh3.googleusercontent.com" | "lh3.google.com")
                ))
    {
        return None;
    }
    Some(raw.into())
}

pub(super) fn manifest(project: &str, raw: &Value) -> Result<String, GatewayError> {
    let mut pending = vec![raw];
    let mut screens = Vec::new();
    let mut visited = 0;
    while let Some(value) = pending.pop() {
        visited += 1;
        if visited > 8192 {
            return Err(failure(
                "stitch_web_result_limit",
                "Stitch 产物结构超过限制。",
            ));
        }
        let Some(items) = value.as_array() else {
            continue;
        };
        let candidate = items
            .get(4)
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty() && s.len() <= 128);
        if let Some(id) = candidate {
            let html = items.get(1);
            if html.and_then(|v| v.get(5)).and_then(Value::as_str) == Some("text/html") {
                if let Some(html_url) = asset(html.and_then(|v| v.get(2)), true) {
                    screens.push(json!({"id":id,"name":items.get(8).and_then(Value::as_str).unwrap_or(""),
                        "html_url":html_url,"thumbnail_url":asset(items.first().and_then(|v|v.get(2)),false),
                        "width":dimension(items.get(6)),"height":dimension(items.get(7))}));
                    if screens.len() > 16 {
                        return Err(failure(
                            "stitch_web_result_limit",
                            "Stitch 生成界面数量超过限制。",
                        ));
                    }
                    continue;
                }
            }
        }
        pending.extend(items);
    }
    if screens.is_empty() {
        return Err(failure(
            "stitch_web_artifacts_missing",
            "Stitch 未返回可验证的 HTML 界面产物，未记作成功。",
        ));
    }
    serde_json::to_string(&json!({"provider":"stitch-web","project":project,"project_url":format!("{ORIGIN}/{project}"),"screens":screens}))
        .map_err(|_|failure("stitch_web_result_invalid","Stitch 产物清单序列化失败。"))
}
