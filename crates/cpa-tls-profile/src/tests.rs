//! Offline wire regression against the captured CPA/Go 1.26.1 first flight.
use super::*;
use rama_boring::ssl::{SslConnector, SslMethod, SslStream, SslVerifyMode};
use std::io::{self, Read, Write};

#[derive(Debug, Default)]
struct Sink(Vec<u8>);
impl Read for Sink {
    fn read(&mut self, _: &mut [u8]) -> io::Result<usize> {
        Ok(0)
    }
}
impl Write for Sink {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn take<'a>(bytes: &mut &'a [u8], length: usize) -> &'a [u8] {
    let (head, tail) = bytes.split_at(length);
    *bytes = tail;
    head
}
fn number(bytes: &mut &[u8]) -> u16 {
    u16::from_be_bytes(take(bytes, 2).try_into().unwrap())
}
fn vector(bytes: &mut &[u8]) -> Vec<u16> {
    let length = number(bytes) as usize;
    take(bytes, length)
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes(pair.try_into().unwrap()))
        .collect()
}

#[test]
fn first_flight_matches_go_without_weakening_peer_verification() {
    // Repeated samples ensure random key material cannot affect the shape assertion.
    for _ in 0..3 {
        let mut context = SslConnector::builder(SslMethod::tls_client()).unwrap();
        configure_context(&mut context).unwrap();
        let mut ssl = context
            .build()
            .configure()
            .unwrap()
            .into_ssl(Some("ai.hybgzs.com"))
            .unwrap();
        configure_connection(&mut ssl).unwrap();
        assert!(ssl.verify_mode().contains(SslVerifyMode::PEER));
        let mut stream = SslStream::new(ssl, Sink::default()).unwrap();
        assert!(stream.connect().is_err()); // Intentional EOF, not an insecure handshake.
        let mut bytes = stream.get_ref().0.as_slice();
        assert_eq!(take(&mut bytes, 5), &[22, 3, 1, 5, 238]);
        assert_eq!(take(&mut bytes, 4), &[1, 0, 5, 234]);
        assert_eq!(number(&mut bytes), 771);
        take(&mut bytes, 32); // Client random.
        assert_eq!(take(&mut bytes, 1), &[32]);
        take(&mut bytes, 32); // Random compatibility session ID.
        assert_eq!(vector(&mut bytes), CIPHERS);
        assert_eq!(take(&mut bytes, 2), &[1, 0]);
        assert_eq!(number(&mut bytes) as usize, bytes.len());
        let mut extensions = Vec::new();
        while !bytes.is_empty() {
            let id = number(&mut bytes);
            let length = number(&mut bytes) as usize;
            extensions.push((id, take(&mut bytes, length).to_vec()));
        }
        assert_eq!(
            extensions.iter().map(|v| v.0).collect::<Vec<_>>(),
            EXTENSIONS
        );
        let body = |id| extensions.iter().find(|v| v.0 == id).unwrap().1.as_slice();
        assert_eq!(body(0), b"\0\x10\0\0\x0dai.hybgzs.com");
        assert_eq!(body(11), &[1, 0]);
        assert_eq!(body(65281), &[0]);
        assert!(body(23).is_empty());
        assert!(body(18).is_empty());
        assert_eq!(body(5), &[1, 0, 0, 0, 0]);
        assert_eq!(vector(&mut body(10)), [4588, 4587, 4589, 29, 23, 24, 25]);
        assert_eq!(vector(&mut body(13)), SIGNATURES);
        let cert_signatures: Vec<_> = SIGNATURES.iter().copied().chain([513, 515]).collect();
        assert_eq!(vector(&mut body(50)), cert_signatures);
        assert_eq!(body(16), b"\0\x0c\x02h2\x08http/1.1");
        assert_eq!(body(43), &[4, 3, 4, 3, 3]);
        let mut shares = body(51);
        assert_eq!(number(&mut shares) as usize, shares.len());
        for (group, size) in [(4588, 1216), (29, 32)] {
            assert_eq!(number(&mut shares), group);
            assert_eq!(number(&mut shares), size);
            take(&mut shares, size as usize); // Ephemeral public key material.
        }
        assert!(shares.is_empty());
    }
}
