//! Explicit Cookie-only imports. No browser state, arbitrary headers, or executable/URL fields.
use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use super::{invalid, GeminiAuthFamily};
use crate::error::GatewayError;

#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Cookie {
    pub name: String,
    pub value: String,
    domain: String,
    path: String,
    expires: f64,
    secure: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    http_only: Option<bool>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    same_site: Option<String>,
}

impl Cookie {
    pub(super) fn matches_root(&self, host: &str) -> bool {
        self.path == "/"
            && (self.domain == host
                || self.domain.starts_with('.')
                    && (self.domain[1..] == *host || host.ends_with(&self.domain)))
    }
}

pub(super) fn validate(
    cookies: &mut Vec<Cookie>,
    family: GeminiAuthFamily,
) -> Result<(), GatewayError> {
    if cookies.is_empty() || cookies.len() > 64 {
        return Err(invalid("Cookie 数量必须为 1 至 64 条。"));
    }
    let aistudio = family == GeminiAuthFamily::AistudioWeb;
    let flow = family == GeminiAuthFamily::FlowWeb;
    let stitch = family == GeminiAuthFamily::StitchWeb;
    let mut keys = HashSet::new();
    for cookie in cookies.iter_mut() {
        cookie.domain = cookie.domain.to_ascii_lowercase();
        let pair_len = cookie.name.len() + 1 + cookie.value.len();
        let canvas_wire_len = pair_len
            + 7
            + cookie.path.len()
            + if cookie.domain.starts_with('.') {
                9 + cookie.domain.len()
            } else {
                0
            }
            + if cookie.secure { 8 } else { 0 };
        let domain = cookie.domain.strip_prefix('.').unwrap_or(&cookie.domain);
        let allowed = matches!(domain, "google.com" | "gemini.google.com")
            && !aistudio
            && !flow
            && !stitch
            || matches!(
                domain,
                "google.com" | "aistudio.google.com" | "alkalimakersuite-pa.clients6.google.com"
            ) && aistudio
            || matches!(
                cookie.domain.as_str(),
                ".google.com" | "flow.google.com" | ".flow.google.com"
            ) && flow
            || matches!(
                cookie.domain.as_str(),
                ".withgoogle.com" | "stitch.withgoogle.com" | ".stitch.withgoogle.com"
            ) && stitch;
        if !allowed
            || cookie.name.is_empty()
            || cookie.name.len() > 256
            || !cookie
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"!#$%&'*+-.^_`|~".contains(&b))
            || cookie.value.len() > 8192
            || family == GeminiAuthFamily::GeminiWeb && pair_len > 8192
            || family == GeminiAuthFamily::GeminiCanvas && canvas_wire_len > 8192
            || !cookie
                .value
                .bytes()
                .all(|b| (0x21..=0x7e).contains(&b) && !b";,\"\\".contains(&b))
            || cookie.path.len() > 2048
            || !cookie.path.starts_with('/')
            || cookie
                .path
                .bytes()
                .any(|b| b < 0x20 || b == 0x7f || b == b';')
            || !(cookie.expires == -1.0 || (0.0..=253402300799.0).contains(&cookie.expires))
            || cookie
                .same_site
                .as_deref()
                .is_some_and(|s| !matches!(s, "Strict" | "Lax" | "None"))
            || (cookie.name.starts_with("__Secure-") || cookie.name.starts_with("__Host-"))
                && !cookie.secure
            || cookie.name.starts_with("__Host-")
                && (cookie.domain.starts_with('.') || cookie.path != "/")
            || !keys.insert((
                cookie.domain.clone(),
                cookie.path.clone(),
                cookie.name.clone(),
            ))
        {
            return Err(invalid(
                "Cookie 格式、作用域或大小无效；只接受对应 Google 产品的必要材料。",
            ));
        }
        // Web's legacy Cookie-header owner cannot retain path scopes; never broaden them silently.
        if family == GeminiAuthFamily::GeminiWeb && cookie.path != "/" {
            return Err(invalid("Gemini Web JSON 只接受根路径 Cookie。"));
        }
    }
    let now = time::OffsetDateTime::now_utc().unix_timestamp() as f64;
    cookies.retain(|cookie| cookie.expires == -1.0 || cookie.expires > now);
    let host = if stitch {
        "stitch.withgoogle.com"
    } else if flow {
        "flow.google.com"
    } else if aistudio {
        "aistudio.google.com"
    } else {
        "gemini.google.com"
    };
    let has = |name: &str| {
        cookies.iter().any(|cookie| {
            cookie.name == name && !cookie.value.is_empty() && cookie.matches_root(host)
        })
    };
    let valid = match family {
        GeminiAuthFamily::GeminiWeb => has("__Secure-1PSID"),
        GeminiAuthFamily::FlowWeb => has("__Secure-1PSID"),
        GeminiAuthFamily::StitchWeb => cookies
            .iter()
            .any(|cookie| !cookie.value.is_empty() && cookie.matches_root(host)),
        GeminiAuthFamily::GeminiCanvas => ["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"]
            .iter()
            .any(|name| has(name)),
        GeminiAuthFamily::AistudioWeb => ["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"]
            .iter()
            .all(|name| has(name)),
        _ => false,
    };
    if !valid {
        return Err(invalid(
            "缺少有效的必要认证 Cookie，可能已过期或作用域不匹配。",
        ));
    }
    if serde_json::to_vec(cookies)
        .map_err(|_| invalid("Cookie 无效。"))?
        .len()
        > 32000
    {
        return Err(invalid("Cookie 材料不能超过 32 KiB。"));
    }
    Ok(())
}

pub(super) fn web_header(cookies: &[Cookie]) -> Result<String, GatewayError> {
    let mut names = HashSet::new();
    let mut values = Vec::new();
    for cookie in cookies
        .iter()
        .filter(|cookie| cookie.matches_root("gemini.google.com"))
    {
        if !names.insert(&cookie.name) {
            return Err(invalid("Gemini Web Cookie 名称重复，无法确定正确作用域。"));
        }
        values.push(format!("{}={}", cookie.name, cookie.value));
    }
    Ok(values.join("; "))
}
