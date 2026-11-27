//! Credential-free CONNECT + verified Rustls/h2 diagnostic for the fixed Cursor origins.
//! Build with the repository's already-built Rustls/NativeCerts rlibs; no HTTP or Run is sent.
use std::{
    io::{Read, Write},
    net::{SocketAddrV4, TcpStream},
    sync::Arc,
    time::Duration,
};

fn tls_config() -> Result<Arc<rustls::ClientConfig>, String> {
    let native = rustls_native_certs::load_native_certs();
    let mut roots = rustls::RootCertStore::empty();
    let mut rejected = 0;
    for cert in native.certs {
        if roots.add(cert).is_err() {
            rejected += 1;
        }
    }
    println!(
        "roots={} rejected={} loader_errors={} SSL_CERT_FILE={} SSL_CERT_DIR={}",
        roots.len(),
        rejected,
        native.errors.len(),
        std::env::var_os("SSL_CERT_FILE").is_some(),
        std::env::var_os("SSL_CERT_DIR").is_some()
    );
    if roots.is_empty() {
        return Err("empty_native_roots".into());
    }
    // Keep the trust policy identical to upstream/transport_live.rs:
    // native roots, ring, TLS 1.3/1.2, hostname verification, h2 only.
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = rustls::ClientConfig::builder_with_provider(provider)
        .with_protocol_versions(&[&rustls::version::TLS13, &rustls::version::TLS12])
        .map_err(|e| format!("config:{e:?}"))?
        .with_root_certificates(roots)
        .with_no_client_auth();
    config.alpn_protocols = vec![b"h2".to_vec()];
    Ok(Arc::new(config))
}

fn probe(port: u16, host: &'static str, config: Arc<rustls::ClientConfig>) -> Result<(), String> {
    let address = SocketAddrV4::new(std::net::Ipv4Addr::LOCALHOST, port);
    let mut tcp = TcpStream::connect_timeout(&address.into(), Duration::from_secs(10))
        .map_err(|e| format!("proxy_connect:{:?}", e.kind()))?;
    tcp.set_read_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| format!("read_timeout:{:?}", e.kind()))?;
    tcp.set_write_timeout(Some(Duration::from_secs(15)))
        .map_err(|e| format!("write_timeout:{:?}", e.kind()))?;
    tcp.set_nodelay(true)
        .map_err(|e| format!("nodelay:{:?}", e.kind()))?;
    write!(
        tcp,
        "CONNECT {host}:443 HTTP/1.1\r\nHost: {host}:443\r\n\r\n"
    )
    .map_err(|e| format!("connect_write:{:?}", e.kind()))?;
    let mut header = Vec::new();
    while !header.ends_with(b"\r\n\r\n") {
        if header.len() >= 4096 {
            return Err("proxy_headers_too_large".into());
        }
        let mut byte = [0];
        tcp.read_exact(&mut byte)
            .map_err(|e| format!("connect_read:{:?}", e.kind()))?;
        header.push(byte[0]);
    }
    if !(header.starts_with(b"HTTP/1.1 200 ")
        || header.starts_with(b"HTTP/1.1 200\r\n")
        || header.starts_with(b"HTTP/1.0 200 ")
        || header.starts_with(b"HTTP/1.0 200\r\n"))
    {
        return Err("proxy_connect_rejected".into());
    }
    println!("host={host} connect=200");
    let name = rustls::pki_types::ServerName::try_from(host).map_err(|_| "invalid_fixed_host")?;
    let mut connection =
        rustls::ClientConnection::new(config, name).map_err(|e| format!("tls_config:{e:?}"))?;
    // No authorization, cookies, HTTP request or inference payload exists in this probe.
    while connection.is_handshaking() {
        connection
            .complete_io(&mut tcp)
            .map_err(|e| format!("tls:{e:?}"))?;
    }
    let protocol = connection.alpn_protocol();
    println!(
        "host={host} verified=true alpn={:?} tls={:?}",
        protocol.map(String::from_utf8_lossy),
        connection.protocol_version()
    );
    if protocol != Some(b"h2") {
        return Err("verified_http2_required".into());
    }
    connection.send_close_notify();
    let _ = connection.complete_io(&mut tcp);
    Ok(())
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let port: u16 = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .filter(|p| *p != 0)
        .expect("explicit IPv4 loopback proxy port required");
    assert_eq!(args.len(), 2, "only a proxy port is accepted");
    // Per-read timeouts alone do not bound a peer that sends one byte at a time.
    std::thread::spawn(|| {
        std::thread::sleep(Duration::from_secs(60));
        eprintln!("diagnostic_deadline_exceeded");
        std::process::exit(2);
    });
    let config = tls_config().expect("native verified TLS configuration required");
    let mut failed = false;
    for host in [
        "api2.cursor.sh",
        "cursor.com",
        "agentn.global.api5.cursor.sh",
    ] {
        if let Err(error) = probe(port, host, config.clone()) {
            eprintln!("host={host} failed={error}");
            failed = true;
        }
    }
    std::process::exit(if failed { 1 } else { 0 });
}
