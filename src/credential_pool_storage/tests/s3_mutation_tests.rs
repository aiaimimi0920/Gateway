//! Mutation capabilities fail closed before unknown endpoints can ignore preconditions.
use super::*;

#[tokio::test]
async fn unknown_s3_endpoint_never_sends_create_or_delete_even_if_server_ignores_conditions() {
    let fixture = fixture().await;
    fixture
        .state
        .ignore_conditions
        .store(true, Ordering::SeqCst);
    *fixture.state.body.lock().unwrap() = b"existing archive".to_vec();
    // Deliberately do not call the explicit test-only capability opt-in.
    let storage = S3Storage::new(&fixture.connection, NAMESPACE).unwrap();
    assert!(storage.put("account.json", b"replacement").await.is_err());
    assert!(storage
        .delete_verified("account.json", b"existing archive", "\"original\"", &|| {
            true
        })
        .await
        .is_err());
    assert_eq!(*fixture.state.body.lock().unwrap(), b"existing archive");
    assert!(fixture.state.requests.lock().unwrap().is_empty());
}

#[tokio::test]
async fn conditional_create_preserves_a_writer_that_wins_after_our_missing_read() {
    let fixture = fixture().await;
    let storage = fixture.storage();
    assert!(storage
        .get("race.json")
        .await
        .unwrap_err()
        .is::<StorageNotFound>());
    assert!(storage.put("race.json", b"losing archive").await.is_err());
    assert_eq!(
        storage.get("race.json").await.unwrap(),
        b"concurrent winner"
    );
    let requests = fixture.state.requests.lock().unwrap();
    let puts: Vec<_> = requests
        .iter()
        .filter(|(method, _, _)| *method == Method::PUT)
        .collect();
    assert_eq!(
        puts.len(),
        1,
        "a conflict must never retry as an unconditional write"
    );
    assert_eq!(puts[0].2["if-none-match"], "*");
    assert!(puts[0].2["authorization"]
        .to_str()
        .unwrap()
        .contains("if-none-match"));
}

#[tokio::test]
async fn conditional_create_does_not_overwrite_an_existing_key() {
    let fixture = fixture().await;
    let storage = fixture.storage();
    storage.put("account.json", b"first archive").await.unwrap();
    assert!(storage.put("account.json", b"replacement").await.is_err());
    assert_eq!(storage.get("account.json").await.unwrap(), b"first archive");
}
