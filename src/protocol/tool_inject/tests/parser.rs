use super::*;

// ── parse_tool_calls_from_text — <tool_calls> format ────────────────

#[test]
fn parse_tool_calls_format() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/test.txt"}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("read_file"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"path\": \"/tmp/test.txt\"}")
    );
    assert!(result.clean_text.is_empty());
}

#[test]
fn parse_tool_calls_format_accepts_tool_tag_alias() {
    let text = r#"<tool_calls>
<tool_call>
<tool>weather</tool>
<parameters>{"city": "Hangzhou"}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"city\": \"Hangzhou\"}")
    );
}

#[test]
fn parse_tool_calls_recovers_name_from_specific_tool_choice() {
    let tools = vec![
        make_tool(
            "weather",
            "Read weather",
            json!({
                "type": "object",
                "properties": {
                    "location": {"type": "string"}
                },
                "required": ["location"]
            }),
        ),
        make_tool(
            "calendar",
            "Read calendar",
            json!({
                "type": "object",
                "properties": {
                    "date": {"type": "string"}
                },
                "required": ["date"]
            }),
        ),
    ];
    let text = r#"<tool_calls>
<tool_call>
<unnamed></unnamed>
<parameters>{"location": "Hangzhou"}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text_with_context(
        text,
        &tools,
        Some(&json!({"type": "function", "function": {"name": "weather"}})),
        None,
    );
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
}

#[test]
fn parse_tool_calls_recovers_name_from_single_tool_context() {
    let tools = vec![make_tool(
        "weather",
        "Read weather",
        json!({
            "type": "object",
            "properties": {
                "location": {"type": "string"}
            },
            "required": ["location"]
        }),
    )];
    let text = r#"<tool_calls>
<tool_call>
<unnamed></unnamed>
<parameters>{"location": "Hangzhou"}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text_with_context(text, &tools, None, None);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
}

#[test]
fn parse_tool_calls_recovers_name_from_conversation_hint() {
    let tools = vec![
        make_tool(
            "weather",
            "Read weather",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        ),
        make_tool(
            "calendar",
            "Read calendar",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        ),
    ];
    let text = r#"<tool_calls>
<tool_call>
<tool_name>unnamed</tool_name>
<parameters>{"city": "Hangzhou"}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text_with_context(
        text,
        &tools,
        Some(&json!("required")),
        Some("Use only the weather tool for Hangzhou."),
    );
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
}

#[test]
fn parse_tool_calls_with_preceding_text() {
    let text = r#"I'll read that file for you.

<tool_calls>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/test.txt"}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.clean_text, "I'll read that file for you.");
}

#[test]
fn parse_tool_calls_recovers_from_malformed_closing_tag() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>weather</tool_name>
<parameters>{"city":"Hangzhou"}</parameters>
</arg_value>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
}

#[test]
fn parse_tool_calls_recovers_when_outer_block_is_not_closed() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>weather</tool_name>
<parameters>{"city":"Hangzhou"}</parameters>
</arg_value>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
    assert_eq!(result.clean_text, "");
}

#[test]
fn parse_empty_tool_block_infers_specific_tool_name_from_context() {
    let tools = vec![
        make_tool(
            "weather",
            "Read weather",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        ),
        make_tool(
            "calendar",
            "Read calendar",
            json!({
                "type": "object",
                "properties": {
                    "city": {"type": "string"}
                },
                "required": ["city"]
            }),
        ),
    ];
    let text = "<tool_calls>\n\n</tool_calls>";

    let result = parse_tool_calls_from_text_with_context(
        text,
        &tools,
        Some(&json!("required")),
        Some("Use only the weather tool for Hangzhou."),
    );

    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("{}"));
}

#[test]
fn parse_tool_calls_recovers_inline_json_without_parameters_tag() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>weather</tool_name>{"city":"San Francisco"}
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"San Francisco\"}")
    );
}

#[test]
fn parse_opening_only_tool_block_infers_name_from_context() {
    let tools = vec![make_tool(
        "weather",
        "Read weather",
        json!({
            "type": "object",
            "properties": {
                "city": {"type": "string"}
            },
            "required": ["city"]
        }),
    )];
    let result = parse_tool_calls_from_text_with_context(
        "<tool_calls>\n",
        &tools,
        Some(&json!("required")),
        Some("Use only the weather tool for Hangzhou."),
    );

    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("{}"));
}

#[test]
fn parse_garbled_tool_block_closer_still_infers_name_from_context() {
    let tools = vec![make_tool(
        "weather",
        "Read weather",
        json!({
            "type": "object",
            "properties": {
                "city": {"type": "string"}
            },
            "required": ["city"]
        }),
    )];
    let result = parse_tool_calls_from_text_with_context(
        "<tool_calls>\n</</tool_calls>",
        &tools,
        Some(&json!("required")),
        Some("Use only the weather tool for Hangzhou."),
    );

    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("weather"));
    assert_eq!(result.tool_calls[0].arguments.as_deref(), Some("{}"));
}

// ── parse_tool_calls_from_text — <function_calls> format ────────────

#[test]
fn parse_function_calls_format() {
    let text = r#"<function_calls>
<function_call>
<tool>get_weather</tool>
<args_json>{"location": "NYC", "units": "metric"}</args_json>
</function_call>
</function_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("get_weather"));
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"location\": \"NYC\", \"units\": \"metric\"}")
    );
}

