use super::{fixture::*, models::read_at, McpError, ModelQuery, VERSION};
use serde_json::{json, Value};
use std::time::Duration;

fn init() -> Reply {
    let mut reply = Reply::json(json!({"jsonrpc":"2.0","id":1,"result":{
        "protocolVersion":VERSION,"capabilities":{"tools":{}}}}));
    reply.session = Some("models-session");
    reply
}
fn models() -> Value {
    json!({"source":"registry","models":[{"id":"sample","modality":"image","aliases":[],
        "actions":[],"capabilities":{},"pricing":null,"strengths":null}]})
}
async fn read(peer: &Peer, query: &ModelQuery) -> Result<Value, McpError> {
    read_at(
        &http(),
        &peer.endpoint,
        "synthetic-access",
        None,
        query,
        Duration::from_secs(3),
    )
    .await
}
async fn peer(result: Value) -> Peer {
    Peer::new(vec![
        init(),
        Reply::initialized(),
        Reply::json(json!({"jsonrpc":"2.0","id":2,"result":result})),
    ])
    .await
}

#[tokio::test]
async fn luma_models_list_calls_only_fixed_read_tool_and_closes_session() {
    let peer = peer(json!({"structuredContent":models(),"content":[],"isError":false})).await;
    assert_eq!(read(&peer, &ModelQuery::List).await.unwrap(), models());
    {
        let calls = peer.calls.lock().unwrap();
        assert_eq!(calls.len(), 3);
        assert_eq!(
            calls[2].1["params"],
            json!({"name":"luma_model_list","arguments":{}})
        );
        assert_eq!(calls[2].0["authorization"], "Bearer synthetic-access");
        assert!(!calls[2].0.contains_key("cookie"));
        assert_eq!(calls[2].0["mcp-session-id"], "models-session");
    }
    assert_eq!(peer.deletes.lock().unwrap().len(), 1);
    peer.stop().await;
}

#[tokio::test]
async fn luma_models_describe_accepts_single_json_text_and_binds_model_and_kind() {
    let query = ModelQuery::Describe {
        model: "sample".into(),
        kind: "image".into(),
    };
    let payload = json!({"model":"sample","type":"image","parameters":{"type":"object"},"media_roles":[],"guidance":null});
    let peer = peer(json!({"content":[{"type":"text","text":payload.to_string()}]})).await;
    assert_eq!(read(&peer, &query).await.unwrap(), payload);
    assert_eq!(
        peer.calls.lock().unwrap()[2].1["params"],
        json!({"name":"luma_model_describe","arguments":{"model":"sample","type":"image"}})
    );
    peer.stop().await;
}

#[tokio::test]
async fn luma_models_errors_are_classified_never_retried_and_still_cleaned() {
    for (result, error) in [
        (
            json!({"isError":true,"content":[{"type":"text","text":"private"}]}),
            McpError::Rejected,
        ),
        (
            json!({"isError":"false","structuredContent":models()}),
            McpError::InvalidResponse,
        ),
        (
            json!({"structuredContent":[],"content":[]}),
            McpError::InvalidResponse,
        ),
        (
            json!({"content":[{"type":"resource","resource":{"uri":"https://untrusted"}}]}),
            McpError::InvalidResponse,
        ),
        (
            json!({"content":[{"type":"text","text":"private"}]}),
            McpError::InvalidResponse,
        ),
    ] {
        let peer = peer(result).await;
        assert_eq!(read(&peer, &ModelQuery::List).await.unwrap_err(), error);
        assert_eq!(peer.count(), 3);
        assert_eq!(peer.deletes.lock().unwrap().len(), 1);
        peer.stop().await;
    }
}

#[tokio::test]
async fn luma_models_reject_duplicate_rows_and_wrong_describe_binding() {
    let mut duplicate = models();
    duplicate["models"]
        .as_array_mut()
        .unwrap()
        .push(models()["models"][0].clone());
    let peer = peer(json!({"structuredContent":duplicate})).await;
    assert_eq!(
        read(&peer, &ModelQuery::List).await.unwrap_err(),
        McpError::InvalidResponse
    );
    peer.stop().await;
    let peer = self::peer(json!({"structuredContent":{"model":"other","type":"image","parameters":{},"media_roles":[]}})).await;
    assert_eq!(
        read(
            &peer,
            &ModelQuery::Describe {
                model: "sample".into(),
                kind: "image".into()
            }
        )
        .await
        .unwrap_err(),
        McpError::InvalidResponse
    );
    peer.stop().await;
}

#[tokio::test]
async fn luma_models_invalid_query_never_sends_or_opens_a_session() {
    let peer = Peer::new(vec![]).await;
    for (model, kind) in [("", "image"), ("sample", "audio"), ("bad\nmodel", "video")] {
        assert_eq!(
            read(
                &peer,
                &ModelQuery::Describe {
                    model: model.into(),
                    kind: kind.into()
                }
            )
            .await
            .unwrap_err(),
            McpError::InvalidResponse
        );
    }
    assert_eq!(peer.count(), 0);
    peer.stop().await;
}
