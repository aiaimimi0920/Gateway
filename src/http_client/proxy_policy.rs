//! 保留 rquest 5 的代理来源、CGI 与明确空 bypass 语义，不修改环境或注册表。
use rquest::{ClientBuilder, NoProxy, Proxy};
use std::sync::LazyLock;

#[derive(Default)]
struct Addresses {
    http: Option<String>,
    https: Option<String>,
}

impl Addresses {
    fn is_empty(&self) -> bool {
        self.http.is_none() && self.https.is_none()
    }

    #[cfg(any(windows, test))]
    fn set(&mut self, protocol: &str, address: &str) {
        let target = match protocol {
            "http" => &mut self.http,
            "https" => &mut self.https,
            _ => return,
        };
        if let Some(address) = normalize_address(address) {
            *target = Some(address);
        }
    }
}

fn normalize_address(value: &str) -> Option<String> {
    if value.trim().is_empty() {
        return None;
    }
    let url = match url::Url::parse(value) {
        Ok(url) if url.has_host() => url,
        Ok(_) | Err(url::ParseError::RelativeUrlWithoutBase) => {
            url::Url::parse(&format!("http://{value}")).ok()?
        }
        Err(_) => return None,
    };
    // 未启用 SOCKS；不可把客户端接受但 matcher 会丢弃的 scheme 算作有效代理。
    if !url.has_host() || !matches!(url.scheme(), "http" | "https") {
        return None;
    }
    let value = url.to_string();
    Proxy::all(value.clone()).ok()?;
    Some(value)
}

fn environment_addresses(read: impl Fn(&str) -> Option<String>, cgi: bool) -> Addresses {
    let valid = |upper, lower| {
        read(upper)
            .and_then(|value| normalize_address(&value))
            .or_else(|| read(lower).and_then(|value| normalize_address(&value)))
    };
    let all = valid("ALL_PROXY", "all_proxy");
    Addresses {
        http: if cgi {
            None
        } else {
            valid("HTTP_PROXY", "http_proxy")
        }
        .or_else(|| all.clone()),
        https: valid("HTTPS_PROXY", "https_proxy").or(all),
    }
}

#[cfg(any(windows, test))]
fn scheme_prefix(address: &str) -> Option<&str> {
    let (prefix, _) = address.split_once("://")?;
    (!prefix.is_empty() && !prefix.contains([':', '/'])).then_some(prefix)
}

#[cfg(any(windows, test))]
fn system_addresses(server: &str) -> Addresses {
    let mut result = Addresses::default();
    if server.contains('=') {
        for entry in server.split(';') {
            let parts: Vec<_> = entry.split('=').collect();
            let [protocol, address] = parts.as_slice() else {
                return Addresses::default();
            };
            if scheme_prefix(address).is_some() {
                result.set(protocol, address);
            } else {
                result.set(protocol, &format!("http://{address}"));
            }
        }
    } else if let Some(protocol) = scheme_prefix(server) {
        result.set(protocol, server);
    } else {
        result.set("http", &format!("http://{server}"));
        result.set("https", &format!("http://{server}"));
    }
    result
}

#[cfg(any(windows, test))]
fn resolve_addresses(environment: Addresses, enabled: bool, server: Option<&str>) -> Addresses {
    if environment.is_empty() && enabled {
        server.map(system_addresses).unwrap_or_default()
    } else {
        environment
    }
}

fn resolve_bypass(environment: Option<String>, system: Option<String>) -> Option<NoProxy> {
    let raw = environment.or_else(|| {
        system.map(|value| {
            value
                .split(';')
                .map(str::trim)
                .collect::<Vec<_>>()
                .join(",")
                .replace("*.", "")
        })
    });
    raw.map(|value| NoProxy::from_string(&value).unwrap_or_default())
}

#[cfg(windows)]
fn internet_settings() -> Option<windows_registry::Key> {
    windows_registry::CURRENT_USER
        .open("Software\\Microsoft\\Windows\\CurrentVersion\\Internet Settings")
        .ok()
}

#[cfg(windows)]
fn platform_addresses(environment: Addresses) -> Addresses {
    let settings = internet_settings();
    let enabled = settings
        .as_ref()
        .and_then(|key| key.get_u32("ProxyEnable").ok())
        == Some(1);
    let server = settings
        .as_ref()
        .and_then(|key| key.get_string("ProxyServer").ok());
    resolve_addresses(environment, enabled, server.as_deref())
}

#[cfg(not(windows))]
fn platform_addresses(environment: Addresses) -> Addresses {
    environment
}

fn platform_bypass() -> Option<String> {
    #[cfg(windows)]
    return internet_settings().and_then(|key| key.get_string("ProxyOverride").ok());
    #[cfg(not(windows))]
    None
}

fn apply(
    mut builder: ClientBuilder,
    addresses: &Addresses,
    bypass: Option<NoProxy>,
) -> ClientBuilder {
    builder = builder.no_proxy();
    for (address, constructor) in [
        (
            &addresses.http,
            Proxy::http as fn(String) -> rquest::Result<Proxy>,
        ),
        (
            &addresses.https,
            Proxy::https as fn(String) -> rquest::Result<Proxy>,
        ),
    ] {
        if let Some(address) = address {
            // 地址已通过同一公开 constructor 验证；错误不携带代理 URL/凭证进入日志。
            if let Ok(proxy) = constructor(address.clone()) {
                builder = builder.proxy(proxy.no_proxy(bypass.clone()));
            }
        }
    }
    builder
}

pub(super) fn configure(builder: ClientBuilder) -> ClientBuilder {
    // 与旧客户端相同：代理地址在首次构造时冻结，bypass 每次构造读取。
    static ADDRESSES: LazyLock<Addresses> = LazyLock::new(|| {
        platform_addresses(environment_addresses(
            |key| std::env::var(key).ok(),
            std::env::var_os("REQUEST_METHOD").is_some(),
        ))
    });
    // macOS 无环境代理时继续委托平台实现；本轮仅验收 Windows/Linux 代理来源合同。
    #[cfg(target_os = "macos")]
    if ADDRESSES.is_empty() {
        return builder;
    }
    let environment = std::env::var("NO_PROXY")
        .or_else(|_| std::env::var("no_proxy"))
        .ok();
    let system = environment.is_none().then(platform_bypass).flatten();
    apply(builder, &ADDRESSES, resolve_bypass(environment, system))
}

#[cfg(test)]
#[path = "proxy_policy_tests.rs"]
mod tests;
