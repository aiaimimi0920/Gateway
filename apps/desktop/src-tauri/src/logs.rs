use crate::paths::gateway_log_dir;
use crate::state::{GatewayDesktopState, GatewayProcessSnapshot};
use serde::Serialize;
use std::collections::VecDeque;
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;
use std::process::Command;

const MAX_LOG_TAIL_BYTES: u64 = 256 * 1024;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayLogTail {
    pub path: Option<String>,
    pub lines: Vec<String>,
}

fn redact_after_marker(line: &str, marker: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let marker_lower = marker.to_ascii_lowercase();
    let Some(start) = lower.find(&marker_lower) else {
        return line.to_string();
    };
    let value_start_raw = start + marker.len();
    let mut value_start = value_start_raw;
    while value_start < line.len() {
        let Some(character) = line[value_start..].chars().next() else {
            break;
        };
        if !character.is_whitespace() {
            break;
        }
        value_start += character.len_utf8();
    }
    if matches!(line[value_start..].chars().next(), Some('"' | '\'')) {
        value_start += 1;
    }
    let value_end = line[value_start..]
        .find(|character: char| character.is_whitespace() || matches!(character, ',' | ';' | '"'))
        .map(|offset| value_start + offset)
        .unwrap_or(line.len());
    let mut scrubbed = String::with_capacity(line.len());
    scrubbed.push_str(&line[..value_start]);
    scrubbed.push_str("[redacted]");
    scrubbed.push_str(&line[value_end..]);
    scrubbed
}

fn redact_bearer_token(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let Some(prefix_start) = lower.find("bearer ") else {
        return line.to_string();
    };
    let token_start = prefix_start + "bearer ".len();
    let token_end = line[token_start..]
        .find(char::is_whitespace)
        .map(|offset| token_start + offset)
        .unwrap_or(line.len());
    let mut scrubbed = String::with_capacity(line.len());
    scrubbed.push_str(&line[..token_start]);
    scrubbed.push_str("[redacted]");
    scrubbed.push_str(&line[token_end..]);
    scrubbed
}

fn redact_url_credentials(line: &str) -> String {
    let mut scrubbed = line.to_string();
    let mut search_from = 0;
    while let Some(relative_scheme) = scrubbed[search_from..].find("://") {
        let scheme_end = search_from + relative_scheme + 3;
        let authority_end = scrubbed[scheme_end..]
            .find(|character: char| character.is_whitespace() || matches!(character, '"' | '\''))
            .map(|offset| scheme_end + offset)
            .unwrap_or(scrubbed.len());
        let authority = &scrubbed[scheme_end..authority_end];
        let Some(at_offset) = authority.find('@') else {
            search_from = scheme_end;
            continue;
        };
        let credentials = &authority[..at_offset];
        if credentials.contains(':') {
            let replacement = format!("***:***@{}", &authority[at_offset + 1..]);
            scrubbed.replace_range(scheme_end..authority_end, &replacement);
            search_from = scheme_end + replacement.len();
        } else {
            search_from = authority_end;
        }
    }
    scrubbed
}

/// Remove credential material before a log line is returned to the desktop UI.
/// The headless process keeps its normal logs on disk; only the UI-facing tail is scrubbed.
pub(crate) fn scrub_runtime_log_line(line: &str) -> String {
    let mut scrubbed = line.to_string();
    loop {
        let next = redact_bearer_token(&scrubbed);
        if next == scrubbed {
            break;
        }
        scrubbed = next;
    }
    let markers = [
        "GATEWAY_MANAGEMENT_TOKEN=",
        "GATEWAY_API_KEY=",
        "GATEWAY_API_KEY_SECRET=",
        "GATEWAY_KEEPALIVE_BEARER_TOKEN=",
        "authorization=",
        "Authorization:",
        "cookie=",
        "Cookie:",
        "password=",
        "passwd=",
        "secret=",
        "secret:",
        "\"secret\":",
        "token=",
        "token:",
        "\"token\":",
        "\"password\":",
        "\"authorization\":",
        "\"cookie\":",
    ];
    for marker in markers {
        loop {
            let next = redact_after_marker(&scrubbed, marker);
            if next == scrubbed {
                break;
            }
            scrubbed = next;
        }
    }
    redact_url_credentials(&scrubbed)
}

