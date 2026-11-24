//! Model-only projection for ordinary Chat SSE. Do not impose a native
//! product's finish/usage admission rules on generic compatible peers.
use crate::protocol::{
    stream_decode::{BoundedSseDecoder, DecodeStep, MAX_TRANSLATED_SSE_FRAME_BYTES},
    stream_error::ProtocolStreamError,
};
use bytes::Bytes;
use futures::{stream, Stream, StreamExt};
use serde_json::Value;

pub(super) fn set_model(object: &mut serde_json::Map<String, Value>, model: &str) {
    object.insert("model".into(), Value::String(model.into()));
}

pub(crate) fn project<S, E>(inner: S, model: String) -> impl Stream<Item = Result<Bytes, E>> + Send
where
    S: Stream<Item = Result<Bytes, E>> + Send,
    E: From<ProtocolStreamError> + Send,
{
    let decoder =
        BoundedSseDecoder::new_strict(MAX_TRANSLATED_SSE_FRAME_BYTES).with_passthrough_fields();
    stream::try_unfold(
        (Box::pin(inner), decoder, model, 0),
        |mut state| async move {
            loop {
                match state.1.decode_next() {
                    DecodeStep::Frame(frame) => {
                        let (fields, has_data) = state
                            .1
                            .take_passthrough_fields()
                            .expect("projection decoder preserves framing fields");
                        return Ok(Some((
                            project_frame(fields, has_data, frame.data, &state.2),
                            state,
                        )));
                    }
                    DecodeStep::Error(error) => return Err(error.into()),
                    DecodeStep::NeedInput => {}
                }
                match state.0.next().await {
                    Some(bytes) => state.1.push_chunk(bytes?),
                    None => {
                        state.1.finish_strict()?;
                        return Ok(None);
                    }
                }
                // A ready peer can send arbitrarily many empty or comment-only events.
                state.3 += 1;
                if state.3 == 32 {
                    state.3 = 0;
                    tokio::task::yield_now().await;
                }
            }
        },
    )
}

fn project_frame(mut fields: String, has_data: bool, mut data: String, model: &str) -> Bytes {
    if has_data {
        // Not all compatible peers use JSON for their named auxiliary events.
        // Forward them without changing their message or terminal semantics.
        if let Ok(mut value) = serde_json::from_str::<Value>(&data) {
            if let Some(object) = value.as_object_mut() {
                // Preserve extension/error frames, projecting a reported model if present.
                if object.contains_key("model") || object.contains_key("choices") {
                    set_model(object, model);
                    data = value.to_string();
                }
            }
        }
        for line in data.split('\n') {
            fields.push_str("data: ");
            fields.push_str(line);
            fields.push('\n');
        }
    }
    fields.push('\n');
    Bytes::from(fields)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn mapped_chat_keeps_fragmented_tools_usage_and_terminal() {
        let frames = [
            r#"{"model":"wire","choices":[{"delta":{"tool_calls":[{"function":{"arguments":"{\"model\":\"argument\"}"}}]}}]}"#,
            r#"{"model":"wire","choices":[{"delta":{},"finish_reason":"tool_calls"}]}"#,
            r#"{"choices":[],"usage":{"prompt_tokens":2,"completion_tokens":3,"total_tokens":5}}"#,
        ];
        let input = format!(
            ": heartbeat\r\n\r\nid: 4\r\ndata:{}\r\n\r\ndata: {}\n\ndata: {}\n\nevent: ping\nretry: 500\ndata: still here\n\ndata: [DONE]\n\n",
            frames[0], frames[1], frames[2]
        );
        let chunks = input
            .into_bytes()
            .chunks(7)
            .map(Bytes::copy_from_slice)
            .map(Ok::<_, ProtocolStreamError>)
            .collect::<Vec<_>>();
        let output = project(stream::iter(chunks), "alias".into())
            .collect::<Vec<_>>()
            .await;
        assert_eq!(output.len(), 6);
        assert_eq!(output[0].as_ref().unwrap().as_ref(), b": heartbeat\n\n");
        assert!(output[1].as_ref().unwrap().starts_with(b"id: 4\n"));
        for (wire, public) in frames.into_iter().zip(&output[1..4]) {
            let mut expected: Value = serde_json::from_str(wire).unwrap();
            expected["model"] = "alias".into();
            let public = std::str::from_utf8(public.as_ref().unwrap()).unwrap();
            assert_eq!(
                serde_json::from_str::<Value>(
                    public
                        .lines()
                        .find_map(|line| line.strip_prefix("data: "))
                        .unwrap()
                )
                .unwrap(),
                expected
            );
        }
        assert_eq!(
            output[4].as_ref().unwrap().as_ref(),
            b"event: ping\nretry: 500\ndata: still here\n\n"
        );
        assert_eq!(output[5].as_ref().unwrap().as_ref(), b"data: [DONE]\n\n");
    }
}
