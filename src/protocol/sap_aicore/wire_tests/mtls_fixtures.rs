//! Ephemeral certificates for loopback tests only; no private keys in the repository.
use btls::{
    asn1::Asn1Time,
    bn::BigNum,
    hash::MessageDigest,
    pkey::{PKey, Private},
    rsa::Rsa,
    x509::{
        extension::{BasicConstraints, ExtendedKeyUsage, SubjectAlternativeName},
        X509NameBuilder, X509,
    },
};
use serde_json::{json, Value};
use std::sync::OnceLock;

pub(super) struct Fixtures {
    pub root_pem: Vec<u8>,
    pub root_der: Vec<u8>,
    pub server_der: Vec<u8>,
    pub server_key: Vec<u8>,
    client: (String, String),
    rotated: (String, String),
}

pub(super) fn get() -> &'static Fixtures {
    static FIXTURES: OnceLock<Fixtures> = OnceLock::new();
    FIXTURES.get_or_init(|| {
        let ca_key = key();
        let ca = certificate(1, "Gateway synthetic SAP fixture CA", &ca_key, None);
        let server_key = key();
        let server = certificate(2, "localhost", &server_key, Some((&ca, &ca_key)));
        let client_identity = |id| {
            let client_key = key();
            let cert = certificate(
                id,
                "Gateway synthetic SAP client",
                &client_key,
                Some((&ca, &ca_key)),
            );
            (
                String::from_utf8(cert.to_pem().unwrap()).unwrap(),
                String::from_utf8(client_key.rsa().unwrap().private_key_to_pem().unwrap()).unwrap(),
            )
        };
        Fixtures {
            root_pem: ca.to_pem().unwrap(),
            root_der: ca.to_der().unwrap(),
            server_der: server.to_der().unwrap(),
            server_key: server_key.private_key_to_pem_pkcs8().unwrap(),
            client: client_identity(3),
            rotated: client_identity(4),
        }
    })
}

pub(super) fn service_key(rotated: bool) -> Value {
    let fixture = get();
    let (certificate, key) = if rotated {
        &fixture.rotated
    } else {
        &fixture.client
    };
    json!({
        "clientid":"synthetic-sap-client",
        "url":"https://synthetic.authentication.eu10.hana.ondemand.com",
        "certurl":"https://synthetic.authentication.cert.eu10.hana.ondemand.com",
        "certificate":certificate,
        "key":key,
        "serviceurls":{"AI_API_URL":"https://api.ai.prod.eu-central-1.aws.ml.hana.ondemand.com"}
    })
}

fn key() -> PKey<Private> {
    PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap()
}

fn certificate(
    serial: u32,
    subject: &str,
    key: &PKey<Private>,
    issuer: Option<(&X509, &PKey<Private>)>,
) -> X509 {
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", subject).unwrap();
    let name = name.build();
    let mut cert = X509::builder().unwrap();
    cert.set_version(2).unwrap();
    cert.set_serial_number(&BigNum::from_u32(serial).unwrap().to_asn1_integer().unwrap())
        .unwrap();
    cert.set_subject_name(&name).unwrap();
    cert.set_issuer_name(issuer.map_or(name.as_ref(), |(ca, _)| ca.subject_name()))
        .unwrap();
    cert.set_not_before(&Asn1Time::from_unix(1_577_836_800).unwrap())
        .unwrap();
    cert.set_not_after(&Asn1Time::days_from_now(2).unwrap())
        .unwrap();
    cert.set_pubkey(key).unwrap();
    let mut constraints = BasicConstraints::new();
    if issuer.is_none() {
        constraints.ca();
    }
    cert.append_extension(&constraints.critical().build().unwrap())
        .unwrap();
    if subject == "localhost" {
        let context = cert.x509v3_context(issuer.map(|(ca, _)| ca.as_ref()), None);
        let san = SubjectAlternativeName::new()
            .dns(subject)
            .build(&context)
            .unwrap();
        cert.append_extension(&san).unwrap();
        cert.append_extension(&ExtendedKeyUsage::new().server_auth().build().unwrap())
            .unwrap();
    } else if issuer.is_some() {
        cert.append_extension(&ExtendedKeyUsage::new().client_auth().build().unwrap())
            .unwrap();
    }
    cert.sign(
        issuer.map_or(key, |(_, issuer_key)| issuer_key),
        MessageDigest::sha256(),
    )
    .unwrap();
    cert.build()
}
