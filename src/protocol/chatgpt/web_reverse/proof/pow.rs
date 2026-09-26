//! Legacy browser configuration and bounded PoW attempt generation.
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use rand::seq::SliceRandom;
use serde_json::{json, Value};
use sha3::{Digest, Sha3_512};
use std::time::{SystemTime, UNIX_EPOCH};
use time::{OffsetDateTime, UtcOffset};

use super::super::{
    ChatGptWebBootstrap, CHATGPT_WEB_DEFAULT_POW_SCRIPT, CHATGPT_WEB_REVERSE_ADAPTER,
};
use crate::error::GatewayError;

pub fn build_legacy_requirements_token(
    bootstrap: &ChatGptWebBootstrap,
    user_agent: &str,
) -> String {
    let seed = format!("{}", rand::random::<f64>());
    let config = build_pow_config(bootstrap, user_agent);
    let (answer, _solved) = pow_generate(&seed, "0fffff", &config, 500_000);
    format!("gAAAAAC{answer}")
}

pub fn build_proof_token(
    bootstrap: &ChatGptWebBootstrap,
    user_agent: &str,
    seed: &str,
    difficulty: &str,
) -> Result<String, GatewayError> {
    if !valid_pow_difficulty(difficulty) {
        return Err(GatewayError::service_unavailable(
            "ChatGPT Web reverse received invalid proof difficulty",
        )
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code("chatgpt_web_proof_token_failed"));
    }
    let config = build_pow_config(bootstrap, user_agent);
    let (answer, solved) = pow_generate(seed, difficulty, &config, 500_000);
    if !solved {
        return Err(GatewayError::service_unavailable(format!(
            "ChatGPT Web reverse failed to solve proof token: difficulty={difficulty}"
        ))
        .with_provider(CHATGPT_WEB_REVERSE_ADAPTER)
        .with_code("chatgpt_web_proof_token_failed"));
    }
    Ok(format!("gAAAAAB{answer}"))
}

