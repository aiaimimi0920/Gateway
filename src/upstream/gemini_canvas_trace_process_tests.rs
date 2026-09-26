use super::TraceDirectory;
use crate::upstream::gemini_canvas_trace_writer::append_bounded_gemini_canvas_trace;
use std::io::Read;
use std::process::{Child, Command, ExitStatus, Stdio};
use std::time::{Duration, Instant};

const PROBE_DIRECTORY: &str = "GATEWAY_TEST_TRACE_PROBE_DIRECTORY";
const PROBE_MODE: &str = "GATEWAY_TEST_TRACE_PROBE_MODE";

struct ReapChild(Child);

impl Drop for ReapChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn spawn_probe(dir: &TraceDirectory, mode: &str) -> ReapChild {
    std::fs::create_dir_all(dir.0.join(".runtime")).unwrap();
    let module = module_path!().split_once("::").unwrap().1;
    let test = format!("{module}::trace_child_probe");
    let mut command = Command::new(std::env::current_exe().unwrap());
    command.env_remove("GEMINI_CANVAS_IMAGE_EDIT_TRACE");
    command.env_remove("GEMINI_CANVAS_IMAGE_EDIT_TRACE_RAW_IMAGES");
    command.env_remove("GEMINI_CANVAS_IMAGE_EDIT_RESOURCE_PATH_OVERRIDE");
    if matches!(mode, "image-on" | "image-only") {
        command.env("GEMINI_CANVAS_IMAGE_EDIT_TRACE_RAW_IMAGES", "1");
    } else if mode == "image-zero" {
        command.env("GEMINI_CANVAS_IMAGE_EDIT_TRACE_RAW_IMAGES", "0");
    }
    if matches!(
        mode,
        "snapshot" | "image-off" | "image-zero" | "image-on" | "upload-http-on"
    ) {
        command.env("GEMINI_CANVAS_IMAGE_EDIT_TRACE", "1");
    }
    ReapChild(
        command
            .current_dir(&dir.0)
            .args(["--exact", &test, "--ignored", "--nocapture"])
            .env(PROBE_DIRECTORY, &dir.0)
            .env(PROBE_MODE, mode)
            .stdout(Stdio::piped())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

fn wait_for_exit(child: &mut ReapChild, deadline: Instant) -> ExitStatus {
    loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            return status;
        }
        assert!(Instant::now() < deadline, "trace child timed out");
        std::thread::sleep(Duration::from_millis(10));
    }
}

fn assert_probe_passed(child: &mut ReapChild, status: ExitStatus) {
    let mut output = String::new();
    child
        .0
        .stdout
        .take()
        .unwrap()
        .take(8192)
        .read_to_string(&mut output)
        .unwrap();
    assert!(status.success(), "trace probe failed: {output}");
    assert!(
        output.contains("1 passed; 0 failed"),
        "trace probe did not run: {output}"
    );
}

#[test]
fn trace_process_lock_skips_without_waiting_and_releases_cleanly() {
    let dir = TraceDirectory::new();
    std::fs::write(dir.log(), b"existing\n").unwrap();
    let mut child = spawn_probe(&dir, "lock");
    let deadline = Instant::now() + Duration::from_secs(30);
    while !dir.0.join("ready").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "lock probe exited before ready"
        );
        assert!(Instant::now() < deadline, "lock probe was not ready");
        std::thread::sleep(Duration::from_millis(10));
    }
    let started = Instant::now();
    assert!(!append_bounded_gemini_canvas_trace(&dir.log(), "stage", "detail").unwrap());
    assert!(started.elapsed() < Duration::from_secs(5));
    std::fs::write(dir.0.join("release"), b"release").unwrap();
    let status = wait_for_exit(&mut child, deadline);
    assert_probe_passed(&mut child, status);
    assert_eq!(std::fs::read(dir.log()).unwrap(), b"existing\n");
    assert!(append_bounded_gemini_canvas_trace(&dir.log(), "stage", "detail").unwrap());
}

