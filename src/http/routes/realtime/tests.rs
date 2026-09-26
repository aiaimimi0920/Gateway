use serde_json::json;

use super::parse_openai_session_tools;

#[test]
fn parse_openai_session_tools_accepts_responses_function_shape() {
    let raw = json!({
        "model": "gpt-5.4",
        "tools": [
            {
                "type": "function",
                "name": "weather",
                "description": "Return the weather for a city.",
                "parameters": {
                    "type": "object",
                    "properties": {
                        "city": { "type": "string" }
                    },
                    "required": ["city"]
                }
            }
        ],
        "tool_choice": "required"
    });

    let (tools, tool_choice) = parse_openai_session_tools(&raw).expect("tools should parse");
    assert_eq!(tools.len(), 1);
    assert_eq!(tools[0].name.as_deref(), Some("weather"));
    assert_eq!(tool_choice, Some(json!("required")));
}
