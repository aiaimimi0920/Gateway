//! TLS reference measured from Claude Code 2.1.278 on Windows, not a browser profile.
//! Keep certificate/hostname verification and the shared explicit proxy policy intact.
use rquest::tls::{AlpnProtocol, TlsOptions};
use std::time::Duration;

// TLS 1.3 uses BoringSSL's default AES128/AES256/CHACHA ordering. This list
// constrains TLS 1.2 to the suites actually offered by the reference CLI.
const CIPHERS: &str = concat!(
    "ECDHE-ECDSA-AES128-GCM-SHA256:ECDHE-RSA-AES128-GCM-SHA256:",
    "ECDHE-ECDSA-AES256-GCM-SHA384:ECDHE-RSA-AES256-GCM-SHA384:",
    "ECDHE-ECDSA-CHACHA20-POLY1305:ECDHE-RSA-CHACHA20-POLY1305:",
    "ECDHE-ECDSA-AES128-SHA:ECDHE-RSA-AES128-SHA:",
    "ECDHE-ECDSA-AES256-SHA:ECDHE-RSA-AES256-SHA:",
    "AES128-GCM-SHA256:AES256-GCM-SHA384:AES128-SHA:AES256-SHA"
);

fn options() -> TlsOptions {
    TlsOptions::builder()
        .alpn_protocols([AlpnProtocol::HTTP1])
        .cipher_list(CIPHERS)
        .curves_list("X25519MLKEM768:X25519:P-256:P-384")
        .enable_ocsp_stapling(true)
        .enable_signed_cert_timestamps(true)
        .build()
}

pub(crate) fn client(timeout: Duration) -> rquest::Client {
    super::builder()
        .tls_options(options())
        .http1_only()
        .timeout(timeout)
        .build()
        .expect("failed to build native Claude HTTP client")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_claude_tls_reference_is_explicit_and_not_browser_emulation() {
        let value = options();
        assert_eq!(
            value.alpn_protocols.as_deref(),
            Some(&[AlpnProtocol::HTTP1][..])
        );
        assert_eq!(value.cipher_list.as_deref(), Some(CIPHERS));
        assert_eq!(
            value.curves_list.as_deref(),
            Some("X25519MLKEM768:X25519:P-256:P-384")
        );
        assert!(value.enable_ocsp_stapling && value.enable_signed_cert_timestamps);
        assert!(!value.enable_ech_grease);
        assert!(value.alps_protocols.is_none());
        // Build also validates BoringSSL cipher and group names without network I/O.
        let _ = client(Duration::from_secs(5));
    }
}
