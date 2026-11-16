//! One response, one terminal sequence. Tail usage remains admissible until full upstream EOF.
use super::{frame::Output, invalid, tools::Tools, usage::Usage, UsageHandle};
use crate::protocol::{sse_parse::SseFrame, stream_error::ProtocolStreamError};
use serde_json::{json, Value};
use std::time::Instant;

pub(super) struct State {
    pub output: Output,
    tools: Tools,
    usage: Usage,
    started: bool,
    text: Option<u32>,
    next_block: u32,
    finish: Option<&'static str>,
    done: bool,
    started_at: Instant,
    validated_usage: UsageHandle,
    native: super::native::Native,
}

impl State {
    pub fn new(started_at: Instant, validated_usage: UsageHandle) -> Self {
        Self {
            output: Output::default(),
            tools: Tools::default(),
            usage: Usage::default(),
            started: false,
            text: None,
            next_block: 0,
            finish: None,
            done: false,
            started_at,
            validated_usage,
            native: Default::default(),
        }
    }

    pub fn observe(&mut self, frame: SseFrame) -> Result<(), ProtocolStreamError> {
        if frame.event_name.as_deref() == Some("error") {
            return Err(invalid("bedrock_stream_upstream_error: normalized_stream"));
        }
        if frame.data.is_empty() {
            return Ok(());
        }
        if self.done {
            return Err(invalid("Bedrock received an event after DONE"));
        }
        if frame.data == "[DONE]" {
            if self.native.active() {
                self.native.done()?;
                self.done = true;
                return Ok(());
            }
            if self.finish.is_none() {
                return Err(invalid("Bedrock received DONE before finish"));
            }
            self.done = true;
            return Ok(());
        }
        let raw: Value = serde_json::from_str(&frame.data)
            .map_err(|_| invalid("Bedrock received invalid normalized SSE JSON"))?;
        if raw.get("error").is_some_and(|value| !value.is_null()) {
            return Err(invalid("bedrock_stream_upstream_error: normalized_stream"));
        }
        if self.native.observe(&raw, &mut self.output)? {
            return Ok(());
        }
        if let Some(usage) = raw.get("usage").filter(|v| !v.is_null()) {
            self.usage.observe(usage)?;
        }
        let choices = raw
            .get("choices")
            .and_then(Value::as_array)
            .ok_or_else(|| invalid("Bedrock normalized SSE has no choices array"))?;
        if choices.len() > 1 {
            return Err(invalid("Bedrock cannot project multiple response choices"));
        }
        let Some(choice) = choices.first() else {
            return Ok(());
        };
        if choice.get("index").is_some_and(|v| v.as_u64() != Some(0)) {
            return Err(invalid("Bedrock response choice index is not zero"));
        }
        let delta = choice
            .get("delta")
            .and_then(Value::as_object)
            .ok_or_else(|| invalid("Bedrock normalized SSE delta is not an object"))?;
        if self.finish.is_some() && delta.values().any(|v| !v.is_null()) {
            return Err(invalid("Bedrock received content after finish"));
        }
        if delta.iter().any(|(key, value)| {
            !matches!(key.as_str(), "content" | "role" | "tool_calls") && !value.is_null()
        }) {
            return Err(invalid(
                "Bedrock normalized SSE contains unsupported content",
            ));
        }
        if delta
            .get("role")
            .is_some_and(|v| !v.is_null() && v != "assistant")
        {
            return Err(invalid("Bedrock response role is not assistant"));
        }
        if let Some(text) = delta.get("content").filter(|v| !v.is_null()) {
            let text = text
                .as_str()
                .ok_or_else(|| invalid("Bedrock text delta is not a string"))?;
            self.start()?;
            let index = *self.text.get_or_insert_with(|| {
                let index = self.next_block;
                self.next_block += 1;
                index
            });
            self.output.delta(index, false, text)?;
        }
        if let Some(calls) = delta.get("tool_calls").filter(|v| !v.is_null()) {
            let calls = calls
                .as_array()
                .ok_or_else(|| invalid("Bedrock tool calls are not an array"))?;
            self.start()?;
            for (fallback, call) in calls.iter().enumerate() {
                self.tools
                    .observe(call, fallback, &mut self.next_block, &mut self.output)?;
            }
        }
        if let Some(reason) = choice.get("finish_reason").filter(|v| !v.is_null()) {
            if self.finish.is_some() {
                return Err(invalid("Bedrock received duplicate finish"));
            }
            self.finish = Some(match reason.as_str() {
                Some("stop") => "end_turn",
                Some("tool_calls") if !self.tools.is_empty() => "tool_use",
                Some("length") => "max_tokens",
                Some("content_filter") => "guardrail_intervened",
                _ => return Err(invalid("Bedrock received unsupported finish reason")),
            });
        }
        Ok(())
    }

    fn start(&mut self) -> Result<(), ProtocolStreamError> {
        if !self.started {
            self.output
                .emit("messageStart", json!({"role":"assistant"}))?;
            self.started = true;
        }
        Ok(())
    }

    pub fn finish(&mut self) -> Result<(), ProtocolStreamError> {
        if self.native.active() {
            let usage = self.native.finish(&mut self.output)?;
            *self
                .validated_usage
                .lock()
                .map_err(|_| invalid("Bedrock usage owner is unavailable"))? = Some(usage);
            return Ok(());
        }
        let reason = self
            .finish
            .ok_or_else(|| invalid("Bedrock response EOF before finish"))?;
        let usage = self.usage.finish()?;
        self.tools.finish(&mut self.output)?;
        self.start()?;
        for index in 0..self.next_block {
            self.output
                .emit("contentBlockStop", json!({"contentBlockIndex":index}))?;
        }
        self.output
            .emit("messageStop", json!({"stopReason":reason}))?;
        // This is Gateway request elapsed time, not a fabricated AWS inference-service metric.
        let latency = self.started_at.elapsed().as_millis().min(i64::MAX as u128) as u64;
        let mut wire_usage = json!({"inputTokens":usage.prompt_tokens,
            "outputTokens":usage.completion_tokens,"totalTokens":usage.total_tokens});
        if let Some(read) = usage.cache_read_input_tokens {
            wire_usage["cacheReadInputTokens"] = json!(read);
        }
        if let Some(write) = usage.cache_creation_input_tokens {
            wire_usage["cacheWriteInputTokens"] = json!(write);
        }
        self.output.emit(
            "metadata",
            json!({"usage":wire_usage,"metrics":{"latencyMs":latency}}),
        )?;
        // The outer successful EOF callback settles this exact snapshot, not another SSE parser.
        *self
            .validated_usage
            .lock()
            .map_err(|_| invalid("Bedrock usage owner is unavailable"))? = Some(usage);
        Ok(())
    }
}
