//! Invalid upstream success and truncation cannot settle successfully or replay to a second account.
use super::*;
#[tokio::test]
async fn sap_aicore_bad_json_and_bad_stream_do_not_replay_or_charge() {
    Box::pin(async {
        for endpoint in ENDPOINTS {
            for (stream,raw) in [(false,b"{bad".to_vec()),(false,br#"{"error":{"message":"synthetic-sap-secret"}}"#.to_vec()),
                (true,Vec::new()),(true,b"data: [DONE]\n\n".to_vec()),
                (true,b"data: {\"choices\":[{\"index\":0,\"delta\":{\"tool_calls\":[{\"index\":0}]},\"finish_reason\":\"tool_calls\"}]}\n\ndata: [DONE]\n\n".to_vec()),
                (true,[native::reply("chat",false,true,true),b"data: {bad".to_vec()].concat())] {
                let peer=peer::Peer::raw(raw).await;let fixture=Box::pin(Fixture::with_fallback(&peer,true)).await;
                super::super::test_support::origin(peer.origin.clone(),async {
                    let response=fixture.open(endpoint,body(endpoint,stream)).await;let mut body=response.into_body();let mut bytes=Vec::new();
                    tokio::time::timeout(Duration::from_secs(5),async { while let Some(frame)=body.frame().await {
                        match frame { Ok(frame)=>if let Ok(data)=frame.into_data(){bytes.extend_from_slice(&data)},Err(_)=>break }
                    }}).await.unwrap();drop(body);
                    let text=String::from_utf8(bytes).unwrap();
                    for value in ["synthetic-sap-secret","response.completed","message_stop","[DONE]"]{assert!(!text.contains(value),"{text}");}
                    assert_ne!(fixture.audits(1).await[0].status,"completed");assert_eq!(fixture.remaining().await,2);assert_wire(&peer,1,endpoint);
                    peer.wait_closed().await;
                }).await;fixture.finish().await;peer.stop().await;
            }
        }
    }).await;
}