#[test]
fn trace_disabled_gate_does_not_evaluate_diagnostic_input() {
    let dir = TraceDirectory::new();
    let mut child = spawn_probe(&dir, "disabled");
    let status = wait_for_exit(&mut child, Instant::now() + Duration::from_secs(30));
    assert_probe_passed(&mut child, status);
    assert!(!dir
        .0
        .join(".runtime")
        .join("gemini-canvas-image-edit-trace.log")
        .exists());
    assert!(!dir.0.join(".runtime/gemini-canvas-debug").exists());
}

#[test]
fn gemini_canvas_upload_http_branches_survive_trace_gating() {
    for mode in ["upload-http-off", "upload-http-on"] {
        let dir = TraceDirectory::new();
        let mut child = spawn_probe(&dir, mode);
        let status = wait_for_exit(&mut child, Instant::now() + Duration::from_secs(30));
        assert_probe_passed(&mut child, status);
        assert_eq!(
            dir.0.join(".runtime/gemini-canvas-debug").exists(),
            mode == "upload-http-on"
        );
    }
}

#[test]
fn gemini_canvas_snapshot_probe_writes_only_sanitized_isolated_files() {
    let dir = TraceDirectory::new();
    let mut child = spawn_probe(&dir, "snapshot");
    let status = wait_for_exit(&mut child, Instant::now() + Duration::from_secs(30));
    assert_probe_passed(&mut child, status);
    let root = dir.0.join(".runtime");
    let snapshots = root.join("gemini-canvas-debug");
    let json = std::fs::read_to_string(snapshots.join("gemini-canvas-fixture.json")).unwrap();
    assert!(!json.contains("fixture-secret"));
    assert!(json.contains("<redacted>"));
    let text = std::fs::read_to_string(snapshots.join("gemini-canvas-response.txt")).unwrap();
    assert!(!text.contains("fixture-secret"));
    assert!(text.contains("[REDACTED]"));
    let request = std::fs::read_to_string(snapshots.join("gemini-canvas-request.json")).unwrap();
    assert!(!request.contains("fixture-secret"));
    assert!(request.contains("<redacted>"));
    assert!(!root.join("gemini-canvas-fixture.json").exists());
}

#[test]
fn gemini_canvas_raw_image_snapshot_requires_explicit_dual_opt_in() {
    for mode in ["image-off", "image-zero", "image-on", "image-only"] {
        let dir = TraceDirectory::new();
        let mut child = spawn_probe(&dir, mode);
        let status = wait_for_exit(&mut child, Instant::now() + Duration::from_secs(30));
        assert_probe_passed(&mut child, status);
        let path = dir
            .0
            .join(".runtime/gemini-canvas-debug/gemini-canvas-private.jpg");
        assert_eq!(path.exists(), mode == "image-on", "{mode}");
        if path.exists() {
            assert_eq!(std::fs::read(path).unwrap(), b"fixture-private-image");
        }
    }
}

