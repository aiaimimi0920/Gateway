//! Process-scoped trust for the packaged SenseNova test, never for regular clients.
use anyhow::{bail, ensure, Context};
use std::{fs::File, io::Read, net::SocketAddrV4, path::Path, time::Duration};

const PROXY: &str = "GATEWAY_DEVTEST_SENSENOVA_PROXY";
const ROOT: &str = "GATEWAY_DEVTEST_SENSENOVA_CA";
const MAX_CA: u64 = 16 * 1024;

pub(crate) fn load(config: &crate::config::Config) -> anyhow::Result<Option<rquest::Client>> {
    load_isolated(config, "SenseNova", "packaged-sensenova-", PROXY, ROOT)
}

pub(super) fn load_isolated(
    config: &crate::config::Config,
    name: &str,
    namespace: &str,
    proxy_var: &str,
    ca_var: &str,
) -> anyhow::Result<Option<rquest::Client>> {
    Ok(
        load_isolated_with_root(config, name, namespace, proxy_var, ca_var)?
            .map(|transport| transport.http),
    )
}

pub(crate) struct IsolatedTransport {
    pub(crate) http: rquest::Client,
    pub(crate) root_pem: Vec<u8>,
}

pub(crate) fn load_isolated_with_root(
    config: &crate::config::Config,
    name: &str,
    namespace: &str,
    proxy_var: &str,
    ca_var: &str,
) -> anyhow::Result<Option<IsolatedTransport>> {
    let proxy = std::env::var_os(proxy_var);
    let root = std::env::var_os(ca_var);
    if proxy.is_none() && root.is_none() {
        return Ok(None);
    }
    ensure!(
        gateway_local_data::devtest::isolated_build(),
        "{name} fixture requires an isolated devtest binary"
    );
    let (Some(proxy), Some(root)) = (proxy, root) else {
        bail!("{name} fixture requires both proxy and CA settings");
    };
    ensure!(
        std::env::var("GATEWAY_BIND_HOST").as_deref() == Ok("127.0.0.1")
            && config.port != 0 && config.port != gateway_local_data::devtest::default_api_port()
            && std::env::var_os("GATEWAY_DATA_DIR").is_some_and(|dir| Path::new(&dir).is_absolute())
            && config.storage_mode == crate::config::GatewayStorageMode::Server
            && config.runtime_role == crate::config::GatewayRuntimeRole::Standalone
            && config.console.redis_namespace.starts_with(namespace)
            && config.database_url.as_deref().is_some_and(|url| fixture_service(url, "postgresql", 5432, "/gateway_fixture"))
            && fixture_service(&config.redis_url, "redis", 6379, "/"),
        "{name} fixture requires loopback listeners, explicit data isolation and the named Server fixture stores"
    );
    let proxy = proxy
        .to_str()
        .and_then(loopback_proxy)
        .with_context(|| format!("Invalid {name} fixture proxy"))?;
    let path = Path::new(&root);
    ensure!(
        path.is_absolute() && path.is_file(),
        "{name} fixture CA must be an absolute file path"
    );
    let mut ca = Vec::new();
    File::open(path)?.take(MAX_CA + 1).read_to_end(&mut ca)?;
    ensure!(
        !ca.is_empty() && ca.len() as u64 <= MAX_CA,
        "Invalid {name} fixture CA size"
    );
    let roots = rquest::tls::trust::CertStore::from_pem_stack(&ca)?;
    let client = rquest::Client::builder()
        .emulation(rquest_util::Emulation::Chrome136)
        .tls_cert_store(roots)
        .no_proxy()
        .proxy(rquest::Proxy::all(format!("http://{proxy}"))?)
        .redirect(rquest::redirect::Policy::none())
        .retry(rquest::retry::Policy::never())
        .pool_max_idle_per_host(0)
        .timeout(Duration::from_secs(config.upstream_timeout_secs))
        .build()?;
    Ok(Some(IsolatedTransport {
        http: client,
        root_pem: ca,
    }))
}

pub(crate) fn loopback_proxy(input: &str) -> Option<SocketAddrV4> {
    let socket: SocketAddrV4 = input.parse().ok()?;
    (*socket.ip() == std::net::Ipv4Addr::LOCALHOST && socket.port() > 0).then_some(socket)
}

fn fixture_service(input: &str, scheme: &str, default_port: u16, path: &str) -> bool {
    url::Url::parse(input).is_ok_and(|url| {
        url.scheme() == scheme
            && url.host_str() == Some("127.0.0.1")
            && url.path() == path
            && url.query().is_none()
            && url.fragment().is_none()
            && url
                .port()
                .is_some_and(|port| port > 0 && port != default_port)
    })
}

pub(crate) fn synthetic_key(key: &str) -> bool {
    namespaced_key(key, "cpk-local-fixture-")
}

pub(crate) fn namespaced_key(key: &str, prefix: &str) -> bool {
    key.strip_prefix(prefix).is_some_and(|value| {
        value.len() == 32 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sensenova_fixture_requires_literal_loopback_and_uid_scoped_synthetic_key() {
        assert!(loopback_proxy("127.0.0.1:40000").is_some());
        for bad in [
            "localhost:40000",
            "127.0.0.1:0",
            "127.0.0.2:40000",
            "192.168.1.1:40000",
            "http://127.0.0.1:40000",
        ] {
            assert!(loopback_proxy(bad).is_none(), "{bad}");
        }
        assert!(synthetic_key(
            "cpk-local-fixture-00112233445566778899aabbccddeeff"
        ));
        assert!(!synthetic_key("cpk-regular-account-key"));
        assert!(!synthetic_key(
            "cpk-local-fixture-zz112233445566778899aabbccddeeff"
        ));
    }

    #[test]
    fn sensenova_fixture_rejects_nonfixture_database_and_default_ports() {
        assert!(fixture_service(
            "postgresql://admin@127.0.0.1:15532/gateway_fixture",
            "postgresql",
            5432,
            "/gateway_fixture"
        ));
        for bad in [
            "postgresql://admin@127.0.0.1:15532/production",
            "postgresql://admin@127.0.0.1:5432/gateway_fixture",
            "postgresql://admin@localhost:15532/gateway_fixture",
            "postgresql://admin@127.0.0.1:15532/gateway_fixture?options=foreign",
        ] {
            assert!(
                !fixture_service(bad, "postgresql", 5432, "/gateway_fixture"),
                "{bad}"
            );
        }
        assert!(fixture_service(
            "redis://127.0.0.1:16379/",
            "redis",
            6379,
            "/"
        ));
    }
}
