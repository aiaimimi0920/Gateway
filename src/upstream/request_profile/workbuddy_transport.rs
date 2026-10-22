//! WorkBuddy 5.8.4 bundled Node ClientHello, measured through a byte-preserving tunnel.
//! Request-local emulation partitions the pool; verification/proxy policy stay client-owned.
use rquest::{tls::TlsOptions, Emulation, Group, RequestBuilder};

fn emulation() -> Emulation {
    let tls = TlsOptions::builder()
        .alpn_protocols([])
        .curves_list("X25519:P-256:P-384")
        .cipher_list(concat!(
            "ECDHE-RSA-AES128-GCM-SHA256:ECDHE-ECDSA-AES128-GCM-SHA256:",
            "ECDHE-RSA-AES256-GCM-SHA384:ECDHE-ECDSA-AES256-GCM-SHA384:",
            "ECDHE-RSA-AES128-SHA256:ECDHE-ECDSA-CHACHA20-POLY1305:",
            "ECDHE-RSA-CHACHA20-POLY1305:ECDHE-ECDSA-AES128-SHA:",
            "ECDHE-RSA-AES128-SHA:ECDHE-ECDSA-AES256-SHA:ECDHE-RSA-AES256-SHA:",
            "AES128-GCM-SHA256:AES256-GCM-SHA384:AES128-SHA:AES256-SHA"
        ))
        .sigalgs_list(concat!(
            "ecdsa_secp256r1_sha256:rsa_pss_rsae_sha256:rsa_pkcs1_sha256:",
            "ecdsa_secp384r1_sha384:rsa_pss_rsae_sha384:rsa_pkcs1_sha384:",
            "rsa_pss_rsae_sha512:rsa_pkcs1_sha512:rsa_pkcs1_sha1"
        ))
        .build();
    Emulation::builder()
        .tls_options(tls)
        .build(Group::new("workbuddy-5.8.4"))
}

pub(crate) fn apply(builder: RequestBuilder) -> RequestBuilder {
    builder
        .emulation(emulation())
        // Explicit HTTP_11 makes wreq inject ALPN and padding despite empty TLS options.
        // No ALPN negotiates HTTP/1.1 naturally, matching the reference Node client.
        .redirect(rquest::redirect::Policy::none())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workbuddy_transport_has_no_browser_negotiation_or_grease() {
        let profile = emulation();
        let tls = profile.tls_options.unwrap();
        assert!(tls.alpn_protocols.unwrap().is_empty());
        assert_eq!(tls.curves_list.as_deref(), Some("X25519:P-256:P-384"));
        assert!(!tls.enable_ech_grease);
        assert!(!tls.enable_ocsp_stapling);
        assert!(!tls.enable_signed_cert_timestamps);
        assert!(profile.headers.is_empty());
        let request = apply(rquest::Client::new().get("https://example.com"))
            .build()
            .unwrap();
        assert_eq!(request.version(), None);
    }
}
