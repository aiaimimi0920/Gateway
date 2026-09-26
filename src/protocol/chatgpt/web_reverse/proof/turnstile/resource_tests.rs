use base64::{engine::general_purpose::STANDARD as BASE64_STANDARD, Engine};
use serde_json::{json, Value};

use crate::protocol::chatgpt::web_reverse::solve_turnstile_token;

fn encode(program: &Value) -> String {
    BASE64_STANDARD.encode(serde_json::to_vec(program).expect("fixture JSON"))
}

fn assert_token(program: Value, expected: &str) {
    assert_eq!(
        solve_turnstile_token(&encode(&program), ""),
        Some(BASE64_STANDARD.encode(expected))
    );
}

fn assert_rejected(program: Value) {
    assert!(
        solve_turnstile_token(&encode(&program), "").is_none(),
        "over-budget program must not produce a token"
    );
    assert_token(json!([[3, "fresh-request"]]), "fresh-request");
}

fn doubling(seed: Value, count: usize) -> Vec<Value> {
    let mut program = vec![json!([2, 30, seed])];
    program.extend((0..count).map(|_| json!([5, 30, 30])));
    program
}

fn conditional(depth: usize) -> Value {
    let mut instruction = vec![json!(23)];
    for _ in 1..depth {
        instruction.extend([json!(1), json!(23)]);
    }
    instruction.extend([json!(1), json!(3), json!("conditional-ok")]);
    json!([instruction])
}

fn stage(next: Value, ignored: usize) -> Value {
    let mut program = vec![json!([21]); ignored];
    program.push(json!([2, 30, serde_json::to_string(&next).unwrap()]));
    program.push(json!([14, 9, 30]));
    json!(program)
}

#[test]
fn rejects_raw_input_before_whitespace_normalization() {
    let dx = " ".repeat(2 * 1024 * 1024) + &encode(&json!([[3, "ok"]]));
    assert!(solve_turnstile_token(&dx, "").is_none());
}

#[test]
fn rejects_oversized_xor_key() {
    let dx = encode(&json!([[3, "ok"]]));
    assert!(solve_turnstile_token(&dx, &"\0".repeat(64 * 1024 + 1)).is_none());
}

