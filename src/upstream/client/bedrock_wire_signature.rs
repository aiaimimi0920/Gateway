//! Independent HMAC check for these fixtures' fixed URI and empty query only.
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

fn hmac(key: &[u8], input: &str) -> Vec<u8> {
    let mut mac = Hmac::<Sha256>::new_from_slice(key).unwrap();
    mac.update(input.as_bytes());
    mac.finalize().into_bytes().to_vec()
}

pub(super) fn assert_captured_signature(captured: &[u8], expected_path: &str) {
    let end = captured.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let header = std::str::from_utf8(&captured[..end]).unwrap();
    let headers: BTreeMap<_, _> = header
        .lines()
        .skip(1)
        .map(|line| {
            let (name, value) = line.split_once(':').unwrap();
            (
                name.to_ascii_lowercase(),
                value.split_whitespace().collect::<Vec<_>>().join(" "),
            )
        })
        .collect();
    let authorization = &headers["authorization"];
    let fields: BTreeMap<_, _> = authorization
        .strip_prefix("AWS4-HMAC-SHA256 ")
        .unwrap()
        .split(", ")
        .map(|part| part.split_once('=').unwrap())
        .collect();
    let scope = fields["Credential"].strip_prefix("AKIAFIXTURE/").unwrap();
    let signed = fields["SignedHeaders"];
    let canonical_headers: String = signed
        .split(';')
        .map(|name| format!("{name}:{}\n", headers[name]))
        .collect();
    let path = header
        .lines()
        .next()
        .unwrap()
        .split_whitespace()
        .nth(1)
        .unwrap();
    assert_eq!(path, expected_path);
    // Fixtures contain only unreserved/encoded labels and '/', so double-encoding '%' is sufficient.
    let canonical_path = path.replace('%', "%25");
    let body_hash = hex::encode(Sha256::digest(&captured[end + 4..]));
    let canonical = format!("POST\n{canonical_path}\n\n{canonical_headers}\n{signed}\n{body_hash}");
    let to_sign = format!(
        "AWS4-HMAC-SHA256\n{}\n{scope}\n{}",
        headers["x-amz-date"],
        hex::encode(Sha256::digest(canonical.as_bytes()))
    );
    let parts: Vec<_> = scope.split('/').collect();
    assert_eq!(&parts[1..], &["us-east-1", "bedrock", "aws4_request"]);
    let key = hmac(b"AWS4secret-fixture", parts[0]);
    let key = hmac(&key, parts[1]);
    let key = hmac(&key, parts[2]);
    let key = hmac(&key, parts[3]);
    assert_eq!(fields["Signature"], hex::encode(hmac(&key, &to_sign)));
}
