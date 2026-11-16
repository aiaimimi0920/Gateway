//! Dropping a real public body cancels the source and refunds the local reservation.
use super::*;
#[tokio::test]
async fn lobsterai_caller_cancel_closes_source_and_refunds() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            let peer = peer::Peer::new("chat", false, true, 200).await;
            let fixture = Box::pin(Fixture::new(&peer)).await;
            super::super::transport::TEST_ORIGIN
                .scope(peer.origin.clone(), async {
                    let response = fixture.open(endpoint, body(endpoint, true)).await;
                    assert_eq!(response.status(), StatusCode::OK);
                    let mut body = response.into_body();
                    assert!(body.frame().await.unwrap().is_ok());
                    drop(body);
                    assert_ne!(fixture.audits(1).await[0].status, "completed");
                    assert_eq!(fixture.remaining().await, 2);
                    peer.wait_closed().await;
                    assert_wire(&peer, 1, endpoint);
                })
                .await;
            fixture.finish().await;
            peer.stop().await;
        }
    })
    .await;
}
