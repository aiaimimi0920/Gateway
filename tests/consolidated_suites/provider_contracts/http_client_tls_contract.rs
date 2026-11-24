//! Loopback ClientHello checks for the two deployed Chrome profiles.
//! Normalize per-connection randomness without claiming a complete JA3/JA4 identity.
//! This does not establish server certificate, HTTP/2 SETTINGS or provider acceptance.
use rquest::Client;
use rquest_util::{Emulation, Profile};
use std::time::Duration;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;

const MAX_HELLO: usize = 16 * 1024;
const DEADLINE: Duration = Duration::from_secs(3);

#[derive(Debug, Default)]
struct Hello {
    ciphers: Vec<u16>,
    groups: Vec<u16>,
    signatures: Vec<u16>,
    versions: Vec<u16>,
    extensions: Vec<u16>,
    alpn: Vec<Vec<u8>>,
    grease_cipher: bool,
}

struct Fields<'a>(&'a [u8]);

impl<'a> Fields<'a> {
    fn take(&mut self, count: usize) -> &'a [u8] {
        assert!(count <= self.0.len(), "ClientHello field exceeds its bound");
        let (field, rest) = self.0.split_at(count);
        self.0 = rest;
        field
    }

    fn byte(&mut self) -> usize {
        usize::from(self.take(1)[0])
    }

    fn word(&mut self) -> usize {
        let bytes = self.take(2);
        usize::from(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn vector8(&mut self) -> &'a [u8] {
        let count = self.byte();
        self.take(count)
    }

    fn vector16(&mut self) -> &'a [u8] {
        let count = self.word();
        self.take(count)
    }
}

fn grease(value: u16) -> bool {
    value & 0x0f0f == 0x0a0a && value.to_be_bytes()[0] == value.to_be_bytes()[1]
}

fn words(bytes: &[u8]) -> Vec<u16> {
    assert_eq!(bytes.len() % 2, 0, "incomplete TLS parameter");
    bytes
        .chunks_exact(2)
        .map(|pair| u16::from_be_bytes([pair[0], pair[1]]))
        .filter(|value| !grease(*value))
        .collect()
}

fn decode(bytes: &[u8]) -> Hello {
    assert_eq!(bytes[0], 1, "expected ClientHello handshake");
    let mut fields = Fields(&bytes[4..]);
    fields.take(2 + 32); // Legacy version and per-connection random.
    fields.vector8(); // Session ID is deliberately excluded.
    let cipher_bytes = fields.vector16();
    let mut hello = Hello {
        ciphers: words(cipher_bytes),
        grease_cipher: cipher_bytes
            .chunks_exact(2)
            .any(|pair| grease(u16::from_be_bytes([pair[0], pair[1]]))),
        ..Hello::default()
    };
    fields.vector8(); // Legacy compression methods.
    let mut extensions = Fields(fields.vector16());
    assert!(fields.0.is_empty());
    while !extensions.0.is_empty() {
        let kind = extensions.word() as u16;
        let mut data = Fields(extensions.vector16());
        if !grease(kind) {
            hello.extensions.push(kind);
        }
        match kind {
            10 => hello.groups = words(data.vector16()),
            13 => hello.signatures = words(data.vector16()),
            43 => hello.versions = words(data.vector8()),
            16 => {
                let mut names = Fields(data.vector16());
                while !names.0.is_empty() {
                    hello.alpn.push(names.vector8().to_vec());
                }
            }
            _ => continue,
        }
        assert!(data.0.is_empty());
    }
    hello.extensions.sort_unstable();
    hello
}

async fn read_hello(listener: TcpListener) -> Hello {
    let (mut socket, peer) = listener.accept().await.unwrap();
    assert!(peer.ip().is_loopback());
    let mut handshake = Vec::new();
    loop {
        let mut header = [0; 5];
        socket.read_exact(&mut header).await.unwrap();
        assert_eq!(header[0], 22, "expected TLS handshake record");
        let length = usize::from(u16::from_be_bytes([header[3], header[4]]));
        assert!(length > 0 && handshake.len() + length <= MAX_HELLO);
        let start = handshake.len();
        handshake.resize(start + length, 0);
        socket.read_exact(&mut handshake[start..]).await.unwrap();
        if handshake.len() >= 4 {
            let length = (usize::from(handshake[1]) << 16)
                | (usize::from(handshake[2]) << 8)
                | usize::from(handshake[3]);
            assert!(length + 4 <= MAX_HELLO);
            if handshake.len() >= length + 4 {
                assert_eq!(handshake.len(), length + 4);
                return decode(&handshake);
            }
        }
    }
}

async fn capture(profile: Option<Profile>) -> Hello {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let target = format!("https://{}/", listener.local_addr().unwrap());
    let mut builder = Client::builder().no_proxy().timeout(DEADLINE);
    if let Some(profile) = profile {
        builder = builder.emulation(profile);
    }
    let client = builder.build().unwrap();
    // No spawned fixture task: deadline/caller cancellation drops both sockets.
    let (result, hello) = tokio::time::timeout(DEADLINE, async {
        tokio::join!(client.get(target).send(), read_hello(listener))
    })
    .await
    .expect("loopback ClientHello capture must be bounded");
    assert!(
        result.is_err(),
        "fixture does not complete or trust a TLS server"
    );
    hello
}

fn assert_chrome(hello: &Hello, alps: u16) {
    // Reviewed rquest-util Chrome cipher/signature order, excluding GREASE.
    assert_eq!(
        hello.ciphers,
        [
            0x1301, 0x1302, 0x1303, 0xc02b, 0xc02f, 0xc02c, 0xc030, 0xcca9, 0xcca8, 0xc013, 0xc014,
            0x009c, 0x009d, 0x002f, 0x0035,
        ]
    );
    assert_eq!(hello.groups, [4588, 29, 23, 24]);
    assert_eq!(
        hello.signatures,
        [0x0403, 0x0804, 0x0401, 0x0503, 0x0805, 0x0501, 0x0806, 0x0601]
    );
    assert_eq!(hello.versions, [0x0304, 0x0303]);
    assert_eq!(hello.alpn, [b"h2".to_vec(), b"http/1.1".to_vec()]);
    assert!(hello.grease_cipher);
    assert!(hello.extensions.contains(&alps));
    assert!(!hello
        .extensions
        .contains(&if alps == 17513 { 17613 } else { 17513 }));
}

#[tokio::test]
async fn splitter_chrome131_preserves_reviewed_client_hello() {
    assert_chrome(&capture(Some(Emulation::Chrome131)).await, 17513);
}

#[tokio::test]
async fn upstream_chrome136_preserves_reviewed_client_hello() {
    assert_chrome(&capture(Some(Emulation::Chrome136)).await, 17613);
}

#[tokio::test]
async fn plain_client_does_not_acquire_chrome_alps_emulation() {
    let hello = capture(None).await;
    assert!(!hello.extensions.contains(&17513));
    assert!(!hello.extensions.contains(&17613));
}
