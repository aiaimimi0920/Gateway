//! Fixed independent AES-GCM vector for the pre-extraction access-key format.
use super::*;

#[tokio::test]
async fn access_key_legacy_seal_vector_and_limits_remain_compatible() {
    let root = std::env::temp_dir().join(format!(
        "gateway-local-seal-vector-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(
        root.join("access-key-seal.json"),
        b"\"ERERERERERERERERERERERERERERERERERERERERERE=\"",
    )
    .unwrap();
    let store = KeySecretStore(LocalSecretSeal::open(&root, true));
    let hash = "0780669c3da4a43ba9ab1c4ad938dcdd187615526694b8d4a17fda02812bfadd";
    let sealed = "IiIiIiIiIiIiIiIiZJwqLrfi8zqCXr1FZcCGp3Lyuc8M6NQ/Jz1vQpQLhZosHUS3LvZclDwA+Q==";
    assert_eq!(
        store.open_token("legacy-id", hash, sealed).unwrap(),
        "sk-gw-legacy-format-fixture"
    );
    for (id, token_hash, payload) in [
        ("other", hash, sealed),
        ("legacy-id", "wrong", sealed),
        ("legacy-id", hash, "bad"),
    ] {
        assert_eq!(
            store
                .open_token(id, token_hash, payload)
                .unwrap_err()
                .code
                .as_deref(),
            Some("access_key_secret_unavailable")
        );
    }
    assert!(store
        .open_token("legacy-id", hash, &"x".repeat(1025))
        .is_err());
    crate::local_runtime::test_support::remove_test_root(&root).await;
}
