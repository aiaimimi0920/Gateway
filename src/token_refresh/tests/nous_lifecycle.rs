//! End-to-end local refresh must use the dedicated single-use protocol and the durable receipt owner.
use super::{nous_fixture::*, nous_scheduler_fixture::*, *};

#[tokio::test]
async fn nous_scheduler_preserves_receipt_before_publishing_the_exact_successor() {
    let f = fixture().await;
    let original = request(&f);
    let mock = Mock::new(vec![(200, reply())], 0).await;
    force_due(&f);
    let mut pending = Pending::new();
    refresh(&f, &mock, &mut pending).await;
    assert_eq!(mock.count(), 1);
    assert!(pending.is_empty());
    let published = load(&f, &original);
    assert_eq!(
        published.phase(),
        crate::console::refresh_grants::GrantPhase::Published
    );
    assert!(published.receipt().is_some());
    let saved = f.saved();
    assert_eq!(saved.refresh_token.as_deref(), Some("nous-new-refresh"));
    assert_eq!(saved.expires_at.as_deref(), Some("2100-01-01T00:01:40Z"));
    assert_eq!(
        saved.nous_portal.unwrap().inference_target(),
        crate::protocol::nous_portal::InferenceTarget::Welcome
    );
    let requests = mock.requests();
    assert_eq!(requests[0].0["x-nous-refresh-token"], "nous-old-refresh");
    assert!(!requests[0].1.contains("nous-old-refresh"));
    assert!(!requests[0].0.contains_key("authorization"));
    mock.stop().await;
}
