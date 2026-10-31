use super::*;
use serde_json::json;

#[test]
fn document_inputs_envelopes_never_resolve_names_urls_or_ids() {
    for value in [
        json!({"type":"file","file":{"filename":"../../not-read.pdf","file_data":"data:application/pdf;base64,JVBERg=="}}),
        json!({"type":"input_file","filename":"C:/not-read.pdf","file_data":"data:application/pdf;base64,JVBERg=="}),
    ] {
        assert_eq!(
            openai_file(&value),
            Ok(Some(("application/pdf", "JVBERg==")))
        );
    }
    for value in [
        json!({"type":"file","file":{"file_id":"file-not-resolved"}}),
        json!({"type":"input_file","file_url":"https://private.invalid/doc.pdf"}),
        json!({"type":"input_file","file_data":"data:text/plain;base64,WA==","file_id":"file-ignored"}),
        json!({"type":"file","file":{"file_data":"data:text/plain;base64,WA==","extra":true}}),
        json!({"type":"file","file":{"file_data":"data:text/plain;base64,WA=="},"inlineData":{}}),
        json!({"type":"input_file","file_data":"data:text/plain;base64,WA==","filename":null}),
        json!({"type":"input_file","file_data":"data:application/zip;base64,WA=="}),
        json!({"type":"input_file","file_data":"data:text/plain;charset=utf-8;base64,WA=="}),
        json!({"type":"input_file","file_data":"data:text/plain;base64,"}),
        json!({"type":"input_file","file_data":"WA=="}),
    ] {
        assert!(openai_file(&value).is_err(), "{value:?}");
    }
    assert_eq!(openai_file(&json!({"text":"unchanged"})), Ok(None));
}

#[test]
fn document_inputs_minimum_envelope_checks_do_not_claim_full_pdf_decoding() {
    assert!(valid_bytes("application/pdf", b"%PDF-1.4\n%%EOF\n"));
    assert!(!valid_bytes("application/pdf", b"<html>not a PDF</html>"));
    assert!(!valid_bytes("application/pdf", b"%PDF-1.4\ntruncated"));
    assert!(!valid_bytes("application/pdf", b""));
    for mime in [
        "text/plain",
        "text/markdown",
        "text/csv",
        "text/html",
        "text/xml",
        "application/xml",
        "application/json",
    ] {
        assert!(valid_bytes(mime, "合成文件正文".as_bytes()));
        assert!(!valid_bytes(mime, b"\xff"));
        assert!(!valid_bytes(mime, b"text\0binary"));
        assert!(!valid_bytes(mime, b""));
    }
    assert!(!valid_bytes("application/octet-stream", b"binary"));
}
