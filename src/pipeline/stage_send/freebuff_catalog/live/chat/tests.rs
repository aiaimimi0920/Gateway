//! Projection measures serialized cumulative bytes, never native finish/DONE as public success.
use super::*;
#[test]
fn freebuff_chat_projection_keeps_native_extensions_and_delays_success_without_duplicate_content() {
    let mut projection = Projection::new();
    let frame = json!({"id":"native","model":"native-model","choices":[{"index":0,"delta":{"role":"assistant","content":"中\n\"","reasoning_content":"native","reasoning_details":[{"opaque":1}]},"finish_reason":"stop"}],"usage":{"cost":0.125}});
    let first = projection.frame(frame.clone()).unwrap();
    let first = std::str::from_utf8(&first).unwrap();
    assert!(first.contains("reasoning_details"));
    assert!(first.contains("\"cost\":0.125"));
    assert!(!first.contains("\"finish_reason\":\"stop\""));
    let completion = json!({"id":"native","model":"native-model","choices":[{"index":0,"message":frame["choices"][0]["delta"],"finish_reason":"stop"}]});
    let tail = projection.finish(completion).unwrap();
    let tail = std::str::from_utf8(&tail).unwrap();
    assert!(tail.contains("[DONE]"));
    assert!(tail.contains("\"finish_reason\":\"stop\""));
    assert!(tail.contains("\"delta\":{}"));
    assert!(!tail.contains("reasoning_details"));
}
#[test]
fn freebuff_chat_projection_counts_all_frames_and_late_terminal_framing() {
    let frame = json!({"choices":[{"delta":{"content":"\n\"中"},"finish_reason":null}]});
    let mut measure = Projection::new();
    let size = measure.frame(frame.clone()).unwrap().len();
    let mut projection = Projection {
        observed: false,
        remaining: size * 2 - 1,
    };
    assert!(projection.frame(frame.clone()).is_ok());
    assert!(matches!(
        projection.frame(frame),
        Err(BusinessError::ResponseTooLarge)
    ));
    let completion = json!({"choices":[{"index":0,"message":{"role":"assistant","content":""},"finish_reason":"stop"}]});
    let size = Projection::new().finish(completion.clone()).unwrap().len();
    let mut projection = Projection {
        observed: false,
        remaining: size,
    };
    assert!(projection.finish(completion.clone()).is_ok());
    assert_eq!(projection.remaining, 0);
    let mut projection = Projection {
        observed: false,
        remaining: size - 1,
    };
    assert!(projection.finish(completion).is_err());
}
