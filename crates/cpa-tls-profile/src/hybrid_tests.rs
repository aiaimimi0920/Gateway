//! Real, verified loopback handshakes for both hybrid groups and TLS version boundaries.
use super::*;
use rama_boring::{
    asn1::Asn1Time,
    bn::BigNum,
    hash::MessageDigest,
    pkey::PKey,
    rsa::Rsa,
    ssl::{Ssl, SslConnector, SslMethod, SslStream},
    x509::{X509NameBuilder, X509},
};
use std::{
    io::{Read, Write},
    net::{TcpListener, TcpStream},
    time::Duration,
};

fn handshake(
    group: &str,
    id: u16,
    direct: bool,
    version: SslVersion,
    fault: Option<bool>,
) -> (bool, bool) {
    let key = PKey::from_rsa(Rsa::generate(2048).unwrap()).unwrap();
    let mut name = X509NameBuilder::new().unwrap();
    name.append_entry_by_text("CN", "localhost").unwrap();
    let name = name.build();
    let mut cert = X509::builder().unwrap();
    cert.set_version(2).unwrap();
    cert.set_serial_number(&BigNum::from_u32(1).unwrap().to_asn1_integer().unwrap())
        .unwrap();
    cert.set_subject_name(&name).unwrap();
    cert.set_issuer_name(&name).unwrap();
    cert.set_pubkey(&key).unwrap();
    cert.set_not_before(&Asn1Time::days_from_now(0).unwrap())
        .unwrap();
    cert.set_not_after(&Asn1Time::days_from_now(1).unwrap())
        .unwrap();
    cert.sign(&key, MessageDigest::sha256()).unwrap();
    let cert = cert.build();
    let mut server = SslContextBuilder::new(SslMethod::tls_server()).unwrap();
    server.set_certificate(&cert).unwrap();
    server.set_private_key(&key).unwrap();
    server.set_curves_list(group).unwrap();
    server.set_min_proto_version(Some(version)).unwrap();
    server.set_max_proto_version(Some(version)).unwrap();
    let server = server.build();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let task = std::thread::spawn(move || {
        let (socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        socket
            .set_write_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut stream = SslStream::new(Ssl::new(&server).unwrap(), socket).unwrap();
        if stream.accept().is_err() {
            return false;
        }
        let mut bytes = [0; 4];
        stream.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"ping");
        stream.write_all(b"pong").unwrap();
        true
    });
    let mut client = SslConnector::builder(SslMethod::tls_client()).unwrap();
    configure_context(&mut client).unwrap();
    client.cert_store_mut().add_cert(cert).unwrap();
    let mut ssl = client
        .build()
        .configure()
        .unwrap()
        .into_ssl(Some("localhost"))
        .unwrap();
    configure_connection(&mut ssl).unwrap();
    if direct {
        ssl.set_raw_key_shares(&[id]).unwrap();
    }
    let socket = TcpStream::connect(address).unwrap();
    socket
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    socket
        .set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut stream = SslStream::new(
        ssl,
        super::malformed_peer::Peer {
            socket,
            wrong_length: fault,
        },
    )
    .unwrap();
    let success = stream.connect().is_ok();
    if success {
        stream.write_all(b"ping").unwrap();
        let mut bytes = [0; 4];
        stream.read_exact(&mut bytes).unwrap();
        assert_eq!(&bytes, b"pong");
    }
    drop(stream);
    (success, task.join().unwrap())
}

#[test]
fn both_hybrid_groups_support_hrr_and_direct_first_flight() {
    for (group, id) in [("SecP256r1MLKEM768", 4587), ("SecP384r1MLKEM1024", 4589)] {
        for direct in [false, true] {
            assert_eq!(
                handshake(group, id, direct, SslVersion::TLS1_3, None),
                (true, true),
                "{group}: direct={direct}"
            );
        }
    }
}

#[test]
fn hybrid_only_server_cannot_negotiate_tls12() {
    for (group, id) in [("SecP256r1MLKEM768", 4587), ("SecP384r1MLKEM1024", 4589)] {
        assert_eq!(
            handshake(group, id, false, SslVersion::TLS1_2, None),
            (false, false)
        );
    }
}

#[test]
fn malformed_hybrid_points_and_lengths_are_rejected() {
    for (group, id) in [("SecP256r1MLKEM768", 4587), ("SecP384r1MLKEM1024", 4589)] {
        for wrong_length in [false, true] {
            assert_eq!(
                handshake(group, id, true, SslVersion::TLS1_3, Some(wrong_length)),
                (false, false)
            );
        }
    }
}