fn read_bounded_tail(path: &Path) -> Result<Vec<u8>, std::io::Error> {
    let mut file = File::open(path)?;
    let length = file.metadata()?.len();
    let tail_length = length.min(MAX_LOG_TAIL_BYTES);
    file.seek(SeekFrom::End(-(tail_length as i64)))?;
    let mut bytes = Vec::with_capacity(tail_length as usize);
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

pub(crate) fn read_log_tail_lines(path: &Path, max_lines: usize) -> Result<Vec<String>, String> {
    let limit = max_lines.max(1);
    let bytes = read_bounded_tail(path)
        .map_err(|error| format!("failed to read gateway log {}: {error}", path.display()))?;
    let contents = String::from_utf8_lossy(&bytes);
    let mut tail = VecDeque::with_capacity(limit);
    for line in contents.lines() {
        if tail.len() == limit {
            tail.pop_front();
        }
        tail.push_back(scrub_runtime_log_line(line));
    }
    Ok(tail.into_iter().collect())
}

fn current_snapshot(
    state: &tauri::State<'_, GatewayDesktopState>,
) -> Result<GatewayProcessSnapshot, String> {
    let guard = state
        .process
        .lock()
        .map_err(|_| "gateway desktop process lock poisoned".to_string())?;
    Ok(guard.snapshot.clone())
}

fn open_path_in_system_shell(path: &Path) -> Result<(), String> {
    #[cfg(windows)]
    {
        Command::new("explorer")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("failed to open log directory {}: {error}", path.display()))
    }

    #[cfg(target_os = "macos")]
    {
        Command::new("open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("failed to open log directory {}: {error}", path.display()))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        Command::new("xdg-open")
            .arg(path)
            .spawn()
            .map(|_| ())
            .map_err(|error| format!("failed to open log directory {}: {error}", path.display()))
    }
}

#[tauri::command]
pub fn open_gateway_log_directory() -> Result<String, String> {
    let log_dir = gateway_log_dir()?;
    open_path_in_system_shell(&log_dir)?;
    Ok(log_dir.display().to_string())
}

#[tauri::command]
pub fn read_gateway_log_tail(
    state: tauri::State<'_, GatewayDesktopState>,
    max_lines: Option<usize>,
) -> Result<GatewayLogTail, String> {
    let snapshot = current_snapshot(&state)?;
    let Some(path) = snapshot.log_path.clone() else {
        return Ok(GatewayLogTail {
            path: None,
            lines: Vec::new(),
        });
    };

    Ok(GatewayLogTail {
        path: Some(path.clone()),
        lines: read_log_tail_lines(Path::new(&path), max_lines.unwrap_or(120))?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temporary_log_path() -> std::path::PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock must be after unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("neuro-gateway-log-contract-{nonce}.log"))
    }

    #[test]
    fn log_tail_is_bounded_and_scrubs_runtime_secrets() {
        let path = temporary_log_path();
        let mut contents = String::new();
        for index in 0..20_000 {
            contents.push_str(&format!(
                "line-{index} GATEWAY_MANAGEMENT_TOKEN=secret-{index}\n"
            ));
        }
        fs::write(&path, contents).expect("write test log");

        let lines = read_log_tail_lines(&path, 3).expect("read bounded tail");
        assert_eq!(lines.len(), 3);
        assert!(lines.iter().all(|line| !line.contains("secret-")));
        assert!(lines.iter().all(|line| line.contains("[redacted]")));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn malformed_url_like_log_text_is_scrubbed() {
        let line = "redis://user:super-secret@host:6379 token=abc123";
        let scrubbed = scrub_runtime_log_line(line);
        assert!(!scrubbed.contains("super-secret"));
        assert!(!scrubbed.contains("abc123"));
        assert!(scrubbed.contains("[redacted]"));
    }

    #[test]
    fn header_and_json_secret_values_are_scrubbed() {
        for line in [
            "Authorization: Bearer auth-secret",
            "Cookie: session=cookie-secret",
            r#"{"token": "json-secret"}"#,
        ] {
            let scrubbed = scrub_runtime_log_line(line);
            assert!(!scrubbed.contains("auth-secret"));
            assert!(!scrubbed.contains("cookie-secret"));
            assert!(!scrubbed.contains("json-secret"));
        }
    }
}
