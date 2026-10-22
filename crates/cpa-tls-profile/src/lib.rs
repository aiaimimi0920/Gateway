//! Experimental Go 1.26.1 ClientHello profile, not a complete CPA HTTP transport.
//! Authentication, trust roots, hostname verification and HTTP remain caller-owned.
use rama_boring::{
    error::ErrorStack,
    ssl::{
        ClientHelloShape, Padding, SslContextBuilder, SslOptions, SslRef, SslVersion, UnknownValues,
    },
};

const CIPHERS: &[u16] = &[
    49195, 49199, 49196, 49200, 52393, 52392, 49161, 49171, 49162, 49172, 4865, 4866, 4867,
];
const SIGNATURES: &[u16] = &[2052, 1027, 2055, 2053, 2054, 1025, 1281, 1537, 1283, 1539];
const EXTENSIONS: &[u16] = &[0, 11, 65281, 23, 18, 5, 10, 13, 50, 16, 43, 51];

/// Configure a dedicated context, before constructing its connections.
/// Does not disable certificate or hostname validation or set a credential.
pub fn configure_context(context: &mut SslContextBuilder) -> Result<(), ErrorStack> {
    // Fail closed on an unpatched backend instead of advertising unimplemented groups.
    context.set_curves_list(
        "X25519MLKEM768:SecP256r1MLKEM768:SecP384r1MLKEM1024:X25519:P-256:P-384:P-521",
    )?;
    context.set_min_proto_version(Some(SslVersion::TLS1_2))?;
    context.set_max_proto_version(Some(SslVersion::TLS1_3))?;
    context.set_options(SslOptions::NO_TICKET);
    context.set_grease_enabled(false);
    context.enable_signed_cert_timestamps();
    context.enable_ocsp_stapling();
    context.set_alpn_protos(b"\x02h2\x08http/1.1")?;
    Ok(())
}

/// Configure a fresh client connection before emitting any TLS bytes.
/// Context initialization requires the pinned hybrid-group backend. No resumption is advertised.
pub fn configure_connection(ssl: &mut SslRef) -> Result<(), ErrorStack> {
    let mut shape = ClientHelloShape::new();
    shape
        .set_unknown_values(UnknownValues::Preserve)
        .set_padding(Padding::Off)
        .set_omit_psk_key_exchange_modes(true);
    let mut certificate_signatures = vec![0, 24];
    for signature in SIGNATURES.iter().chain([513, 515].iter()) {
        certificate_signatures.extend_from_slice(&signature.to_be_bytes());
    }
    shape.set_extension_body(50.into(), certificate_signatures);
    ssl.set_client_hello_shape(&shape)?;
    ssl.set_raw_cipher_list(CIPHERS)?;
    ssl.set_raw_supported_groups(&[4588, 4587, 4589, 29, 23, 24, 25])?;
    ssl.set_raw_key_shares(&[4588, 29])?;
    ssl.set_raw_signature_algorithms(SIGNATURES)?;
    ssl.set_raw_supported_versions(&[772, 771])?;
    ssl.set_extension_order(EXTENSIONS)?;
    Ok(())
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod hybrid_tests;
#[cfg(test)]
mod malformed_peer;
