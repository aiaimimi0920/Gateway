use super::*;

// ── build_tool_injection_prompt ──────────────────────────────────────

#[test]
fn build_prompt_generates_correct_xml() {
    let tools = vec![make_tool(
        "read_file",
        "Read the contents of a file",
        json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "The file path to read"
                },
                "encoding": {
                    "type": "string",
                    "description": "File encoding, default utf-8"
                }
            },
            "required": ["path"]
        }),
    )];

    let prompt = build_tool_injection_prompt(&tools, None);

    assert!(prompt.contains("<tools>"));
    assert!(prompt.contains("</tools>"));
    assert!(prompt.contains("<tool name=\"read_file\">"));
    assert!(prompt.contains("Read the contents of a file"));
    assert!(prompt.contains("- path (string, required): The file path to read"));
    assert!(prompt.contains("- encoding (string, optional): File encoding, default utf-8"));
    assert!(prompt.contains("TOOL CALL FORMAT"));
    assert!(prompt.contains("<tool_calls>"));
    assert!(prompt.contains("<tool_name>"));
    assert!(prompt.contains("<parameters>"));
}

#[test]
fn build_prompt_handles_no_schema() {
    let tools = vec![CanonicalTool {
        tool_type: "function".to_string(),
        name: Some("simple_tool".to_string()),
        description: Some("A simple tool".to_string()),
        input_schema: None,
        raw: HashMap::new(),
    }];

    let prompt = build_tool_injection_prompt(&tools, None);
    assert!(prompt.contains("<tool name=\"simple_tool\">"));
    assert!(prompt.contains("A simple tool"));
    // Should not contain "Parameters:" when no schema
    assert!(!prompt.contains("Parameters:"));
}

#[test]
fn build_prompt_includes_required_tool_choice_guidance() {
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

    let prompt = build_tool_injection_prompt(&tools, Some(&json!("required")));
    assert!(prompt.contains("TOOL CHOICE REQUIREMENT"));
    assert!(prompt.contains("MUST call at least one tool"));
}

#[test]
fn build_prompt_includes_specific_tool_choice_guidance() {
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

    let prompt = build_tool_injection_prompt(
        &tools,
        Some(&json!({"type": "function", "function": {"name": "weather"}})),
    );
    assert!(prompt.contains("TOOL CHOICE REQUIREMENT"));
    assert!(prompt.contains("MUST call only the tool `weather`"));
}
