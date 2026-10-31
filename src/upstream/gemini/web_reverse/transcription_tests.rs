use super::*;
use crate::protocol::{gemini::web_reverse::prepare_media, openai::normalize_audio_transcriptions};

fn input(body: Value) -> CanonicalRelayRequest {
    normalize_audio_transcriptions(body).unwrap()
}

#[test]
fn transcription_converts_original_audio_to_web_attachment_not_canvas() {
    for (declared, name, expected) in [
        ("audio/mpeg", "unsafe/ignored.mp3", "audio/mpeg"),
        ("application/octet-stream", "voice.WAV", "audio/wav"),
        ("audio/flac", "audio.bin", "audio/flac"),
    ] {
        let req = input(
            json!({"model":"gemini-3-flash-preview","file":{"mime_type":declared,"file_name":name,"base64":"AAEC"},"response_format":"text","language":"en","prompt":"reference"}),
        );
        let prepared = request(&req).unwrap();
        assert_eq!(prepared.endpoint_kind, EndpointKind::ChatCompletions);
        let (clean, files) = prepare_media(&prepared).unwrap();
        assert_eq!(files[0].bytes, [0, 1, 2]);
        assert_eq!(files[0].mime_type, expected);
        assert!(!files[0].file_name.contains("unsafe"));
        let ContentPart::Text { text } = &clean.messages[0].content[0] else {
            panic!()
        };
        assert!(text.contains("Return only the transcript"));
        assert!(!text.contains(name));
    }
}

#[test]
fn unsupported_transcription_options_are_rejected_before_exchange() {
    for field in [
        json!({"response_format":"srt"}),
        json!({"response_format":" text"}),
        json!({"response_format":"verbose_json"}),
        json!({"stream":true}),
        json!({"language":"English"}),
        json!({"language":"EN"}),
        json!({"temperature":0.7}),
        json!({"temperature":0}),
        json!({"timestamp_granularities":["word"]}),
        json!({"include":["logprobs"]}),
        json!({"prompt":["not scalar"]}),
        json!({"prompt":"a".repeat(16 * 1024 + 1)}),
    ] {
        let mut raw = json!({"file":{"mime_type":"audio/mpeg","base64":"AAEC"}});
        raw.as_object_mut()
            .unwrap()
            .extend(field.as_object().unwrap().clone());
        let req = input(raw);
        assert!(request(&req).is_err());
    }
}

#[test]
fn optional_null_filename_uses_declared_mime_not_a_local_path() {
    let req = input(json!({"file":{"file_name":null,"mime_type":"audio/mpeg","base64":"AAEC"}}));
    let files = prepare_media(&request(&req).unwrap()).unwrap().1;
    assert_eq!(files[0].file_name, "input-1.mp3");
    assert_eq!(files[0].bytes, [0, 1, 2]);
}

#[test]
fn duplicate_file_non_audio_and_corrupt_data_do_not_become_uploads() {
    for file in [
        json!([{"base64":"AAEC"},{"base64":"AAEC"}]),
        json!({"mime_type":"image/png","base64":"AAEC"}),
        json!({"mime_type":"audio/mpeg","base64":"%%%"}),
        json!({"mime_type":"audio/mpeg","base64":""}),
    ] {
        match normalize_audio_transcriptions(json!({"file":file})) {
            Err(_) => {}
            Ok(req) => assert!(request(&req).and_then(|r| prepare_media(&r)).is_err()),
        }
    }
}
