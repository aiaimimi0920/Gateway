use super::*;
use std::path::PathBuf;
use std::sync::{Arc, Barrier};

struct TraceDirectory(PathBuf);

impl TraceDirectory {
    fn new() -> Self {
        let root = std::env::temp_dir().canonicalize().unwrap();
        let path = root.join(format!("gateway-trace-test-{}", uuid::Uuid::new_v4()));
        assert_eq!(path.parent(), Some(root.as_path()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn log(&self) -> PathBuf {
        self.0.join("trace.log")
    }
}

impl Drop for TraceDirectory {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn trace_redacts_fields_and_preserves_one_bounded_tsv_record() {
    let dir = TraceDirectory::new();
    let secret = "token=fixture-token\nAuthorization: Bearer fixture-bearer\r\n";
    let stage = format!("{secret}{}", "s".repeat(1024));
    assert!(append_trace_entry_at(&dir.log(), &stage, secret, 42, MAX_TRACE_BYTES).unwrap());
    let text = std::fs::read_to_string(dir.log()).unwrap();
    assert_eq!(text.lines().count(), 1);
    let fields: Vec<_> = text.trim_end_matches('\n').split('\t').collect();
    assert_eq!(fields.len(), 3);
    assert_eq!(fields[0], "42");
    assert!(fields[1].chars().count() <= 64);
    assert!(fields[2].chars().count() <= 512);
    assert!(!text.contains("fixture-token"));
    assert!(!text.contains("fixture-bearer"));
}

#[test]
fn trace_long_ascii_and_utf8_details_remain_bounded() {
    for unit in ["a", "\u{4f60}"] {
        let dir = TraceDirectory::new();
        let detail = unit.repeat(2048);
        assert!(append_trace_entry_at(&dir.log(), "stage", &detail, 42, MAX_TRACE_BYTES).unwrap());
        let text = std::fs::read_to_string(dir.log()).unwrap();
        let fields: Vec<_> = text.trim_end_matches('\n').split('\t').collect();
        assert_eq!(fields.len(), 3);
        assert_eq!(fields[2].chars().count(), 512);
        assert!(fields[2].ends_with("..."));
    }
}

#[test]
fn trace_metadata_redacts_complete_body_before_preview_cut() {
    use crate::upstream::gemini_canvas_image_edit_local_helpers::build_gemini_canvas_image_edit_stream_response_meta;
    use crate::upstream::gemini_canvas_upload_contract::build_gemini_canvas_image_edit_upload_response_meta;
    let jwt = format!("eyJ{}.{}.signature", "a".repeat(400), "b".repeat(400));
    let stream = build_gemini_canvas_image_edit_stream_response_meta("fixture", None, None, &jwt);
    let upload = [Some(true), Some(false), None].map(|present| {
        build_gemini_canvas_image_edit_upload_response_meta("fixture", None, present, &jwt)
    });
    for meta in std::iter::once(stream).chain(upload) {
        assert!(meta.ends_with("body_preview=[REDACTED]"), "{meta}");
        assert!(!meta.contains("eyJ"));
    }
}

#[test]
fn trace_full_existing_log_is_preserved_without_growth_or_truncation() {
    let dir = TraceDirectory::new();
    let old = vec![b'x'; MAX_TRACE_BYTES as usize + 7];
    std::fs::write(dir.log(), &old).unwrap();
    assert!(!append_bounded_gemini_canvas_trace(&dir.log(), "stage", "detail").unwrap());
    assert_eq!(std::fs::read(dir.log()).unwrap(), old);
}

#[test]
fn trace_exact_capacity_accepts_one_record_and_skips_the_next() {
    let dir = TraceDirectory::new();
    let expected = "42\tstage\tdetail\n";
    let cap = expected.len() as u64;
    assert!(append_trace_entry_at(&dir.log(), "stage", "detail", 42, cap).unwrap());
    assert!(!append_trace_entry_at(&dir.log(), "stage", "detail", 42, cap).unwrap());
    assert_eq!(std::fs::read_to_string(dir.log()).unwrap(), expected);
}

#[test]
fn trace_concurrent_writers_keep_complete_records_within_capacity() {
    let dir = TraceDirectory::new();
    let barrier = Arc::new(Barrier::new(8));
    let writers: Vec<_> = (0..8)
        .map(|_| {
            let path = dir.log();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..32 {
                    append_trace_entry_at(&path, "stage", "detail", 42, 128).unwrap();
                }
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap();
    }
    let bytes = std::fs::read(dir.log()).unwrap();
    assert!(!bytes.is_empty());
    assert!(bytes.len() <= 128);
    let text = String::from_utf8(bytes).unwrap();
    assert!(text.lines().all(|line| line == "42\tstage\tdetail"));
}

#[test]
fn trace_preview_redacts_before_truncation_and_keeps_normalized_contract() {
    use crate::upstream::gemini_canvas_runtime_error_helpers::compact_sanitized_response_preview;
    let dir = TraceDirectory::new();
    let jwt = format!("eyJ{}.{}.signature", "a".repeat(400), "b".repeat(400));
    let preview = compact_sanitized_response_preview(&jwt, 120);
    assert_eq!(preview, "[REDACTED]");
    assert_eq!(compact_sanitized_response_preview(" \n ", 10), "<empty>");
    assert_eq!(
        compact_sanitized_response_preview(" a  b\t c ", 3),
        "a b...(truncated)"
    );
    assert!(append_trace_entry_at(&dir.log(), "preview", &preview, 42, 128).unwrap());
    assert_eq!(
        std::fs::read_to_string(dir.log()).unwrap(),
        "42\tpreview\t[REDACTED]\n"
    );
}

#[test]
fn trace_byte_budget_counts_utf8_not_characters() {
    let dir = TraceDirectory::new();
    let detail = "\u{4f60}".repeat(40);
    let expected = format!("42\tstage\t{detail}\n");
    assert!(!append_trace_entry_at(&dir.log(), "stage", &detail, 42, 64).unwrap());
    assert_eq!(std::fs::metadata(dir.log()).unwrap().len(), 0);
    assert!(
        append_trace_entry_at(&dir.log(), "stage", &detail, 42, expected.len() as u64).unwrap()
    );
    assert_eq!(std::fs::read_to_string(dir.log()).unwrap(), expected);
}

#[test]
fn trace_non_file_target_is_not_modified() {
    let dir = TraceDirectory::new();
    std::fs::create_dir(dir.log()).unwrap();
    let marker = dir.log().join("marker");
    std::fs::write(&marker, b"preserve").unwrap();
    assert!(append_bounded_gemini_canvas_trace(&dir.log(), "stage", "detail").is_err());
    assert_eq!(std::fs::read(marker).unwrap(), b"preserve");
}

#[cfg(unix)]
#[test]
fn trace_symlink_target_is_not_followed() {
    let dir = TraceDirectory::new();
    let target = dir.0.join("target");
    std::fs::write(&target, b"preserve").unwrap();
    std::os::unix::fs::symlink(&target, dir.log()).unwrap();
    assert!(append_bounded_gemini_canvas_trace(&dir.log(), "stage", "detail").is_err());
    assert_eq!(std::fs::read(target).unwrap(), b"preserve");
}

#[path = "gemini_canvas_trace_process_tests.rs"]
mod process_tests;

#[path = "gemini_canvas_upload_http_tests.rs"]
mod upload_http_tests;