#[test]
#[ignore = "invoked by parent tests in an isolated process"]
fn trace_child_probe() {
    let dir = std::path::PathBuf::from(std::env::var_os(PROBE_DIRECTORY).unwrap());
    match std::env::var(PROBE_MODE).unwrap().as_str() {
        mode @ ("upload-http-off" | "upload-http-on") => {
            super::upload_http_tests::run_http_contracts(mode == "upload-http-on");
        }
        mode @ ("image-off" | "image-zero" | "image-on" | "image-only") => {
            let result = crate::upstream::gemini_canvas_diagnostics::write_gemini_canvas_image_edit_debug_bytes(
                "gemini-canvas-private.jpg", b"fixture-private-image",
            );
            assert_eq!(result.is_some(), mode == "image-on", "{mode}");
        }
        "snapshot" => {
            use crate::upstream::gemini_canvas_diagnostics::{
                append_gemini_canvas_image_edit_debug_json,
                append_gemini_canvas_image_edit_request_debug_snapshot,
                append_gemini_canvas_image_edit_stream_response_debug_snapshot,
            };
            let calls = std::cell::Cell::new(0);
            let builds = std::cell::Cell::new(0);
            let upload = std::cell::LazyCell::new(|| {
                builds.set(builds.get() + 1);
                crate::upstream::gemini_canvas_upload_debug::build_gemini_canvas_upload_debug(
                    b"abc",
                )
            });
            append_gemini_canvas_image_edit_debug_json("gemini-canvas-fixture.json", || {
                calls.set(calls.get() + 1);
                serde_json::json!({"token": "fixture-secret", "uploadSha": upload.sha256})
            });
            append_gemini_canvas_image_edit_request_debug_snapshot(
                "gemini-canvas-request.json",
                "fixture",
                "https://example.test",
                &[],
                &[],
                || {
                    calls.set(calls.get() + 1);
                    serde_json::json!({"Authorization": "fixture-secret"})
                },
                || {
                    calls.set(calls.get() + 1);
                    serde_json::json!({"dimensions": upload.dimensions})
                },
            );
            append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                "gemini-canvas-response",
                "fixture",
                "https://example.test",
                200,
                None,
                None,
                "token=fixture-secret",
                || {
                    calls.set(calls.get() + 1);
                    serde_json::json!({})
                },
            );
            assert_eq!(calls.get(), 4);
            assert_eq!(builds.get(), 1);
            assert_eq!(
                upload.sha256,
                "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
            );
            assert_eq!(upload.dimensions, None);
            assert_eq!(upload.output_path, None);
            let mut png = std::io::Cursor::new(Vec::new());
            image::DynamicImage::new_rgb8(2, 3)
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            let valid =
                crate::upstream::gemini_canvas_upload_debug::build_gemini_canvas_upload_debug(
                    png.get_ref(),
                );
            assert_eq!(valid.dimensions, Some((2, 3)));
            assert_eq!(valid.output_path, None);
            let prepared = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
                .block_on(
                    crate::upstream::gemini_canvas_upload_debug::prepare_gemini_canvas_upload_debug(
                        &bytes::Bytes::from(png.into_inner()),
                    ),
                )
                .unwrap();
            assert_eq!(prepared.dimensions, valid.dimensions);
            assert_eq!(prepared.sha256, valid.sha256);
            assert_eq!(prepared.output_path, valid.output_path);
        }
        "disabled" => {
            use crate::upstream::gemini_canvas_diagnostics::{
                append_gemini_canvas_image_edit_debug_json,
                append_gemini_canvas_image_edit_request_debug_snapshot,
                append_gemini_canvas_image_edit_stream_response_debug_snapshot,
            };
            let upload = std::cell::LazyCell::new(|| -> serde_json::Value {
                panic!("disabled snapshot forced cached upload metadata")
            });
            append_gemini_canvas_image_edit_debug_json("gemini-canvas-disabled.json", || {
                (*upload).clone()
            });
            append_gemini_canvas_image_edit_request_debug_snapshot(
                "gemini-canvas-disabled-request.json",
                "disabled",
                "https://example.test",
                &[],
                &[],
                || panic!("disabled snapshot evaluated headers"),
                || panic!("disabled snapshot evaluated request extra"),
            );
            append_gemini_canvas_image_edit_stream_response_debug_snapshot(
                "gemini-canvas-disabled-response",
                "disabled",
                "https://example.test",
                200,
                None,
                None,
                "fixture body",
                || panic!("disabled snapshot evaluated response extra"),
            );
            crate::upstream::gemini_canvas_diagnostics::append_gemini_canvas_image_edit_trace(
                "disabled",
                || -> &str { panic!("disabled trace evaluated its input") },
            );
        }
        "lock" => {
            let file = std::fs::OpenOptions::new()
                .read(true)
                .write(true)
                .open(dir.join("trace.log"))
                .unwrap();
            file.try_lock().unwrap();
            std::fs::write(dir.join("ready"), b"ready").unwrap();
            let deadline = Instant::now() + Duration::from_secs(30);
            while !dir.join("release").exists() {
                assert!(Instant::now() < deadline, "lock probe release timed out");
                std::thread::sleep(Duration::from_millis(10));
            }
        }
        mode => panic!("unknown trace probe mode: {mode}"),
    }
}