fn valid_pow_difficulty(difficulty: &str) -> bool {
    // Bound untrusted input before scanning or decoding; SHA3-512 has 64 bytes.
    (2..=128).contains(&difficulty.len())
        && difficulty.len() % 2 == 0
        && difficulty.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn build_pow_config(bootstrap: &ChatGptWebBootstrap, user_agent: &str) -> Vec<Value> {
    let script_sources = if bootstrap.pow_script_sources.is_empty() {
        vec![CHATGPT_WEB_DEFAULT_POW_SCRIPT.to_string()]
    } else {
        bootstrap.pow_script_sources.clone()
    };
    let navigator_keys = vec![
        "registerProtocolHandler−function registerProtocolHandler() { [native code] }",
        "storage−[object StorageManager]",
        "locks−[object LockManager]",
        "appCodeName−Mozilla",
        "permissions−[object Permissions]",
        "share−function share() { [native code] }",
        "webdriver−false",
        "managed−[object NavigatorManagedData]",
        "canShare−function canShare() { [native code] }",
        "vendor−Google Inc.",
        "mediaDevices−[object MediaDevices]",
        "vibrate−function vibrate() { [native code] }",
        "storageBuckets−[object StorageBucketManager]",
        "mediaCapabilities−[object MediaCapabilities]",
        "cookieEnabled−true",
        "virtualKeyboard−[object VirtualKeyboard]",
        "product−Gecko",
        "presentation−[object Presentation]",
        "onLine−true",
        "mimeTypes−[object MimeTypeArray]",
        "credentials−[object CredentialsContainer]",
        "serviceWorker−[object ServiceWorkerContainer]",
        "keyboard−[object Keyboard]",
        "gpu−[object GPU]",
        "doNotTrack",
        "serial−[object Serial]",
        "pdfViewerEnabled−true",
        "language−zh-CN",
        "geolocation−[object Geolocation]",
        "userAgentData−[object NavigatorUAData]",
        "getUserMedia−function getUserMedia() { [native code] }",
        "sendBeacon−function sendBeacon() { [native code] }",
        "hardwareConcurrency−32",
        "windowControlsOverlay−[object WindowControlsOverlay]",
    ];
    let window_keys = vec![
        "0",
        "window",
        "self",
        "document",
        "name",
        "location",
        "customElements",
        "history",
        "navigation",
        "innerWidth",
        "innerHeight",
        "scrollX",
        "scrollY",
        "visualViewport",
        "screenX",
        "screenY",
        "outerWidth",
        "outerHeight",
        "devicePixelRatio",
        "screen",
        "chrome",
        "navigator",
        "onresize",
        "performance",
        "crypto",
        "indexedDB",
        "sessionStorage",
        "localStorage",
        "scheduler",
        "alert",
        "atob",
        "btoa",
        "fetch",
        "matchMedia",
        "postMessage",
        "queueMicrotask",
        "requestAnimationFrame",
        "setInterval",
        "setTimeout",
        "caches",
        "__NEXT_DATA__",
        "__BUILD_MANIFEST",
        "__NEXT_PRELOADREADY",
    ];
    let document_keys = vec!["_reactListeningo743lnnpvdg", "location"];
    let cores = [8_i64, 16, 24, 32];
    let mut rng = rand::thread_rng();
    let script_source = script_sources
        .choose(&mut rng)
        .cloned()
        .unwrap_or_else(|| CHATGPT_WEB_DEFAULT_POW_SCRIPT.to_string());
    let navigator_key = navigator_keys
        .choose(&mut rng)
        .copied()
        .unwrap_or("vendor−Google Inc.");
    let window_key = window_keys.choose(&mut rng).copied().unwrap_or("window");
    let document_key = document_keys
        .choose(&mut rng)
        .copied()
        .unwrap_or("location");
    let core_count = cores.choose(&mut rng).copied().unwrap_or(8);
    let timestamp_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as f64;
    let perf_ms = timestamp_ms.fract() * 1000.0;
    let time_origin_ms = timestamp_ms - perf_ms;
    vec![
        json!(*[3000_i64, 4000, 5000].choose(&mut rng).unwrap_or(&3000)),
        json!(legacy_parse_time()),
        json!(4294705152u64),
        json!(0),
        json!(user_agent),
        json!(script_source),
        json!(bootstrap.pow_data_build.clone().unwrap_or_default()),
        json!("en-US"),
        json!("en-US,es-US,en,es"),
        json!(0),
        json!(navigator_key),
        json!(document_key),
        json!(window_key),
        json!(perf_ms),
        json!(uuid::Uuid::new_v4().to_string()),
        json!(""),
        json!(core_count),
        json!(time_origin_ms),
    ]
}

fn legacy_parse_time() -> String {
    const WEEKDAYS: [&str; 7] = ["Mon", "Tue", "Wed", "Thu", "Fri", "Sat", "Sun"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let offset = UtcOffset::from_hms(-5, 0, 0).unwrap_or(UtcOffset::UTC);
    let now = OffsetDateTime::now_utc().to_offset(offset);
    let weekday = WEEKDAYS[now.weekday().number_days_from_monday() as usize];
    let month = MONTHS[(now.month() as u8).saturating_sub(1) as usize];
    format!(
        "{weekday} {month} {:02} {:04} {:02}:{:02}:{:02} GMT-0500 (Eastern Standard Time)",
        now.day(),
        now.year(),
        now.hour(),
        now.minute(),
        now.second()
    )
}

pub(super) fn pow_generate(
    seed: &str,
    difficulty: &str,
    config: &[Value],
    limit: usize,
) -> (String, bool) {
    if !valid_pow_difficulty(difficulty) {
        return (String::new(), false);
    }
    let Ok(target) = hex::decode(difficulty) else {
        return (String::new(), false);
    };
    let diff_len = target.len();
    let static_1 = legacy_json_prefix(&config[..3]);
    let static_2 = legacy_json_middle(&config[4..9]);
    let static_3 = legacy_json_tail(&config[10..]);
    for i in 0..limit {
        let final_json = format!("{static_1}{i}{static_2}{}{static_3}", i >> 1);
        let encoded = BASE64_STANDARD.encode(final_json.as_bytes());
        let mut hasher = Sha3_512::new();
        hasher.update(seed.as_bytes());
        hasher.update(encoded.as_bytes());
        let hash = hasher.finalize();
        if hash[..diff_len] <= target[..diff_len] {
            return (encoded, true);
        }
    }
    (
        format!(
            "wQ8Lk5FbGpA2NcR9dShT6gYjU7VxZ4D{}",
            BASE64_STANDARD.encode(format!("\"{seed}\"").as_bytes())
        ),
        false,
    )
}

fn legacy_json_prefix(items: &[Value]) -> String {
    let mut text = serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string());
    if text.ends_with(']') {
        text.pop();
    }
    text.push(',');
    text
}

fn legacy_json_middle(items: &[Value]) -> String {
    let text = serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string());
    let inner = text.trim_start_matches('[').trim_end_matches(']');
    format!(",{inner},")
}

fn legacy_json_tail(items: &[Value]) -> String {
    let text = serde_json::to_string(items).unwrap_or_else(|_| "[]".to_string());
    let tail = text.strip_prefix('[').unwrap_or(&text);
    format!(",{tail}")
}

#[cfg(test)]
mod input_tests;
