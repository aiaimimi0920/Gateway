use serde_json::{json, Value};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CanonicalToolChoice {
    Auto,
    None,
    Required,
    Specific(String),
    PromptOnly,
}

impl CanonicalToolChoice {
    pub fn semantics_label(&self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::None => "none",
            Self::Required => "required",
            Self::Specific(_) => "specific_function",
            Self::PromptOnly => "prompt_only",
        }
    }
}

pub fn canonicalize_tool_choice(raw: Option<&Value>) -> Option<Value> {
    let choice = parse_tool_choice(raw)?;
    Some(match choice {
        CanonicalToolChoice::Auto => json!("auto"),
        CanonicalToolChoice::None => json!("none"),
        CanonicalToolChoice::Required => json!("required"),
        CanonicalToolChoice::Specific(name) => {
            json!({"type": "function", "function": {"name": name}})
        }
        CanonicalToolChoice::PromptOnly => json!("prompt_only"),
    })
}

pub fn parse_tool_choice(raw: Option<&Value>) -> Option<CanonicalToolChoice> {
    let raw = raw?;

    if let Some(text) = raw.as_str() {
        return match text.trim() {
            "auto" | "AUTO" => Some(CanonicalToolChoice::Auto),
            "none" | "NONE" => Some(CanonicalToolChoice::None),
            "required" | "REQUIRED" | "any" | "ANY" => Some(CanonicalToolChoice::Required),
            _ => None,
        };
    }

    let object = raw.as_object()?;

    if let Some(mode) = object.get("mode").and_then(|value| value.as_str()) {
        return match mode {
            "AUTO" => Some(CanonicalToolChoice::Auto),
            "NONE" => Some(CanonicalToolChoice::None),
            "ANY" => object
                .get("allowedFunctionNames")
                .and_then(|value| value.as_array())
                .and_then(|value| value.first())
                .and_then(|value| value.as_str())
                .filter(|value| !value.trim().is_empty())
                .map(|value| CanonicalToolChoice::Specific(value.to_string()))
                .or(Some(CanonicalToolChoice::Required)),
            _ => None,
        };
    }

    if object.get("auto").is_some() {
        return Some(CanonicalToolChoice::Auto);
    }
    if object.get("any").is_some() {
        return Some(CanonicalToolChoice::Required);
    }
    if let Some(name) = object
        .get("tool")
        .and_then(|value| value.get("name"))
        .and_then(|value| value.as_str())
        .filter(|value| !value.trim().is_empty())
    {
        return Some(CanonicalToolChoice::Specific(name.to_string()));
    }

    match object
        .get("type")
        .and_then(|value| value.as_str())
        .unwrap_or("")
    {
        "auto" => Some(CanonicalToolChoice::Auto),
        "none" => Some(CanonicalToolChoice::None),
        "any" | "required" => Some(CanonicalToolChoice::Required),
        "tool" => object
            .get("name")
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(|value| CanonicalToolChoice::Specific(value.to_string())),
        "function" => object
            .get("function")
            .and_then(|value| value.get("name"))
            .or_else(|| object.get("name"))
            .and_then(|value| value.as_str())
            .filter(|value| !value.trim().is_empty())
            .map(|value| CanonicalToolChoice::Specific(value.to_string())),
        _ => None,
    }
}

pub fn pack_gemini_tool_choice(raw: Option<&Value>) -> Option<Value> {
    let choice = parse_tool_choice(raw)?;
    Some(match choice {
        CanonicalToolChoice::Auto => json!({"mode": "AUTO"}),
        CanonicalToolChoice::None => json!({"mode": "NONE"}),
        CanonicalToolChoice::Required => json!({"mode": "ANY"}),
        CanonicalToolChoice::Specific(name) => {
            json!({"mode": "ANY", "allowedFunctionNames": [name]})
        }
        CanonicalToolChoice::PromptOnly => json!({"mode": "NONE"}),
    })
}

pub fn pack_bedrock_tool_choice(raw: Option<&Value>) -> Option<Value> {
    let choice = parse_tool_choice(raw)?;
    Some(match choice {
        CanonicalToolChoice::Auto | CanonicalToolChoice::None => json!({"auto": {}}),
        CanonicalToolChoice::Required => json!({"any": {}}),
        CanonicalToolChoice::Specific(name) => json!({"tool": {"name": name}}),
        CanonicalToolChoice::PromptOnly => json!({"auto": {}}),
    })
}

pub fn pack_cohere_tool_choice(raw: Option<&Value>) -> Option<Value> {
    let choice = parse_tool_choice(raw)?;
    Some(match choice {
        CanonicalToolChoice::Auto => json!("AUTO"),
        CanonicalToolChoice::None => json!("NONE"),
        CanonicalToolChoice::Required => json!("REQUIRED"),
        CanonicalToolChoice::Specific(name) => json!({"type": "function", "name": name}),
        CanonicalToolChoice::PromptOnly => json!("NONE"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_gemini_any_to_required() {
        let raw = json!({"mode": "ANY"});
        assert_eq!(
            canonicalize_tool_choice(Some(&raw)),
            Some(json!("required"))
        );
    }

    #[test]
    fn parse_openai_required() {
        assert_eq!(
            parse_tool_choice(Some(&json!("required"))),
            Some(CanonicalToolChoice::Required)
        );
    }

    #[test]
    fn parse_anthropic_specific_tool() {
        assert_eq!(
            parse_tool_choice(Some(&json!({"type": "tool", "name": "weather"}))),
            Some(CanonicalToolChoice::Specific("weather".to_string()))
        );
    }

    #[test]
    fn pack_bedrock_specific_tool() {
        let raw = json!({"type": "function", "function": {"name": "weather"}});
        assert_eq!(
            pack_bedrock_tool_choice(Some(&raw)),
            Some(json!({"tool": {"name": "weather"}}))
        );
    }

    #[test]
    fn prompt_only_semantics_label_is_exposed() {
        assert_eq!(
            CanonicalToolChoice::PromptOnly.semantics_label(),
            "prompt_only"
        );
    }
}