#[test]
fn parse_tool_calls_unescapes_xml_encoded_arguments() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>weather</tool_name>
<parameters>{&quot;city&quot;:&quot;Hangzhou&quot;}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(
        result.tool_calls[0].arguments.as_deref(),
        Some("{\"city\":\"Hangzhou\"}")
    );
}

// ── parse_tool_calls_from_text — <invoke> format ────────────────────

#[test]
fn parse_invoke_format() {
    let text = r#"<invoke name="search_web">
<parameter name="query">rust programming</parameter>
<parameter name="limit">10</parameter>
</invoke>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("search_web"));

    let args: Value =
        serde_json::from_str(result.tool_calls[0].arguments.as_deref().unwrap()).unwrap();
    assert_eq!(args["query"], "rust programming");
    assert_eq!(args["limit"], "10");
}

// ── parse_tool_calls_from_text — no tool calls ──────────────────────

#[test]
fn parse_no_tool_calls() {
    let text = "Just a regular response with no tool calls.";
    let result = parse_tool_calls_from_text(text);
    assert!(!result.had_tool_calls);
    assert!(result.tool_calls.is_empty());
    assert_eq!(result.clean_text, text);
}

// ── Multiple tool calls in one response ─────────────────────────────

#[test]
fn parse_multiple_tool_calls() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/a.txt"}</parameters>
</tool_call>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/b.txt"}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 2);
    assert_eq!(result.tool_calls[0].name.as_deref(), Some("read_file"));
    assert_eq!(result.tool_calls[1].name.as_deref(), Some("read_file"));

    let args0: Value =
        serde_json::from_str(result.tool_calls[0].arguments.as_deref().unwrap()).unwrap();
    let args1: Value =
        serde_json::from_str(result.tool_calls[1].arguments.as_deref().unwrap()).unwrap();
    assert_eq!(args0["path"], "/tmp/a.txt");
    assert_eq!(args1["path"], "/tmp/b.txt");
}

// ── Complex JSON parameters ─────────────────────────────────────────

#[test]
fn parse_complex_json_parameters() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>execute_query</tool_name>
<parameters>{"query": "SELECT * FROM users", "options": {"limit": 10, "offset": 0}, "tags": ["prod", "readonly"]}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);

    let args: Value =
        serde_json::from_str(result.tool_calls[0].arguments.as_deref().unwrap()).unwrap();
    assert_eq!(args["query"], "SELECT * FROM users");
    assert_eq!(args["options"]["limit"], 10);
    assert_eq!(args["tags"][0], "prod");
}

// ── strip_think_blocks ──────────────────────────────────────────────

#[test]
fn strip_think_simple() {
    let text = "<think>internal reasoning</think>The answer is 42.";
    let result = strip_think_blocks(text);
    assert_eq!(result, "The answer is 42.");
}

#[test]
fn strip_think_nested() {
    let text = "<think>outer <think>inner</think> still thinking</think>Done.";
    let result = strip_think_blocks(text);
    assert_eq!(result, "Done.");
}

#[test]
fn strip_think_no_think_blocks() {
    let text = "No thinking here.";
    let result = strip_think_blocks(text);
    assert_eq!(result, "No thinking here.");
}

#[test]
fn strip_think_preserves_utf8_without_think_blocks() {
    let text = "法国的首都是巴黎（Paris）。";
    let result = strip_think_blocks(text);
    assert_eq!(result, text);
}

#[test]
fn strip_think_preserves_utf8_around_removed_block() {
    let text = "法国的首都<think>先分析一下</think>是巴黎。";
    let result = strip_think_blocks(text);
    assert_eq!(result, "法国的首都是巴黎。");
}

#[test]
fn strip_think_with_tool_calls() {
    let text = r#"<think>Let me think about which tool to call...</think>
<tool_calls>
<tool_call>
<tool_name>read_file</tool_name>
<parameters>{"path": "/tmp/test.txt"}</parameters>
</tool_call>
</tool_calls>"#;

    let cleaned = strip_think_blocks(text);
    assert!(!cleaned.contains("<think>"));
    assert!(cleaned.contains("<tool_calls>"));

    let result = parse_tool_calls_from_text(text);
    assert!(result.had_tool_calls);
    assert_eq!(result.tool_calls.len(), 1);
}

// ── Tool call IDs ───────────────────────────────────────────────────

#[test]
fn parsed_tool_calls_have_unique_ids() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>tool_a</tool_name>
<parameters>{}</parameters>
</tool_call>
<tool_call>
<tool_name>tool_b</tool_name>
<parameters>{}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert_eq!(result.tool_calls.len(), 2);

    let id_a = result.tool_calls[0].id.as_ref().unwrap();
    let id_b = result.tool_calls[1].id.as_ref().unwrap();
    assert_ne!(id_a, id_b);
    assert!(id_a.starts_with("call_"));
    assert!(id_b.starts_with("call_"));
}

// ── call_type is always "function" ──────────────────────────────────

#[test]
fn parsed_tool_calls_have_function_type() {
    let text = r#"<tool_calls>
<tool_call>
<tool_name>my_tool</tool_name>
<parameters>{"x": 1}</parameters>
</tool_call>
</tool_calls>"#;

    let result = parse_tool_calls_from_text(text);
    assert_eq!(result.tool_calls[0].call_type, "function");
}
