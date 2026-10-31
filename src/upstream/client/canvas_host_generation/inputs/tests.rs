use super::*;
use crate::protocol::canonical::ContentPart;

fn request(parts: Value) -> CanonicalRelayRequest {
    crate::protocol::openai::normalize_chat_completions(json!({
        "model":"gemini-3-flash-preview", "messages":[{"role":"user", "content":parts}]
    }))
    .unwrap()
}

#[test]
fn host_input_audio_is_inline_not_an_openai_shape_forwarded_to_gemini() {
    for (format, mime) in [("wav", "audio/wav"), ("mp3", "audio/mpeg")] {
        let req = request(json!([
            {"type":"text", "text":"Transcribe the audio."},
            {"type":"input_audio", "input_audio":{"format":format, "data":"AAEC"}}
        ]));
        let body = text_request(&req, "gemini-3-flash-preview").unwrap();
        assert_eq!(
            body["contents"][0]["parts"][1],
            json!({"inlineData":{"mimeType":mime,"data":"AAEC"}})
        );
        assert_eq!(
            body["contents"][0]["parts"][0]["text"],
            "Transcribe the audio."
        );
    }
}

#[test]
fn host_data_uri_images_preserve_original_bytes_without_upload_or_remote_fetch() {
    for mime in ["image/png", "image/jpeg", "image/webp"] {
        let req = request(json!([
            {"type":"text", "text":"Name this shape."},
            {"type":"image_url", "image_url":{"url":format!("data:{mime};base64,AAEC")}}
        ]));
        let body = text_request(&req, "gemini-3-flash-preview").unwrap();
        assert_eq!(
            body["contents"][0]["parts"][1],
            json!({"inlineData":{"mimeType":mime,"data":"AAEC"}})
        );
    }
}

#[test]
fn host_native_inline_media_has_one_canonical_shape_and_keeps_metadata() {
    for mime in ["image/png", "audio/wav", "video/mp4"] {
        let mut req = request(json!([{"type":"text","text":"Describe the media."}]));
        req.messages[0].content.push(ContentPart::Raw {
            value: json!({
                "inline_data":{"mime_type":mime,"data":"AAEC"}, "videoMetadata":{"fps":1}
            }),
        });
        let body = text_request(&req, "gemini-3-flash-preview").unwrap();
        let part = &body["contents"][0]["parts"][1];
        assert_eq!(part["inlineData"], json!({"mimeType":mime,"data":"AAEC"}));
        assert!(part.get("inline_data").is_none());
        assert_eq!(part["videoMetadata"]["fps"], 1);
    }
}

#[test]
fn host_malformed_unsupported_or_remote_media_fails_before_generation() {
    for value in [
        json!({"type":"input_audio","input_audio":{"format":"pcm16","data":"AAEC"}}),
        json!({"type":"input_audio","input_audio":{"format":"wav","data":"%%%"}}),
        json!({"type":"input_audio","input_audio":{"format":"wav","data":""}}),
        json!({"type":"image_url","image_url":{"url":"https://private.invalid/image.png"}}),
        json!({"type":"image_url","image_url":{"url":"data:text/html;base64,AAEC"}}),
        json!({"inlineData":{"mimeType":"audio/wav","data":"%%%"}}),
        json!({"inlineData":{"mimeType":"application/octet-stream","data":"AAEC"}}),
        json!({"inlineData":{"mimeType":"audio/wav","data":"AAEC"},"inline_data":{}}),
        json!({"fileData":{"mimeType":"audio/wav","fileUri":"https://private.invalid/a.wav"}}),
    ] {
        let mut req = request(json!([{"type":"text","text":"Describe."}]));
        req.messages[0].content.push(ContentPart::Raw { value });
        assert!(text_request(&req, "gemini-3-flash-preview").is_err());
    }
}

#[test]
fn host_inline_media_count_and_aggregate_encoded_bytes_are_bounded() {
    let mut req = request(json!([{"type":"text","text":"Describe."}]));
    for _ in 0..17 {
        req.messages[0].content.push(ContentPart::Raw {
            value: json!({"inlineData":{"mimeType":"audio/wav","data":"AAEC"}}),
        });
    }
    assert!(text_request(&req, "gemini-3-flash-preview").is_err());
    req.messages[0].content.truncate(1);
    req.messages[0].content.push(ContentPart::Raw {
        value: json!({
            "inlineData":{"mimeType":"audio/wav", "data":"A".repeat(MAX_INLINE_BYTES + 4)}
        }),
    });
    assert!(text_request(&req, "gemini-3-flash-preview").is_err());
    req.messages[0].content.truncate(1);
    for _ in 0..2 {
        req.messages[0].content.push(ContentPart::Raw {
            value: json!({
                "inlineData":{"mimeType":"audio/wav", "data":"A".repeat(MAX_INLINE_BYTES / 2 + 4)}
            }),
        });
    }
    assert!(text_request(&req, "gemini-3-flash-preview").is_err());
}

#[test]
fn document_inputs_canvas_convert_openai_files_and_native_pdf_without_reading_names() {
    let data = base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.4\n%%EOF\n");
    for value in [
        json!({"type":"file","file":{"filename":"../../not-read.pdf","file_data":format!("data:application/pdf;base64,{data}")}}),
        json!({"type":"input_file","filename":"C:/not-read.pdf","file_data":format!("data:application/pdf;base64,{data}")}),
        json!({"inline_data":{"mime_type":"application/pdf","data":data}}),
    ] {
        let req = request(json!([{"type":"text","text":"Summarize."},value]));
        let body = text_request(&req, "gemini-3-flash-preview").unwrap();
        assert_eq!(
            body["contents"][0]["parts"][1],
            json!({"inlineData":{"mimeType":"application/pdf","data":data}})
        );
    }
}

#[test]
fn document_inputs_canvas_reject_remote_ids_ambiguous_and_invalid_documents() {
    for value in [
        json!({"type":"file","file":{"file_id":"file-private"}}),
        json!({"type":"input_file","file_url":"https://private.invalid/a.pdf"}),
        json!({"type":"input_file","file_data":"data:application/pdf;base64,AAEC","file_id":"file-private"}),
        json!({"inlineData":{"mimeType":"application/pdf","data":"AAEC"}}),
        json!({"inlineData":{"mimeType":"text/plain","data":"/w=="}}),
    ] {
        let req = request(json!([{"type":"text","text":"Summarize."},value]));
        assert!(text_request(&req, "gemini-3-flash-preview").is_err());
    }
}
