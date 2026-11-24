//! Public diagnostics contain only local protocol/status labels, never upstream bodies or secrets.
use super::ProtocolProbe;
use std::{collections::BTreeSet, fmt};

#[derive(Debug)]
pub enum DiscoveryFailure {
    Catalogue,
    CatalogueHttp {
        status: u16,
        site_blocked: bool,
        client_restricted: bool,
    },
    Protocols(Vec<ProtocolProbe>),
}
impl fmt::Display for DiscoveryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalogue => write!(f, "无法读取模型目录。请检查目录权限，或在账号中填写已知可用的文本模型后再刷新协议；目录不可用不代表生成协议不可用。"),
            Self::CatalogueHttp { status, site_blocked, client_restricted } => {
                write!(f, "模型目录请求被拒绝（HTTP {status}）。")?;
                if *client_restricted {
                    write!(f, "上游拒绝了本次探测的客户端身份；请核对服务商的客户端限制。此结果不代表所有实际调用均不可用；原配置未更改。")
                } else if *site_blocked {
                    write!(f, "站点访问策略拒绝了当前客户端（Cloudflare 1010），请联系服务商检查访问策略；原配置未更改。")
                } else {
                    write!(f, "请检查该账号的目录权限及服务商访问限制；原配置未更改。")
                }
            }
            Self::Protocols(probes) => {
                write!(f, "尚未确认可用协议（超时不代表不支持）：")?;
                for (index, probe) in probes.iter().enumerate() {
                    if index > 0 { write!(f, "; ")?; }
                    let outcomes = probe.attempts.iter().map(|a| match a.http_status {
                        Some(code) => format!("HTTP {code} {}", a.status),
                        None => a.status.clone(),
                    }).collect::<BTreeSet<_>>();
                    write!(f, "{}: {}", probe.protocol.family(), outcomes.into_iter().collect::<Vec<_>>().join(", "))?;
                }
                if probes.iter().flat_map(|p| &p.attempts).any(|a| a.status == "client_restricted") {
                    write!(f, "。上游拒绝了本次探测的客户端身份；请核对服务商的客户端限制。此结果不代表所有实际调用均不可用；原配置未更改。")?;
                } else if probes.iter().flat_map(|p| &p.attempts).any(|a| a.http_status == Some(403)) {
                    write!(f, "。HTTP 403 表示上游拒绝访问，请检查服务商的账号、模型权限和客户端接入限制；不等于该协议不存在。")?;
                }
                Ok(())
            }
        }
    }
}
impl std::error::Error for DiscoveryFailure {}

/// Inspect a small error envelope only for known denial codes. Never expose upstream text.
pub(super) async fn rejected(response: rquest::Response) -> DiscoveryFailure {
    use futures::StreamExt;
    let status = response.status().as_u16();
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(Ok(chunk)) = stream.next().await {
        if bytes.len() + chunk.len() > 8192 {
            break;
        }
        bytes.extend_from_slice(&chunk);
    }
    let site_blocked = status == 403
        && serde_json::from_slice::<serde_json::Value>(&bytes)
            .ok()
            .is_some_and(|v| {
                v.get("cloudflare_error").and_then(|v| v.as_bool()) == Some(true)
                    && v.get("error_code").and_then(|v| v.as_u64()) == Some(1010)
            });
    // Some relays append SSE after a JSON error. Match only the known restriction;
    // arbitrary upstream text, model IDs and secrets never enter public diagnostics.
    let client_restricted = status == 403
        && String::from_utf8_lossy(&bytes)
            .contains("This account only allows Codex official clients");
    DiscoveryFailure::CatalogueHttp {
        status,
        site_blocked,
        client_restricted,
    }
}
