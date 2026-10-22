//! Process-owned explicit CA configuration; never sourced from inbound headers or JSON.
use rustls::pki_types::{pem::PemObject, CertificateDer};
use std::{ffi::OsString, io::Read, sync::OnceLock};

const MAX_CA_BYTES: u64 = 1024 * 1024;
type Configuration = Result<Option<rustls::ClientConfig>, &'static str>;
static CONFIGURATION: OnceLock<Configuration> = OnceLock::new();
static WS_DEFAULT: OnceLock<Result<rustls::ClientConfig, &'static str>> = OnceLock::new();

fn selected_path(primary: Option<OsString>, fallback: Option<OsString>) -> Option<OsString> {
    primary
        .filter(|p| !p.is_empty())
        .or_else(|| fallback.filter(|p| !p.is_empty()))
}

fn parse(mut input: impl Read) -> Result<Vec<CertificateDer<'static>>, &'static str> {
    let mut bytes = Vec::new();
    input
        .by_ref()
        .take(MAX_CA_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| "cannot read custom CA bundle")?;
    if bytes.len() as u64 > MAX_CA_BYTES {
        return Err("custom CA bundle exceeds 1 MiB");
    }
    let certificates = CertificateDer::pem_slice_iter(&bytes)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "invalid PEM custom CA bundle")?;
    if certificates.is_empty() {
        return Err("custom CA bundle contains no certificates");
    }
    Ok(certificates)
}

fn load() -> Configuration {
    let path = selected_path(
        std::env::var_os("CODEX_CA_CERTIFICATE"),
        std::env::var_os("SSL_CERT_FILE"),
    );
    path.map(|path| {
        let input = std::fs::File::open(path).map_err(|_| "cannot open custom CA bundle")?;
        let certificates = parse(input)?;
        tls_config(certificates)
    })
    .transpose()
}

fn tls_config(
    certificates: Vec<CertificateDer<'static>>,
) -> Result<rustls::ClientConfig, &'static str> {
    let mut roots = rustls::RootCertStore::empty();
    roots.add_parsable_certificates(rustls_native_certs::load_native_certs().certs);
    for certificate in certificates {
        roots
            .add(certificate)
            .map_err(|_| "invalid X509 custom CA certificate")?;
    }
    // This lane must not inherit main's global ring provider: the reference
    // custom-CA client offers the AWS-LC default suites and hybrid key shares.
    let mut provider = rustls::crypto::aws_lc_rs::default_provider();
    // The reference CLI predates ML-DSA signature negotiation. Keep certificate
    // chain verification algorithms intact; constrain only the advertised
    // handshake schemes in this version-bound lane, not global Rustls policy.
    let mapping = provider.signature_verification_algorithms.mapping;
    if let Some(index) = mapping.iter().position(|(scheme, _)| is_ml_dsa(*scheme)) {
        if !mapping[index..]
            .iter()
            .all(|(scheme, _)| is_ml_dsa(*scheme))
        {
            return Err("unexpected native signature algorithm ordering");
        }
        provider.signature_verification_algorithms.mapping = &mapping[..index];
    }
    rustls::ClientConfig::builder_with_provider(std::sync::Arc::new(provider))
        .with_safe_default_protocol_versions()
        .map_err(|_| "invalid native TLS protocol configuration")
        .map(|builder| {
            let mut config = builder.with_root_certificates(roots).with_no_client_auth();
            config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];
            config
        })
}

fn is_ml_dsa(scheme: rustls::SignatureScheme) -> bool {
    matches!(
        scheme,
        rustls::SignatureScheme::ML_DSA_44
            | rustls::SignatureScheme::ML_DSA_65
            | rustls::SignatureScheme::ML_DSA_87
    )
}

pub(super) fn configure(
    builder: reqwest::ClientBuilder,
    websocket: bool,
) -> Result<(reqwest::ClientBuilder, bool), &'static str> {
    // UpstreamClient initializes this once at startup. New accepted connections
    // reuse parsed roots, avoiding filesystem IO on the request hot path.
    match CONFIGURATION.get_or_init(load) {
        Ok(Some(config)) => {
            let mut config = config.clone();
            // RFC 6455 upgrade uses HTTP/1.1, not h2 extended CONNECT. The
            // reference WS client offers no ALPN; HTTP clients retain their list.
            if websocket {
                config.alpn_protocols.clear();
            }
            // Share immutable trust configuration, not TLS sessions across
            // unrelated accepted client connections.
            config.resumption = rustls::client::Resumption::default();
            Ok((builder.use_preconfigured_tls(config), true))
        }
        Ok(None) if websocket => {
            // Codex WS uses explicit Rustls even without a custom CA. Share the
            // roots/provider policy, never the HTTP Schannel compatibility ceiling.
            let config = WS_DEFAULT
                .get_or_init(|| tls_config(Vec::new()))
                .as_ref()
                .map_err(|e| *e)?;
            let mut config = config.clone();
            config.alpn_protocols.clear();
            config.resumption = rustls::client::Resumption::default();
            Ok((builder.use_preconfigured_tls(config), true))
        }
        Ok(None) => Ok((builder.use_native_tls(), false)),
        Err(error) => Err(error),
    }
}

pub(super) fn websocket_tls() -> Result<rustls::ClientConfig, &'static str> {
    let config = match CONFIGURATION.get_or_init(load) {
        Ok(Some(config)) => config,
        Ok(None) => WS_DEFAULT
            .get_or_init(|| tls_config(Vec::new()))
            .as_ref()
            .map_err(|e| *e)?,
        Err(error) => return Err(error),
    };
    let mut config = config.clone();
    config.alpn_protocols.clear();
    config.resumption = rustls::client::Resumption::default();
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_ca_path_priority_and_empty_fallback() {
        assert_eq!(
            selected_path(Some("explicit.pem".into()), Some("fallback.pem".into())),
            Some("explicit.pem".into())
        );
        assert_eq!(
            selected_path(Some("".into()), Some("fallback.pem".into())),
            Some("fallback.pem".into())
        );
        assert_eq!(selected_path(None, Some("".into())), None);
    }

    #[test]
    fn native_ca_invalid_empty_and_oversized_bundles_fail_closed() {
        assert!(parse(&b""[..]).is_err());
        assert!(parse(&b"not a certificate"[..]).is_err());
        assert!(parse(&vec![b'x'; MAX_CA_BYTES as usize + 1][..]).is_err());
    }
}
