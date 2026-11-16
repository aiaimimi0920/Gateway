//! Public diagnostics contain only local protocol/status labels, never upstream bodies or secrets.
use super::ProtocolProbe;
use std::{collections::BTreeSet, fmt};

#[derive(Debug)]
pub enum DiscoveryFailure {
    Catalogue,
    Protocols(Vec<ProtocolProbe>),
}
impl fmt::Display for DiscoveryFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Catalogue => write!(f, "无法读取模型目录。请检查目录权限，或在账号中填写已知可用的文本模型后再刷新协议；目录不可用不代表生成协议不可用。"),
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
                Ok(())
            }
        }
    }
}
impl std::error::Error for DiscoveryFailure {}
