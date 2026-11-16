//! Product-level invalid native Messages responses cannot be promoted into billable success.
use super::super::test_support::{Peer, Reply};
use super::*;
#[tokio::test]
async fn lobsterai_virtual_invalid_messages_refund_without_replaying_inference() {
    Box::pin(async {
        let base=json!({"type":"message","role":"assistant","model":UPSTREAM_MODEL,"content":[],"stop_reason":"tool_use","usage":{"input_tokens":2,"output_tokens":3}});
        for stream in [false,true] {for invalid in [base.clone(),{
            let mut v=base.clone();v["stop_reason"]=json!("end_turn");v["usage"]["cache_read_input_tokens"]=json!(-1);v
        }] {
            let peer=Peer::start(vec![virtual_models::resolution("anthropic"),Reply::json(invalid)]).await;
            let fixture=Box::pin(virtual_models::fixture(&peer,"yd-fast")).await;
            super::super::transport::TEST_ORIGIN.scope(peer.origin.clone(),async {
                let response=fixture.open("chat/completions",body("chat/completions",stream)).await;
                assert_ne!(response.status(),StatusCode::OK);
                drain(response.into_body()).await;
            }).await;
            assert_ne!(fixture.audits(1).await[0].status,"completed");
            assert_eq!(fixture.remaining().await,2);
            fixture.finish().await;
            peer.inspect(|calls|assert_eq!(calls.len(),2));
        }}
    }).await;
}
