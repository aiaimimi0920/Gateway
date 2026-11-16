//! Embeddings report input-only usage; do not infer zero output for a conversational endpoint.
use crate::protocol::canonical::TokenUsage;
use serde_json::Value;

pub(crate) fn parse_embeddings_usage(body: &Value) -> Option<TokenUsage> {
    let usage = body.get("usage")?;
    let prompt_tokens = usage.get("prompt_tokens")?.as_u64()?;
    let total_tokens = usage.get("total_tokens")?.as_u64()?;
    if usage
        .get("completion_tokens")
        .is_some_and(|v| v.as_u64() != Some(0))
    {
        return None;
    }
    Some(TokenUsage {
        prompt_tokens,
        completion_tokens: 0,
        total_tokens,
        cache_creation_input_tokens: None,
        cache_read_input_tokens: usage
            .get("prompt_tokens_details")
            .and_then(|details| details.get("cached_tokens"))
            .and_then(Value::as_u64),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn embeddings_usage_is_input_only_and_keeps_reported_total() {
        for count in [0, 2] {
            let body = json!({"usage":{"prompt_tokens":count,"total_tokens":count}});
            let usage = parse_embeddings_usage(&body).unwrap();
            assert_eq!(
                (
                    usage.prompt_tokens,
                    usage.completion_tokens,
                    usage.total_tokens
                ),
                (count, 0, count)
            );
            assert!(crate::redis::usage_tracking::parse_upstream_usage(&body, "openai").is_none());
        }
    }

    #[test]
    fn embeddings_usage_does_not_invent_missing_or_invalid_counts() {
        for body in [
            json!({}),
            json!({"usage":{}}),
            json!({"usage":{"prompt_tokens":2}}),
            json!({"usage":{"prompt_tokens":-1,"total_tokens":2}}),
            json!({"usage":{"prompt_tokens":2,"total_tokens":"2"}}),
            json!({"usage":{"prompt_tokens":2,"total_tokens":2,"completion_tokens":1}}),
        ] {
            assert!(parse_embeddings_usage(&body).is_none(), "{body}");
        }
    }
}