#[test]
fn rejects_instruction_budget() {
    let mut program = vec![json!([21]); 8192];
    program.push(json!([3, "ok"]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_ignored_instruction_budget() {
    let mut program = vec![Value::Null; 8192];
    program.push(json!([3, "ok"]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_instruction_budget_across_stages() {
    let mut last = vec![json!([21]); 5000];
    last.push(json!([3, "ok"]));
    assert_rejected(stage(json!(last), 5000));
}

#[test]
fn rejects_recursive_dispatch_depth() {
    // Keep the failing-before implementation away from the test harness stack limit.
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| assert_rejected(conditional(80)))
        .expect("bounded fixture thread")
        .join()
        .expect("fixture thread joined");
}

#[test]
fn rejects_runtime_value_depth() {
    let mut program = vec![json!([2, 30, []])];
    for _ in 0..66 {
        program.extend([json!([2, 31, []]), json!([5, 31, 30]), json!([8, 30, 31])]);
    }
    program.push(json!([3, "ok"]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_parsed_value_depth_before_conversion() {
    let mut value = Value::Null;
    for _ in 0..66 {
        value = json!([value]);
    }
    assert_rejected(json!([[2, 30, value], [3, "ok"]]));
}

#[test]
fn rejects_parsed_value_node_budget() {
    assert_rejected(json!([[2, 30, vec![Value::Null; 32768]], [3, "ok"]]));
}

#[test]
fn rejects_string_doubling() {
    let mut program = doubling(json!("x"), 22);
    program.push(json!([3, "ok"]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_array_doubling() {
    let mut program = doubling(json!([]), 16);
    program.push(json!([3, "ok"]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_register_growth() {
    let mut program = (10000..14096)
        .map(|key| json!([2, key, null]))
        .collect::<Vec<_>>();
    program.push(json!([3, "ok"]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_cumulative_retained_bytes() {
    let mut program = vec![json!([2, 30, "x".repeat(256 * 1024)])];
    program.extend((100..140).map(|key| json!([8, key, 30])));
    program.push(json!([3, "ok"]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_repeated_copy_work() {
    let mut program = vec![json!([2, 30, "x".repeat(128 * 1024)])];
    program.extend((0..512).map(|_| json!([8, 30, 30])));
    program.push(json!([3, "ok"]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_encoded_output_growth() {
    let mut program = doubling(json!("1234567"), 18);
    program.push(json!([7, 3, 30]));
    assert_rejected(json!(program));
}

#[test]
fn rejects_exhaustion_after_earlier_result() {
    let mut program = vec![json!([3, "partial-result"])];
    program.extend(doubling(json!("x"), 22));
    assert_rejected(json!(program));
}

#[test]
fn preserves_input_and_key_limit_boundaries() {
    let dx = encode(&json!([[3, "boundary-ok"]]));
    let padded = " ".repeat(2 * 1024 * 1024 - dx.len()) + &dx;
    assert_eq!(
        solve_turnstile_token(&padded, ""),
        Some(BASE64_STANDARD.encode("boundary-ok"))
    );
    assert_eq!(
        solve_turnstile_token(&dx, &"\0".repeat(64 * 1024)),
        Some(BASE64_STANDARD.encode("boundary-ok"))
    );
}

#[test]
fn preserves_instruction_boundary_and_request_reset() {
    for _ in 0..2 {
        let mut program = vec![json!([21]); 8191];
        program.push(json!([3, "boundary-ok"]));
        assert_token(json!(program), "boundary-ok");
    }
}

#[test]
fn preserves_conditional_dispatch_and_null_guard() {
    assert_token(conditional(8), "conditional-ok");
    assert_token(
        json!([[23, 99999, 3, "ignored"], [3, "guard-ok"]]),
        "guard-ok",
    );
}

#[test]
fn preserves_array_and_string_concatenation() {
    assert_token(
        json!([
            [2, 30, []],
            [2, 31, "a"],
            [5, 30, 31],
            [2, 31, "b"],
            [5, 30, 31],
            [7, 3, 30]
        ]),
        "a,b",
    );
    assert_token(
        json!([[2, 30, "a"], [2, 31, 7], [5, 30, 31], [7, 3, 30]]),
        "a7",
    );
}

#[test]
fn preserves_ordered_map_replacement_and_dynamic_apply() {
    for create in [json!([17, 40, 31]), json!([7, 17, 31])] {
        let mut program = vec![
            json!([2, 30, "window.Reflect.set"]),
            json!([2, 31, "window.Object.create"]),
            create.clone(),
        ];
        if create[0] == 7 {
            program.push(json!([8, 40, 17]));
        }
        program.extend([
            json!([2, 41, "first"]),
            json!([2, 42, 1]),
            json!([7, 30, 40, 41, 42]),
            json!([2, 43, "second"]),
            json!([2, 44, 2]),
            json!([7, 30, 40, 43, 44]),
            json!([2, 42, 3]),
            json!([7, 30, 40, 41, 42]),
            json!([15, 45, 40]),
            json!([7, 3, 45]),
        ]);
        assert_token(json!(program), "{\"first\": 3, \"second\": 2}");
    }
}

#[test]
fn preserves_register_replacement_without_retention_leak() {
    let mut program = vec![json!([2, 10000, "replacement"]); 4096];
    program.push(json!([7, 3, 10000]));
    assert_token(json!(program), "replacement");
}

#[test]
fn preserves_four_stages_and_existing_fifth_stage_limit() {
    let mut program = json!([[3, "stage-ok"]]);
    for _ in 0..3 {
        program = stage(program, 0);
    }
    assert_token(program.clone(), "stage-ok");
    assert!(solve_turnstile_token(&encode(&stage(program, 0)), "").is_none());
}

#[test]
fn preserves_base64_padding_whitespace_and_unicode() {
    let program = json!([[3, "Unicode: \u{4e2d}\u{6587}"]]);
    let dx = encode(&program);
    for input in [dx.clone(), format!(" \n{}\t", dx.trim_end_matches('='))] {
        assert_eq!(
            solve_turnstile_token(&input, ""),
            Some(BASE64_STANDARD.encode("Unicode: \u{4e2d}\u{6587}"))
        );
    }
}

#[test]
fn preserves_malformed_wire_rejection() {
    for dx in [
        "%%%".to_string(),
        BASE64_STANDARD.encode("invalid JSON"),
        encode(&json!({})),
        BASE64_STANDARD.encode([0xff]),
    ] {
        assert!(solve_turnstile_token(&dx, "").is_none());
    }
}
