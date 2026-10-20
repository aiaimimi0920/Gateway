use super::*;
use futures::{stream, StreamExt};
use serde_json::json;

fn meta() -> ModelObservationMeta {
    ModelObservationMeta::new("fixture", Some("alias"), "sent", true)
}

async fn observe(wire: Vec<Bytes>, raw: bool) -> Value {
    let metrics = Arc::new(GatewayMetrics::new());
    let expected = wire.clone();
    let source = stream::iter(wire.into_iter().map(Ok::<_, &'static str>));
    let output = tap_model_declaration(source, meta(), metrics.clone(), raw)
        .collect::<Vec<_>>()
        .await;
    assert_eq!(output, expected.into_iter().map(Ok).collect::<Vec<_>>());
    let rows = serde_json::to_value(metrics.model_observations()).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    rows[0].clone()
}

#[tokio::test]
async fn raw_protocol_envelopes_survive_single_byte_fragmentation() {
    for (wire, source) in [
        ("\u{feff}: comment\r\ndata: {\r\ndata: \"choices\":[],\"model\":\"declared\"}\r\n\r\ndata: [DONE]\n\n", "model"),
        ("event: message_start\ndata: {\"type\":\"message_start\",\"message\":{\"model\":\"declared\"}}\n\n", "message.model"),
        ("event: response.created\ndata: {\"type\":\"response.created\",\"response\":{\"model\":\"declared\"}}\n\n", "response.model"),
    ] {
        let row = observe(wire.as_bytes().iter().map(|b| Bytes::copy_from_slice(&[*b])).collect(), true).await;
        assert_eq!(row["declaredModel"], "declared");
        assert_eq!(row["declarationSource"], source);
        assert_eq!(row["relation"], "declared_differs");
        assert_eq!(row["observationEnd"], "eof");
    }
}

#[tokio::test]
async fn missing_and_synthetic_names_remain_unknown() {
    for (wire, raw) in [
        ("data: {\"choices\":[]}\n\n", true),
        ("data: {\"choices\":[],\"model\":\"alias\"}\n\n", false),
        ("data: {\"choices\":[{\"delta\":{\"model\":\"not-metadata\"}}]}\n\n", true),
        ("data: {\"type\":\"response.output_item.done\",\"item\":{\"model\":\"not-metadata\"}}\n\n", true),
        ("data: {\"choices\":[],\"model\":\"unfinished\"}", true),
    ] {
        let row = observe(vec![Bytes::from(wire)], raw).await;
        assert_eq!(row["declaredModel"], json!(null));
        assert_eq!(row["relation"], "unknown");
    }
}

#[tokio::test]
async fn conflicting_invalid_or_oversized_observations_never_produce_false_evidence() {
    let valid = "data: {\"choices\":[],\"model\":\"sent\"}\n\n";
    for (suffix, reason) in [
        (
            "data: {\"choices\":[],\"model\":\"different\"}\n\n".into(),
            "conflicting_declarations",
        ),
        (
            "data: {\"choices\":[],\"model\":\"private prompt\"}\n\n".into(),
            "invalid_declaration",
        ),
        ("data: not-json\n\n".into(), "invalid_envelope"),
        (
            format!("data: {}\n\n", "a".repeat(MAX_FRAME_BYTES)),
            "observation_limit",
        ),
        ("a".repeat(MAX_OBSERVED_BYTES + 1), "observation_limit"),
    ] {
        let row = observe(
            vec![Bytes::from(valid), Bytes::from(suffix), Bytes::from(valid)],
            true,
        )
        .await;
        assert_eq!(row["declaredModel"], json!(null));
        assert_eq!(row["relation"], "unknown");
        assert_eq!(row["observationStatus"], reason);
    }
}

#[tokio::test]
async fn errors_and_cancellation_publish_once_without_altering_stream_errors() {
    for cancelled in [false, true] {
        let metrics = Arc::new(GatewayMetrics::new());
        let first = Bytes::from_static(b"data: {\"choices\":[],\"model\":\"sent\"}\n\n");
        let source = stream::iter(vec![Ok(first.clone()), Err("original transport failure")]);
        let mut tapped = Box::pin(tap_model_declaration(source, meta(), metrics.clone(), true));
        assert_eq!(tapped.next().await, Some(Ok(first)));
        if !cancelled {
            assert_eq!(tapped.next().await, Some(Err("original transport failure")));
            assert_eq!(tapped.next().await, None);
        }
        drop(tapped);
        let rows = serde_json::to_value(metrics.model_observations()).unwrap();
        assert_eq!(rows.as_array().unwrap().len(), 1);
        assert_eq!(rows[0]["declaredModel"], "sent");
        assert_eq!(
            rows[0]["observationEnd"],
            if cancelled {
                "source_dropped"
            } else {
                "transport_error"
            }
        );
    }
}

#[test]
fn dropping_an_unpolled_stream_is_unknown_not_a_synthetic_model_claim() {
    let metrics = Arc::new(GatewayMetrics::new());
    let stream = tap_model_declaration(
        stream::pending::<Result<Bytes, ()>>(),
        meta(),
        metrics.clone(),
        true,
    );
    drop(stream);
    let rows = serde_json::to_value(metrics.model_observations()).unwrap();
    assert_eq!(rows[0]["declaredModel"], json!(null));
    assert_eq!(rows[0]["observationEnd"], "source_dropped");
}
