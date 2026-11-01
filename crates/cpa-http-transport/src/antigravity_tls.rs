//! First-flight shape captured from the installed, unmodified Windows native language server.
//! This pins wire advertisement, not complete cryptographic/HRR/resumption equivalence.
use rama_boring::{
    error::ErrorStack,
    ssl::{
        ClientHelloShape, Padding, SslContextBuilder, SslOptions, SslRef, SslVersion, UnknownValues,
    },
};

const CIPHERS: &[u16] = &[
    49195, 49199, 49196, 49200, 52393, 52392, 49161, 49171, 49162, 49172, 4865, 4866, 4867,
];
const SIGNATURES: &[u16] = &[
    2308, 2309, 2310, 2052, 1027, 2055, 2053, 2054, 1025, 1281, 1537, 1283, 1539,
];

pub(crate) fn context(context: &mut SslContextBuilder) -> Result<(), ErrorStack> {
    // Require real hybrid-group support; never substitute a browser or default profile.
    context.set_curves_list(
        "X25519MLKEM768:SecP256r1MLKEM768:SecP384r1MLKEM1024:X25519:P-256:P-384:P-521",
    )?;
    context.set_min_proto_version(Some(SslVersion::TLS1_2))?;
    context.set_max_proto_version(Some(SslVersion::TLS1_3))?;
    context.set_options(SslOptions::NO_TICKET);
    context.set_grease_enabled(false);
    context.enable_signed_cert_timestamps();
    context.enable_ocsp_stapling();
    Ok(())
}

pub(crate) fn connection(ssl: &mut SslRef) -> Result<(), ErrorStack> {
    let mut shape = ClientHelloShape::new();
    shape
        .set_unknown_values(UnknownValues::Preserve)
        .set_padding(Padding::Off)
        .set_omit_psk_key_exchange_modes(true);
    // Preserve native advertisement, including ML-DSA IDs. This does not add an
    // implementation of unsupported certificate/signature algorithms to the backend.
    let mut cert_signatures = Vec::with_capacity(32);
    cert_signatures.extend_from_slice(&30u16.to_be_bytes());
    for signature in SIGNATURES.iter().chain([513, 515].iter()) {
        cert_signatures.extend_from_slice(&signature.to_be_bytes());
    }
    shape.set_extension_body(50.into(), cert_signatures);
    ssl.set_client_hello_shape(&shape)?;
    ssl.set_raw_cipher_list(CIPHERS)?;
    ssl.set_raw_supported_groups(&[4588, 4587, 4589, 29, 23, 24, 25])?;
    ssl.set_raw_key_shares(&[4588, 29])?;
    ssl.set_raw_signature_algorithms(SIGNATURES)?;
    ssl.set_raw_supported_versions(&[772, 771])?;
    ssl.set_extension_order(&[0, 11, 65281, 23, 18, 5, 10, 13, 50, 43, 51])?;
    Ok(())
}
