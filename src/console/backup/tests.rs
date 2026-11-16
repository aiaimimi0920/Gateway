use super::{decrypt, encrypt};

#[test]
fn encrypted_backup_roundtrip_preserves_credentials_and_randomizes_envelope() {
    let input = br#"{"credential":"not-a-real-secret","id":"stable-id"}"#;
    let first = encrypt(input, "test-password-123").unwrap();
    let second = encrypt(input, "test-password-123").unwrap();
    assert_ne!(first, second);
    assert!(!first.windows(17).any(|part| part == b"not-a-real-secret"));
    assert_eq!(decrypt(&first, "test-password-123").unwrap(), input);
}

#[test]
fn encrypted_backup_rejects_wrong_password_tampering_and_unknown_version() {
    let envelope = encrypt(b"fixture", "test-password-123").unwrap();
    assert!(decrypt(&envelope, "wrong-password-123").is_err());
    let mut corrupt = envelope.clone();
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(decrypt(&corrupt, "test-password-123").is_err());
    corrupt[7] = 2;
    assert!(decrypt(&corrupt, "test-password-123").is_err());
    assert!(decrypt(b"short", "test-password-123").is_err());
    assert!(encrypt(b"fixture", "short").is_err());
}
