use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use rand::seq::SliceRandom;
use serde_json::{json, Value};
use sha3::{Digest, Sha3_512};
use std::collections::HashMap;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use time::{OffsetDateTime, UtcOffset};

use super::{ChatGptWebBootstrap, CHATGPT_WEB_DEFAULT_POW_SCRIPT, CHATGPT_WEB_REVERSE_ADAPTER};
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

fn pow_generate(seed: &str, difficulty: &str, config: &[Value], limit: usize) -> (String, bool) {
    let target = hex::decode(difficulty).unwrap_or_else(|_| vec![0x0f, 0xff, 0xff]);
    let diff_len = difficulty.len() / 2;
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

pub fn solve_turnstile_token(dx: &str, p: &str) -> Option<String> {
    let decoded = String::from_utf8(decode_base64_compat(dx)?).ok()?;
    let token_program_text = xor_string(&decoded, p)?;
    let token_program: Value = serde_json::from_str(&token_program_text).ok()?;
    let mut process_map = initial_turnstile_process_map(&token_program, p);
    let start = Instant::now();
    let mut result = String::new();
    let mut current_program = token_program;

    for _ in 0..4 {
        let instructions = current_program.as_array()?;
        for instruction in instructions {
            let Some(items) = instruction.as_array() else {
                continue;
            };
            let Some(op) = items.first().and_then(turnstile_key_from_json) else {
                continue;
            };
            let args = &items[1..];
            let Some(TurnstileValue::Func(code)) = process_map.get(&op).cloned() else {
                continue;
            };
            call_turnstile_func(code, args, &mut process_map, &start, &mut result);
        }
        if !result.is_empty() {
            break;
        }
        let Some(next_program) = process_map
            .get(&turnstile_key_from_i64(9))
            .and_then(turnstile_value_to_json)
        else {
            break;
        };
        if next_program == current_program {
            break;
        }
        current_program = next_program;
    }

    if result.is_empty() {
        None
    } else {
        Some(result)
    }
}

#[derive(Clone, Debug, PartialEq)]
struct OrderedTurnstileMap {
    entries: Vec<(String, TurnstileValue)>,
}

impl OrderedTurnstileMap {
    fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    fn add(&mut self, key: String, value: TurnstileValue) {
        if let Some((_, existing)) = self
            .entries
            .iter_mut()
            .find(|(existing_key, _)| existing_key == &key)
        {
            *existing = value;
            return;
        }
        self.entries.push((key, value));
    }
}

#[derive(Clone, Debug, PartialEq)]
enum TurnstileValue {
    Undefined,
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    Array(Vec<TurnstileValue>),
    Object(Vec<(String, TurnstileValue)>),
    OrderedMap(OrderedTurnstileMap),
    Func(i64),
}

fn initial_turnstile_process_map(program: &Value, p: &str) -> HashMap<String, TurnstileValue> {
    let mut process_map = HashMap::new();
    for code in [1, 2, 3, 5, 6, 7, 8, 14, 15, 17, 18, 19, 20, 21, 23, 24] {
        process_map.insert(turnstile_key_from_i64(code), TurnstileValue::Func(code));
    }
    process_map.insert(
        turnstile_key_from_i64(9),
        turnstile_value_from_json(program),
    );
    process_map.insert(
        turnstile_key_from_i64(10),
        TurnstileValue::String("window".to_string()),
    );
    process_map.insert(
        turnstile_key_from_i64(16),
        TurnstileValue::String(p.to_string()),
    );
    process_map
}

fn call_turnstile_func(
    code: i64,
    args: &[Value],
    process_map: &mut HashMap<String, TurnstileValue>,
    start: &Instant,
    result: &mut String,
) {
    let call_result = match code {
        1 => {
            let (Some(e), Some(t)) = (
                args.first().and_then(turnstile_key_from_json),
                args.get(1).and_then(turnstile_key_from_json),
            ) else {
                return;
            };
            let left = process_map
                .get(&e)
                .map(turnstile_to_string)
                .unwrap_or_else(|| "undefined".to_string());
            let right = process_map
                .get(&t)
                .map(turnstile_to_string)
                .unwrap_or_else(|| "undefined".to_string());
            xor_string(&left, &right).map(TurnstileValue::String)
        }
        2 => {
            let Some(e) = args.first().and_then(turnstile_key_from_json) else {
                return;
            };
            process_map.insert(
                e,
                args.get(1)
                    .map(turnstile_value_from_json)
                    .unwrap_or(TurnstileValue::Undefined),
            );
            return;
        }
        3 => {
            if let Some(value) = args.first().map(turnstile_value_from_json) {
                *result = BASE64_STANDARD.encode(turnstile_to_string(&value).as_bytes());
            }
            return;
        }
        5 => {
            let (Some(e), Some(t)) = (
                args.first().and_then(turnstile_key_from_json),
                args.get(1).and_then(turnstile_key_from_json),
            ) else {
                return;
            };
            let current = process_map
                .get(&e)
                .cloned()
                .unwrap_or(TurnstileValue::Undefined);
            let incoming = process_map
                .get(&t)
                .cloned()
                .unwrap_or(TurnstileValue::Undefined);
            Some(match current {
                TurnstileValue::Array(mut items) => {
                    items.push(incoming);
                    TurnstileValue::Array(items)
                }
                value
                    if turnstile_is_string_or_number(&value)
                        || turnstile_is_string_or_number(&incoming) =>
                {
                    TurnstileValue::String(format!(
                        "{}{}",
                        turnstile_to_string(&value),
                        turnstile_to_string(&incoming)
                    ))
                }
                _ => TurnstileValue::String("NaN".to_string()),
            })
        }
        6 | 24 => {
            let (Some(e), Some(t), Some(n)) = (
                args.first().and_then(turnstile_key_from_json),
                args.get(1).and_then(turnstile_key_from_json),
                args.get(2).and_then(turnstile_key_from_json),
            ) else {
                return;
            };
            let Some(TurnstileValue::String(tv)) = process_map.get(&t).cloned() else {
                return;
            };
            let Some(TurnstileValue::String(nv)) = process_map.get(&n).cloned() else {
                return;
            };
            let joined = format!("{tv}.{nv}");
            let value = if code == 6 && joined == "window.document.location" {
                "https://chatgpt.com/".to_string()
            } else {
                joined
            };
            process_map.insert(e, TurnstileValue::String(value));
            return;
        }
        7 => {
            call_turnstile_apply(args, process_map, start, result);
            return;
        }
        8 => {
            let (Some(e), Some(t)) = (
                args.first().and_then(turnstile_key_from_json),
                args.get(1).and_then(turnstile_key_from_json),
            ) else {
                return;
            };
            process_map.get(&t).cloned().map(|value| {
                process_map.insert(e, value);
            });
            return;
        }
        14 => {
            let (Some(_e), Some(t)) = (
                args.first().and_then(turnstile_key_from_json),
                args.get(1).and_then(turnstile_key_from_json),
            ) else {
                return;
            };
            let Some(text) = process_map.get(&t).map(turnstile_to_string) else {
                return;
            };
            serde_json::from_str::<Value>(&text)
                .ok()
                .map(|value| turnstile_value_from_json(&value))
        }
        15 => {
            let (Some(_e), Some(t)) = (
                args.first().and_then(turnstile_key_from_json),
                args.get(1).and_then(turnstile_key_from_json),
            ) else {
                return;
            };
            process_map
                .get(&t)
                .map(py_json_dumps_turnstile)
                .map(TurnstileValue::String)
        }
        17 => {
            call_turnstile_dynamic(args, process_map, start);
            return;
        }
        18 => {
            let Some(e) = args.first().and_then(turnstile_key_from_json) else {
                return;
            };
            let Some(text) = process_map.get(&e).map(turnstile_to_string) else {
                return;
            };
            decode_base64_compat(&text)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .map(TurnstileValue::String)
        }
        19 => {
            let Some(e) = args.first().and_then(turnstile_key_from_json) else {
                return;
            };
            process_map
                .get(&e)
                .map(turnstile_to_string)
                .map(|text| TurnstileValue::String(BASE64_STANDARD.encode(text.as_bytes())))
        }
        20 => {
            let (Some(e), Some(t), Some(n)) = (
                args.first().and_then(turnstile_key_from_json),
                args.get(1).and_then(turnstile_key_from_json),
                args.get(2).and_then(turnstile_key_from_json),
            ) else {
                return;
            };
            if process_map.get(&e) == process_map.get(&t) {
                if let Some(TurnstileValue::Func(code)) = process_map.get(&n).cloned() {
                    let values = args[3..]
                        .iter()
                        .filter_map(|arg| turnstile_key_from_json(arg))
                        .filter_map(|key| process_map.get(&key).cloned())
                        .collect::<Vec<_>>();
                    call_turnstile_func_values(code, &values, result);
                }
            }
            return;
        }
        21 => return,
        23 => {
            let (Some(e), Some(t)) = (
                args.first().and_then(turnstile_key_from_json),
                args.get(1).and_then(turnstile_key_from_json),
            ) else {
                return;
            };
            if !matches!(
                process_map.get(&e),
                None | Some(TurnstileValue::Null | TurnstileValue::Undefined)
            ) {
                if let Some(TurnstileValue::Func(code)) = process_map.get(&t).cloned() {
                    call_turnstile_func(code, &args[2..], process_map, start, result);
                }
            }
            return;
        }
        _ => return,
    };
    if let Some(value) = call_result {
        if let Some(e) = args.first().and_then(turnstile_key_from_json) {
            process_map.insert(e, value);
        }
    }
}

fn call_turnstile_apply(
    args: &[Value],
    process_map: &mut HashMap<String, TurnstileValue>,
    start: &Instant,
    result: &mut String,
) {
    let Some(target_key) = args.first().and_then(turnstile_key_from_json) else {
        return;
    };
    let Some(target) = process_map.get(&target_key).cloned() else {
        return;
    };
    if matches!(
        target,
        TurnstileValue::String(ref value) if value == "window.Reflect.set"
    ) {
        let (Some(object_key), Some(key_name_key), Some(value_key)) = (
            args.get(1).and_then(turnstile_key_from_json),
            args.get(2).and_then(turnstile_key_from_json),
            args.get(3).and_then(turnstile_key_from_json),
        ) else {
            return;
        };
        let key_name = process_map
            .get(&key_name_key)
            .map(turnstile_to_string)
            .unwrap_or_else(|| "undefined".to_string());
        let value = process_map
            .get(&value_key)
            .cloned()
            .unwrap_or(TurnstileValue::Undefined);
        if let Some(TurnstileValue::OrderedMap(object)) = process_map.get_mut(&object_key) {
            object.add(key_name, value);
        }
        return;
    }
    if let TurnstileValue::Func(code) = target {
        let values = args[1..]
            .iter()
            .filter_map(turnstile_key_from_json)
            .filter_map(|key| process_map.get(&key).cloned())
            .collect::<Vec<_>>();
        if code == 17 {
            call_turnstile_dynamic(args, process_map, start);
        } else {
            call_turnstile_func_values(code, &values, result);
        }
    }
}

fn call_turnstile_dynamic(
    args: &[Value],
    process_map: &mut HashMap<String, TurnstileValue>,
    start: &Instant,
) {
    let (Some(e), Some(t)) = (
        args.first().and_then(turnstile_key_from_json),
        args.get(1).and_then(turnstile_key_from_json),
    ) else {
        return;
    };
    let target = process_map
        .get(&t)
        .map(turnstile_to_string)
        .unwrap_or_else(|| "undefined".to_string());
    let call_args = args[2..]
        .iter()
        .filter_map(turnstile_key_from_json)
        .filter_map(|key| process_map.get(&key).cloned())
        .collect::<Vec<_>>();
    let value = match target.as_str() {
        "window.performance.now" => Some(TurnstileValue::Number(
            start.elapsed().as_secs_f64() * 1000.0 + rand::random::<f64>(),
        )),
        "window.Object.create" => Some(TurnstileValue::OrderedMap(OrderedTurnstileMap::new())),
        "window.Object.keys" => {
            if matches!(
                call_args.first(),
                Some(TurnstileValue::String(value)) if value == "window.localStorage"
            ) {
                Some(TurnstileValue::Array(
                    [
                        "STATSIG_LOCAL_STORAGE_INTERNAL_STORE_V4",
                        "STATSIG_LOCAL_STORAGE_STABLE_ID",
                        "client-correlated-secret",
                        "oai/apps/capExpiresAt",
                        "oai-did",
                        "STATSIG_LOCAL_STORAGE_LOGGING_REQUEST",
                        "UiState.isNavigationCollapsed.1",
                    ]
                    .into_iter()
                    .map(|value| TurnstileValue::String(value.to_string()))
                    .collect(),
                ))
            } else {
                None
            }
        }
        "window.Math.random" => Some(TurnstileValue::Number(rand::random::<f64>())),
        _ => None,
    };
    if let Some(value) = value {
        process_map.insert(e, value);
    }
}

fn call_turnstile_func_values(code: i64, args: &[TurnstileValue], result: &mut String) {
    if code == 3 {
        if let Some(value) = args.first() {
            *result = BASE64_STANDARD.encode(turnstile_to_string(value).as_bytes());
        }
    }
}

fn turnstile_value_from_json(value: &Value) -> TurnstileValue {
    match value {
        Value::Null => TurnstileValue::Null,
        Value::Bool(value) => TurnstileValue::Bool(*value),
        Value::Number(value) => TurnstileValue::Number(value.as_f64().unwrap_or_default()),
        Value::String(value) => TurnstileValue::String(value.clone()),
        Value::Array(values) => {
            TurnstileValue::Array(values.iter().map(turnstile_value_from_json).collect())
        }
        Value::Object(values) => TurnstileValue::Object(
            values
                .iter()
                .map(|(key, value)| (key.clone(), turnstile_value_from_json(value)))
                .collect(),
        ),
    }
}

fn turnstile_value_to_json(value: &TurnstileValue) -> Option<Value> {
    match value {
        TurnstileValue::Undefined | TurnstileValue::Func(_) => Some(Value::Null),
        TurnstileValue::Null => Some(Value::Null),
        TurnstileValue::Bool(value) => Some(json!(*value)),
        TurnstileValue::Number(value) => serde_json::Number::from_f64(*value).map(Value::Number),
        TurnstileValue::String(value) => Some(Value::String(value.clone())),
        TurnstileValue::Array(values) => values
            .iter()
            .map(turnstile_value_to_json)
            .collect::<Option<Vec<_>>>()
            .map(Value::Array),
        TurnstileValue::Object(values) => {
            let mut map = serde_json::Map::new();
            for (key, value) in values {
                map.insert(key.clone(), turnstile_value_to_json(value)?);
            }
            Some(Value::Object(map))
        }
        TurnstileValue::OrderedMap(map) => {
            let mut object = serde_json::Map::new();
            for (key, value) in &map.entries {
                object.insert(key.clone(), turnstile_value_to_json(value)?);
            }
            Some(Value::Object(object))
        }
    }
}

fn turnstile_key_from_json(value: &Value) -> Option<String> {
    if let Some(value) = value.as_i64() {
        return Some(turnstile_key_from_i64(value));
    }
    if let Some(value) = value.as_u64() {
        return Some(value.to_string());
    }
    value.as_f64().map(turnstile_key_from_f64)
}

fn turnstile_key_from_i64(value: i64) -> String {
    value.to_string()
}

fn turnstile_key_from_f64(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

fn turnstile_to_string(value: &TurnstileValue) -> String {
    match value {
        TurnstileValue::Undefined | TurnstileValue::Null => "undefined".to_string(),
        TurnstileValue::Bool(value) => {
            if *value {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        TurnstileValue::Number(value) => format_turnstile_number(*value),
        TurnstileValue::String(value) => match value.as_str() {
            "window.Math" => "[object Math]".to_string(),
            "window.Reflect" => "[object Reflect]".to_string(),
            "window.performance" => "[object Performance]".to_string(),
            "window.localStorage" => "[object Storage]".to_string(),
            "window.Object" => "function Object() { [native code] }".to_string(),
            "window.Reflect.set" => "function set() { [native code] }".to_string(),
            "window.performance.now" => "function () { [native code] }".to_string(),
            "window.Object.create" => "function create() { [native code] }".to_string(),
            "window.Object.keys" => "function keys() { [native code] }".to_string(),
            "window.Math.random" => "function random() { [native code] }".to_string(),
            _ => value.clone(),
        },
        TurnstileValue::Array(values)
            if values
                .iter()
                .all(|value| matches!(value, TurnstileValue::String(_))) =>
        {
            values
                .iter()
                .map(turnstile_to_string)
                .collect::<Vec<_>>()
                .join(",")
        }
        TurnstileValue::Array(values) => {
            let inner = values
                .iter()
                .map(turnstile_to_string)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        TurnstileValue::Object(values) => {
            let inner = values
                .iter()
                .map(|(key, value)| format!("'{key}': {}", turnstile_to_string(value)))
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
        TurnstileValue::OrderedMap(_) => "<OrderedMap>".to_string(),
        TurnstileValue::Func(code) => format!("<function {code}>"),
    }
}

fn turnstile_is_string_or_number(value: &TurnstileValue) -> bool {
    matches!(value, TurnstileValue::String(_) | TurnstileValue::Number(_))
}

fn format_turnstile_number(value: f64) -> String {
    if value.is_finite() && value.fract() == 0.0 {
        format!("{value:.0}")
    } else {
        format!("{value}")
    }
}

fn py_json_dumps_turnstile(value: &TurnstileValue) -> String {
    match value {
        TurnstileValue::Undefined | TurnstileValue::Null | TurnstileValue::Func(_) => {
            "null".to_string()
        }
        TurnstileValue::Bool(value) => value.to_string(),
        TurnstileValue::Number(value) => format_turnstile_number(*value),
        TurnstileValue::String(value) => {
            serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
        }
        TurnstileValue::Array(values) => {
            let inner = values
                .iter()
                .map(py_json_dumps_turnstile)
                .collect::<Vec<_>>()
                .join(", ");
            format!("[{inner}]")
        }
        TurnstileValue::Object(values) => {
            let inner = values
                .iter()
                .map(|(key, value)| {
                    let key = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                    format!("{key}: {}", py_json_dumps_turnstile(value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
        TurnstileValue::OrderedMap(map) => {
            let inner = map
                .entries
                .iter()
                .map(|(key, value)| {
                    let key = serde_json::to_string(key).unwrap_or_else(|_| "\"\"".to_string());
                    format!("{key}: {}", py_json_dumps_turnstile(value))
                })
                .collect::<Vec<_>>()
                .join(", ");
            format!("{{{inner}}}")
        }
    }
}

fn xor_string(text: &str, key: &str) -> Option<String> {
    if key.is_empty() {
        return Some(text.to_string());
    }
    let key = key.as_bytes();
    let bytes = text
        .as_bytes()
        .iter()
        .enumerate()
        .map(|(index, value)| value ^ key[index % key.len()])
        .collect::<Vec<_>>();
    String::from_utf8(bytes).ok()
}

fn decode_base64_compat(text: &str) -> Option<Vec<u8>> {
    let mut normalized = text.trim().to_string();
    while normalized.len() % 4 != 0 {
        normalized.push('=');
    }
    BASE64_STANDARD.decode(normalized).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bootstrap() -> ChatGptWebBootstrap {
        ChatGptWebBootstrap {
            pow_script_sources: vec!["https://chatgpt.com/backend-api/sentinel/sdk.js".to_string()],
            pow_data_build: Some("c/example/_".to_string()),
        }
    }

    #[test]
    fn build_legacy_requirements_token_returns_prefixed_token() {
        let token = build_legacy_requirements_token(&bootstrap(), "Mozilla/5.0 test");
        assert!(token.starts_with("gAAAAAC"));
        assert!(token.len() > "gAAAAAC".len());
    }

    #[test]
    fn pow_generate_emits_legacy_browser_json_shape() {
        let config = vec![
            json!(3000),
            json!("Mon Jan 02 2006 15:04:05 GMT-0500 (Eastern Standard Time)"),
            json!(4294705152u64),
            json!(0),
            json!("Mozilla/5.0 test"),
            json!("https://chatgpt.com/backend-api/sentinel/sdk.js"),
            json!("c/example/_"),
            json!("en-US"),
            json!("en-US,es-US,en,es"),
            json!(0),
            json!("vendor−Google Inc."),
            json!("location"),
            json!("window"),
            json!(123.0),
            json!("00000000-0000-0000-0000-000000000000"),
            json!(""),
            json!(8),
            json!(456.0),
        ];
        let (answer, solved) = pow_generate("seed", "ff", &config, 1);
        assert!(solved);
        let decoded = BASE64_STANDARD.decode(answer).expect("base64");
        let value: Value = serde_json::from_slice(&decoded).expect("legacy browser json array");
        let items = value.as_array().expect("array");
        assert_eq!(items.len(), 18);
        assert_eq!(items[3], json!(0));
        assert_eq!(items[9], json!(0));
        assert_eq!(items[4], json!("Mozilla/5.0 test"));
    }

    #[test]
    fn solve_turnstile_token_decodes_xored_program() {
        let p = "gAAAAAC-test-seed";
        let program = json!([[3, "turnstile-ok"]]);
        let program_text = serde_json::to_string(&program).expect("program json");
        let xored = xor_string(&program_text, p).expect("xor");
        let dx = BASE64_STANDARD.encode(xored.as_bytes());

        assert_eq!(
            solve_turnstile_token(&dx, p),
            Some(BASE64_STANDARD.encode("turnstile-ok"))
        );
    }

    #[test]
    fn solve_turnstile_token_handles_browser_vm_primitives() {
        let program = json!([
            [2, 30, "window"],
            [2, 31, "document"],
            [24, 32, 30, 31],
            [2, 33, "location"],
            [6, 34, 32, 33],
            [8, 35, 34],
            [19, 35],
            [20, 34, 34, 3, 35]
        ]);
        let program_text = serde_json::to_string(&program).expect("program json");
        let dx = BASE64_STANDARD.encode(program_text.as_bytes());
        let location = "https://chatgpt.com/";
        let once_encoded = BASE64_STANDARD.encode(location);

        assert_eq!(
            solve_turnstile_token(&dx, ""),
            Some(BASE64_STANDARD.encode(once_encoded))
        );
    }

    #[test]
    fn solve_turnstile_token_executes_second_stage_program() {
        let second_stage = json!([[3, "stage-ok"]]);
        let second_stage_text = serde_json::to_string(&second_stage).expect("stage json");
        let program = json!([
            [2, 20, BASE64_STANDARD.encode(second_stage_text.as_bytes())],
            [18, 20],
            [14, 9, 20]
        ]);
        let dx = BASE64_STANDARD.encode(
            serde_json::to_string(&program)
                .expect("program json")
                .as_bytes(),
        );

        assert_eq!(
            solve_turnstile_token(&dx, ""),
            Some(BASE64_STANDARD.encode("stage-ok"))
        );
    }

    #[test]
    fn build_proof_token_returns_prefixed_token_for_easy_difficulty() {
        let token =
            build_proof_token(&bootstrap(), "Mozilla/5.0 test", "seed", "ff").expect("proof token");
        assert!(token.starts_with("gAAAAAB"));
        assert!(token.len() > "gAAAAAB".len());
    }

    #[test]
    fn build_proof_token_errors_for_impossible_difficulty_window() {
        let error = build_proof_token(&bootstrap(), "Mozilla/5.0 test", "seed", "0000000000000000")
            .expect_err("proof failure");

        assert_eq!(
            error.code.as_deref(),
            Some("chatgpt_web_proof_token_failed")
        );
    }
}
