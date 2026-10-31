use super::*;
use crate::protocol::openai::normalize_chat_completions;
use serde_json::json;

fn request(part: Value) -> CanonicalRelayRequest {
    normalize_chat_completions(json!({"model":"gemini-3-flash-preview","messages":[{
        "role":"user","content":[{"type":"text","text":"Describe this."},part]
    }]}))
    .unwrap()
}

#[test]
fn inline_images_and_audio_keep_exact_bytes_without_paths_or_reencoding() {
    for (part, mime, name) in [
        (
            json!({"type":"image_url","image_url":{"url":"data:image/png;base64,AAEC"}}),
            "image/png",
            "input-1.png",
        ),
        (
            json!({"type":"input_audio","input_audio":{"format":"mp3","data":"AAEC"}}),
            "audio/mpeg",
            "input-1.mp3",
        ),
        (
            json!({"type":"input_audio","input_audio":{"format":"wav","data":"AAEC"}}),
            "audio/wav",
            "input-1.wav",
        ),
    ] {
        let req = request(part);
        let (sanitized, uploads) = prepare_media(&req).unwrap();
        assert_eq!(uploads.len(), 1);
        assert_eq!(uploads[0].bytes, [0, 1, 2]);
        assert_eq!(uploads[0].mime_type, mime);
        assert_eq!(uploads[0].file_name, name);
        assert_eq!(sanitized.messages[0].content.len(), 1);
        assert_eq!(req.messages[0].content.len(), 2);
    }
}

#[test]
fn native_inline_video_and_audio_are_accepted_without_metadata_loss() {
    for (key, mime_key, mime) in [
        ("inlineData", "mimeType", "video/mp4"),
        ("inline_data", "mime_type", "audio/flac"),
    ] {
        let mut req = request(json!({"type":"text","text":"unused"}));
        req.messages[0].content.push(ContentPart::Raw {
            value: json!({key:{mime_key:mime,"data":"AAEC"}}),
        });
        let (_, files) = prepare_media(&req).unwrap();
        assert_eq!(files[0].mime_type, mime);
        assert_eq!(files[0].bytes, [0, 1, 2]);
    }
}

#[test]
fn rejects_remote_empty_invalid_or_unrepresentable_inputs() {
    for part in [
        json!({"type":"image_url","image_url":{"url":"https://example.invalid/secret.png"}}),
        json!({"type":"image_url","image_url":{"url":"file:///C:/secret.png"}}),
        json!({"type":"image_url","image_url":{"url":"data:audio/wav;base64,AAEC"}}),
        json!({"type":"input_audio","input_audio":{"format":"mp3","data":""}}),
        json!({"type":"input_audio","input_audio":{"format":"pcm16","data":"AAEC"}}),
        json!({"type":"input_audio","input_audio":{"format":"wav","data":"%%%"}}),
    ] {
        assert!(prepare_media(&request(part)).is_err());
    }
    for value in [
        json!({"inlineData":{"mimeType":"video/mp4","data":"AAEC"},"videoMetadata":{"fps":1}}),
        json!({"inlineData":{"mimeType":"audio/wav","mime_type":"audio/wav","data":"AAEC"}}),
        json!({"inlineData":{"mimeType":"application/pdf","data":"AAEC"}}),
        json!({"fileData":{"mimeType":"video/mp4","fileUri":"https://example.invalid/x"}}),
    ] {
        let mut req = request(json!({"type":"text","text":"unused"}));
        req.messages[0].content.push(ContentPart::Raw { value });
        assert!(prepare_media(&req).is_err());
    }
}

#[test]
fn bounds_total_encoded_bytes_count_and_media_role_before_upload() {
    let part = ContentPart::Raw {
        value: json!({"input_audio":{"format":"mp3","data":"AAEC"},"type":"input_audio"}),
    };
    let mut req = request(json!({"type":"text","text":"unused"}));
    req.messages[0]
        .content
        .extend(std::iter::repeat_n(part.clone(), 16));
    assert_eq!(prepare_media(&req).unwrap().1.len(), 16);
    req.messages[0].content.push(part.clone());
    assert!(prepare_media(&req).is_err());
    req.messages[0].content = vec![part];
    req.messages[0].role = MessageRole::Assistant;
    assert!(prepare_media(&req).is_err());
    req.messages[0].role = MessageRole::User;
    req.messages[0].content = vec![ContentPart::Raw {
        value: json!({"type":"input_audio","input_audio":{"format":"wav","data":"A".repeat(MAX_WEB_INLINE_BYTES + 4)}}),
    }];
    assert!(prepare_media(&req).is_err());
}

#[test]
fn document_inputs_web_prepare_pdf_and_text_without_trusting_caller_filename() {
    let pdf = base64::engine::general_purpose::STANDARD.encode(b"%PDF-1.4\n%%EOF\n");
    for (part, mime, extension, bytes) in [
        (
            json!({"type":"file","file":{"filename":"../../private.pdf","file_data":format!("data:application/pdf;base64,{pdf}")}}),
            "application/pdf",
            "pdf",
            b"%PDF-1.4\n%%EOF\n".to_vec(),
        ),
        (
            json!({"type":"input_file","filename":"C:/private.txt","file_data":"data:text/plain;base64,aGVsbG8="}),
            "text/plain",
            "txt",
            b"hello".to_vec(),
        ),
        (
            json!({"inline_data":{"mime_type":"application/pdf","data":pdf}}),
            "application/pdf",
            "pdf",
            b"%PDF-1.4\n%%EOF\n".to_vec(),
        ),
    ] {
        let (sanitized, files) = prepare_media(&request(part)).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].mime_type, mime);
        assert_eq!(files[0].file_name, format!("input-1.{extension}"));
        assert_eq!(files[0].bytes, bytes);
        assert_eq!(sanitized.messages[0].content.len(), 1);
    }
}

#[test]
fn document_inputs_web_reject_remote_ids_malformed_pdf_and_non_utf8_text() {
    for value in [
        json!({"type":"file","file":{"file_id":"file-private"}}),
        json!({"type":"input_file","file_url":"https://private.invalid/a.pdf"}),
        json!({"inlineData":{"mimeType":"application/pdf","data":"AAEC"}}),
        json!({"inlineData":{"mimeType":"text/plain","data":"/w=="}}),
    ] {
        assert!(prepare_media(&request(value)).is_err());
    }
}
