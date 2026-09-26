use super::response_value::extract_choice_text;
use crate::protocol::sse_parse::{parse_sse_line, SseParseState};
use bytes::Bytes;
use futures::Stream;
use serde_json::{json, Value};
use std::time::{SystemTime, UNIX_EPOCH};

pub fn translate_qwen_web_frame_to_openai_sse(
    data_str: &str,
    model: &str,
    response_id: &str,
    created: i64,
) -> Option<Vec<u8>> {
    if data_str.trim().is_empty() {
        return None;
    }
    if data_str.trim() == "[DONE]" {
        return Some(b"data: [DONE]\n\n".to_vec());
    }
    let data: Value = serde_json::from_str(data_str).ok()?;
    let choice = data
        .get("choices")
        .and_then(|value| value.as_array())
        .and_then(|choices| choices.first())?;
    let content = extract_choice_text(choice);
    let finish_reason = choice
        .get("finish_reason")
        .and_then(|value| value.as_str())
        .map(str::to_string);

    if content.is_none() && finish_reason.is_none() {
        return None;
    }

    let chunk = json!({
        "id": response_id,
        "object": "chat.completion.chunk",
        "created": created,
        "model": data.get("model").and_then(|value| value.as_str()).unwrap_or(model),
        "choices": [{
            "index": 0,
            "delta": content.map(|value| json!({ "content": value })).unwrap_or_else(|| json!({})),
            "finish_reason": finish_reason,
        }]
    });

    Some(format!("data: {chunk}\n\n").into_bytes())
}

pub fn translate_qwen_web_stream(
    inner: impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static,
    model: String,
) -> impl Stream<Item = Result<Bytes, rquest::Error>> + Send + 'static {
    let response_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
    let created = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64;
    let state = QwenWebTranslatorState {
        buffer: Vec::new(),
        parser: SseParseState::new(),
        model,
        response_id,
        created,
        emitted_done: false,
    };

    futures::stream::unfold(
        (
            Box::pin(inner)
                as std::pin::Pin<Box<dyn Stream<Item = Result<Bytes, rquest::Error>> + Send>>,
            state,
            false,
        ),
        |(mut stream, mut st, done)| async move {
            use futures::StreamExt;

            if done {
                return None;
            }

            loop {
                if let Some(pos) = st.buffer.iter().position(|&b| b == b'\n') {
                    let line: Vec<u8> = st.buffer.drain(..=pos).collect();
                    let line_str = match std::str::from_utf8(&line) {
                        Ok(value) => value,
                        Err(_) => continue,
                    };
                    let normalized_line = line_str.trim_end_matches('\n');
                    if let Some(frame) = parse_sse_line(normalized_line, &mut st.parser) {
                        if let Some(translated) = translate_qwen_web_frame_to_openai_sse(
                            &frame.data,
                            &st.model,
                            &st.response_id,
                            st.created,
                        ) {
                            if translated == b"data: [DONE]\n\n".to_vec() {
                                st.emitted_done = true;
                                return Some((Ok(Bytes::from(translated)), (stream, st, true)));
                            }
                            return Some((Ok(Bytes::from(translated)), (stream, st, false)));
                        }
                    }
                    continue;
                }

                match stream.next().await {
                    Some(Ok(chunk)) => st.buffer.extend_from_slice(&chunk),
                    Some(Err(error)) => return Some((Err(error), (stream, st, true))),
                    None => {
                        if !st.emitted_done {
                            st.emitted_done = true;
                            return Some((
                                Ok(Bytes::from_static(b"data: [DONE]\n\n")),
                                (stream, st, true),
                            ));
                        }
                        return None;
                    }
                }
            }
        },
    )
}

#[derive(Debug)]
struct QwenWebTranslatorState {
    buffer: Vec<u8>,
    parser: SseParseState,
    model: String,
    response_id: String,
    created: i64,
    emitted_done: bool,
}
