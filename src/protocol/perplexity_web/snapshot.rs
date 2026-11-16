//! 只解析可归属的完整快照；patch 的 op/path wire 未冻结，不能猜测 append。
use super::{failure, Answer, Source};
use crate::error::GatewayError;
use serde_json::Value;

const ANSWER_LIMIT: usize = 256 * 1024;
const SOURCE_LIMIT: usize = 64;
const BLOCK_LIMIT: usize = 128;

fn invalid() -> GatewayError {
    failure(
        "perplexity_web_invalid_snapshot",
        "Perplexity Web 答案或引用快照无效；未返回原始事件。",
    )
}

pub(super) fn extract(raw: &Value) -> Result<Option<Answer>, GatewayError> {
    if !raw.is_object() || raw.get("error").is_some_and(|v| !v.is_null()) {
        return Err(invalid());
    }
    let mut text = None;
    // 仅交付最终答案同一事件中的来源列表，不跨快照推定引用归属。
    let mut sources = Vec::new();
    let mut source_seen = false;
    if let Some(blocks) = raw.get("blocks") {
        let blocks = blocks
            .as_array()
            .filter(|v| v.len() <= BLOCK_LIMIT)
            .ok_or_else(invalid)?;
        for block in blocks {
            if !block.is_object() {
                return Err(invalid());
            }
            if block
                .get("diff_block")
                .is_some_and(|value| !value.is_null())
            {
                return Err(failure(
                    "perplexity_web_unsupported_patch",
                    "Perplexity Web diff/patch 格式尚未冻结；不会将其猜作文本增量。",
                ));
            }
            match block["intended_usage"].as_str() {
                Some("ask_text") => {
                    if text.is_some() {
                        return Err(invalid());
                    }
                    text = Some(answer_text(&block["markdown_block"]["answer"])?);
                }
                Some("ask_text_0_markdown") => {
                    return Err(failure(
                        "perplexity_web_unsupported_profile",
                        "Perplexity Web 返回另一响应 profile；未混用 schematized 与 auto 快照。",
                    ));
                }
                Some("web_results") => {
                    if source_seen {
                        return Err(invalid());
                    }
                    source_seen = true;
                    sources = extract_sources(&block["web_result_block"]["web_results"])?;
                }
                // 搜索步骤、卡片、图片和未知扩展不能充当 assistant 答案。
                _ => {}
            }
        }
    }
    if let Some(legacy) = raw.get("text").filter(|v| !v.is_null() && *v != "") {
        if let Some(legacy_text) = legacy_final(legacy)? {
            if text.is_some() {
                return Err(invalid());
            }
            text = Some(legacy_text);
        }
    }
    Ok(text.map(|text| Answer { text, sources }))
}

fn answer_text(value: &Value) -> Result<String, GatewayError> {
    value
        .as_str()
        .filter(|s| s.len() <= ANSWER_LIMIT)
        .map(str::to_owned)
        .ok_or_else(invalid)
}

/// The public carrier contains only normalized answer/source fields, never native access metadata.
pub(super) fn public_answer(value: &Value) -> Result<Answer, GatewayError> {
    let object = value
        .as_object()
        .filter(|v| v.len() == 2)
        .ok_or_else(invalid)?;
    Ok(Answer {
        text: answer_text(object.get("text").ok_or_else(invalid)?)?,
        sources: extract_sources(object.get("sources").ok_or_else(invalid)?)?,
    })
}

fn legacy_final(value: &Value) -> Result<Option<String>, GatewayError> {
    let parsed;
    let steps = if let Some(text) = value.as_str() {
        parsed = serde_json::from_str::<Value>(text).map_err(|_| invalid())?;
        &parsed
    } else {
        value
    };
    let steps = steps
        .as_array()
        .filter(|steps| steps.len() <= BLOCK_LIMIT)
        .ok_or_else(invalid)?;
    let mut answer = None;
    for step in steps.iter().filter(|step| step["step_type"] == "FINAL") {
        if answer.is_some() {
            return Err(invalid());
        }
        let value = &step["content"]["answer"];
        let nested;
        let final_answer = if let Some(text) = value.as_str() {
            nested = serde_json::from_str::<Value>(text).map_err(|_| invalid())?;
            &nested
        } else {
            value
        };
        answer = Some(answer_text(&final_answer["answer"])?);
    }
    Ok(answer)
}

fn extract_sources(value: &Value) -> Result<Vec<Source>, GatewayError> {
    fn optional(value: &Value, limit: usize) -> Result<String, GatewayError> {
        match value {
            Value::Null => Ok(String::new()),
            Value::String(s) if s.len() <= limit => Ok(s.clone()),
            _ => Err(invalid()),
        }
    }
    let values = value
        .as_array()
        .filter(|v| v.len() <= SOURCE_LIMIT)
        .ok_or_else(invalid)?;
    values
        .iter()
        .map(|source| {
            let url = source["url"]
                .as_str()
                .filter(|url| url.len() <= 2048)
                .ok_or_else(invalid)?;
            let parsed = url::Url::parse(url).map_err(|_| invalid())?;
            if !matches!(parsed.scheme(), "http" | "https")
                || parsed.host_str().is_none()
                || !parsed.username().is_empty()
                || parsed.password().is_some()
            {
                return Err(invalid());
            }
            Ok(Source {
                name: optional(&source["name"], 1024)?,
                url: url.into(),
                snippet: optional(&source["snippet"], 8192)?,
            })
        })
        .collect()
}
