use super::gemini_canvas_encoder_process::EncoderAdmission;
use super::gemini_canvas_encoder_workspace::{EncoderWorkspace, MAX_SOURCE_BYTES};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::process::Command;

pub(crate) struct GeminiCanvasImageEncoding {
    pub(crate) bytes: Option<Vec<u8>>,
    pub(crate) metadata: Option<Value>,
}

pub(crate) async fn reencode_gemini_canvas_image_edit_input(
    source_bytes: &[u8],
    source_mime_type: &str,
) -> GeminiCanvasImageEncoding {
    let mut encoded_bytes = None;
    let mut browser_reencode_meta = None;
    if source_bytes.len() > MAX_SOURCE_BYTES {
        return GeminiCanvasImageEncoding {
            bytes: None,
            metadata: None,
        };
    }
    if let Some(script_path) = gemini_canvas_image_edit_browser_encoder_script_path() {
        // The lease follows blocking IO and the process reaper, not just this caller future.
        let Some(admission) = EncoderAdmission::acquire(Duration::from_secs(45)).await else {
            return GeminiCanvasImageEncoding {
                bytes: None,
                metadata: None,
            };
        };
        let source = source_bytes.to_vec();
        let extension = gemini_canvas_image_edit_source_extension(source_mime_type);
        let lease = admission.lease();
        let prepared = tokio::task::spawn_blocking(move || {
            EncoderWorkspace::create(&source, extension, lease).map(Arc::new)
        })
        .await;
        let Ok(Ok(workspace)) = prepared else {
            return GeminiCanvasImageEncoding {
                bytes: None,
                metadata: None,
            };
        };
        let input_path = &workspace.source;
        let output_path = &workspace.output;
        let mut command = Command::new("node");
        command
            .arg(&script_path)
            .arg(&input_path)
            .arg(&output_path)
            .arg("127467");
        if let Some(output) = admission.run_retaining(command, workspace.clone()).await {
            let stdout_text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            let stderr_text = String::from_utf8_lossy(&output.stderr).trim().to_string();
            if output.status.success() {
                let reader = workspace.clone();
                if let Ok(Ok(browser_bytes)) =
                    tokio::task::spawn_blocking(move || reader.read_output()).await
                {
                    if !browser_bytes.is_empty() {
                        encoded_bytes = Some(browser_bytes);
                        browser_reencode_meta =
                            Some(build_gemini_canvas_image_edit_browser_reencode_meta(
                                &script_path,
                                &input_path,
                                &output_path,
                                &stdout_text,
                                &stderr_text,
                                true,
                                None,
                            ));
                    }
                }
            }
            if browser_reencode_meta.is_none() {
                browser_reencode_meta = Some(build_gemini_canvas_image_edit_browser_reencode_meta(
                    &script_path,
                    &input_path,
                    &output_path,
                    &stdout_text,
                    &stderr_text,
                    false,
                    output.status.code(),
                ));
            }
        }
    }

    GeminiCanvasImageEncoding {
        bytes: encoded_bytes,
        metadata: browser_reencode_meta,
    }
}

fn gemini_canvas_image_edit_source_extension(mime_type: &str) -> &'static str {
    let mime_type = mime_type.trim();
    if mime_type.eq_ignore_ascii_case("image/jpeg") || mime_type.eq_ignore_ascii_case("image/jpg") {
        "jpg"
    } else if mime_type.eq_ignore_ascii_case("image/webp") {
        "webp"
    } else if mime_type.eq_ignore_ascii_case("image/gif") {
        "gif"
    } else if mime_type.eq_ignore_ascii_case("image/bmp") {
        "bmp"
    } else {
        "png"
    }
}

fn gemini_canvas_image_edit_browser_encoder_script_path() -> Option<PathBuf> {
    let repo_relative = PathBuf::from("gateway")
        .join("scripts")
        .join("gemini-canvas-image-edit-encode.mjs");
    if repo_relative.exists() {
        return Some(repo_relative);
    }
    let gateway_relative = PathBuf::from("scripts").join("gemini-canvas-image-edit-encode.mjs");
    if gateway_relative.exists() {
        return Some(gateway_relative);
    }
    None
}

fn build_gemini_canvas_image_edit_browser_reencode_meta(
    script_path: &std::path::Path,
    source_path: &std::path::Path,
    output_path: &std::path::Path,
    stdout: &str,
    stderr: &str,
    used: bool,
    exit_code: Option<i32>,
) -> Value {
    let mut value = json!({
        "scriptPath": script_path.to_string_lossy(),
        "sourcePath": source_path.to_string_lossy(),
        "outputPath": output_path.to_string_lossy(),
        "stdout": stdout,
        "stderr": stderr,
        "used": used,
    });
    if let Some(code) = exit_code {
        value["exitCode"] = json!(code);
    }
    value
}

#[cfg(test)]
#[path = "gemini_canvas_image_encoder_tests.rs"]
mod tests;
