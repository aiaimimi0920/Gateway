//! Request-local Turnstile program state and opcode execution.
use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde_json::Value;
use std::time::Instant;

mod budget;
use budget::TurnstileState;

use super::values::{
    decode_base64_compat, py_json_dumps_turnstile, turnstile_is_string_or_number,
    turnstile_key_from_i64, turnstile_key_from_json, turnstile_to_string,
    turnstile_value_from_json, turnstile_value_to_json, xor_string, OrderedTurnstileMap,
    TurnstileValue,
};

pub fn solve_turnstile_token(dx: &str, p: &str) -> Option<String> {
    if !budget::valid_input(dx, p) {
        return None;
    }
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
            if !process_map.admit_instruction() {
                return None;
            }
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
            if process_map.failed() {
                return None;
            }
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

    if result.is_empty() || process_map.failed() {
        None
    } else {
        Some(result)
    }
}

fn initial_turnstile_process_map(program: &Value, p: &str) -> TurnstileState {
    let mut process_map = TurnstileState::new();
    if !process_map.admit_json(program) {
        return process_map;
    }
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
    process_map: &mut TurnstileState,
    start: &Instant,
    result: &mut String,
) {
    if !process_map.enter_call() {
        return;
    }
    call_turnstile_func_inner(code, args, process_map, start, result);
    process_map.leave_call();
}

fn call_turnstile_func_inner(
    code: i64,
    args: &[Value],
    process_map: &mut TurnstileState,
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
                process_map.write_result(result, &value);
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
            serde_json::from_str::<Value>(&text).ok().and_then(|value| {
                process_map
                    .admit_json(&value)
                    .then(|| turnstile_value_from_json(&value))
            })
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
                    call_turnstile_func_values(code, &values, process_map, result);
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
    process_map: &mut TurnstileState,
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
        if let Some(TurnstileValue::OrderedMap(mut object)) = process_map.get(&object_key).cloned()
        {
            object.add(key_name, value);
            process_map.insert(object_key, TurnstileValue::OrderedMap(object));
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
            if process_map.enter_call() {
                call_turnstile_dynamic(args, process_map, start);
                process_map.leave_call();
            }
        } else {
            call_turnstile_func_values(code, &values, process_map, result);
        }
    }
}

fn call_turnstile_dynamic(args: &[Value], process_map: &mut TurnstileState, start: &Instant) {
    let (Some(e), Some(t)) = (
        args.first().and_then(turnstile_key_from_json),
        args.get(1).and_then(turnstile_key_from_json),
    ) else {
        return;
    };
    // Dispatch uses the raw locator; display conversion rewrites builtin names.
    let Some(TurnstileValue::String(target)) = process_map.get(&t).cloned() else {
        return;
    };
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

fn call_turnstile_func_values(
    code: i64,
    args: &[TurnstileValue],
    process_map: &mut TurnstileState,
    result: &mut String,
) {
    if !process_map.enter_call() {
        return;
    }
    if code == 3 {
        if let Some(value) = args.first() {
            process_map.write_result(result, value);
        }
    }
    process_map.leave_call();
}

#[cfg(test)]
mod resource_tests;
