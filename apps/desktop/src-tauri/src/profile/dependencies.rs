use super::{GatewayDependencyCheckItem, GatewayProfile};
use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;
fn parse_dependency_endpoint(
    configured_url: &str,
    accepted_schemes: &[&str],
    default_port: u16,
) -> Result<(String, u16), String> {
    let parsed = reqwest::Url::parse(configured_url)
        .map_err(|_| "configured URL is not valid".to_string())?;
    if !accepted_schemes.contains(&parsed.scheme()) {
        return Err(format!(
            "configured URL must use one of: {}",
            accepted_schemes.join(", ")
        ));
    }
    let host = parsed
        .host_str()
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "configured URL is missing a host".to_string())?;
    Ok((host.to_string(), parsed.port().unwrap_or(default_port)))
}

fn tcp_dependency_check(
    name: &str,
    configured_url: &str,
    accepted_schemes: &[&str],
    default_port: u16,
    required: bool,
    remediation: &str,
) -> GatewayDependencyCheckItem {
    let endpoint = parse_dependency_endpoint(configured_url, accepted_schemes, default_port);
    let (host, port) = match endpoint {
        Ok(endpoint) => endpoint,
        Err(error) => {
            return GatewayDependencyCheckItem {
                name: name.to_string(),
                required,
                configured: true,
                ok: false,
                message: format!("{name} configuration is invalid: {error}. {remediation}"),
            };
        }
    };

    let addresses = (host.as_str(), port).to_socket_addrs();
    let reachable = addresses
        .ok()
        .and_then(|mut values| {
            values
                .any(|address| {
                    TcpStream::connect_timeout(&address, Duration::from_millis(500)).is_ok()
                })
                .then_some(())
        })
        .is_some();

    GatewayDependencyCheckItem {
        name: name.to_string(),
        required,
        configured: true,
        ok: reachable,
        message: if reachable {
            format!("{name} accepted a TCP connection")
        } else {
            format!("{name} is not reachable. {remediation}")
        },
    }
}

pub(super) fn optional_database_check(profile: &GatewayProfile) -> GatewayDependencyCheckItem {
    if profile.local_storage() {
        return local_dependency("PostgreSQL");
    }
    let Some(database_url) = profile
        .gateway_database_url
        .as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
    else {
        return GatewayDependencyCheckItem {
            name: "PostgreSQL".to_string(),
            required: false,
            configured: false,
            ok: true,
            message:
                "PostgreSQL is not configured; database-backed Gateway features remain disabled"
                    .to_string(),
        };
    };

    tcp_dependency_check(
        "PostgreSQL",
        database_url,
        &["postgres", "postgresql"],
        5432,
        false,
        "Start PostgreSQL or update GATEWAY_DATABASE_URL.",
    )
}

pub(super) fn redis_check(profile: &GatewayProfile) -> GatewayDependencyCheckItem {
    if profile.local_storage() {
        return local_dependency("Redis");
    }
    tcp_dependency_check(
        "Redis",
        profile.gateway_redis_url.trim(),
        &["redis", "rediss"],
        6379,
        true,
        "Start Redis or update GATEWAY_REDIS_URL.",
    )
}

fn local_dependency(name: &str) -> GatewayDependencyCheckItem {
    GatewayDependencyCheckItem {
        name: name.into(),
        required: false,
        configured: false,
        ok: true,
        message: "Local storage uses SQLite and in-process runtime state".into(),
    }
}
