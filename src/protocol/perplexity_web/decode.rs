//! 复用共享严格 SSE decoder；每帧、总解码字节和事件数均有本地预算。
use super::{failure, schematized, snapshot, workflow, Answer};
use crate::{
    error::GatewayError,
    protocol::stream_decode::{BoundedSseDecoder, DecodeStep},
};
use bytes::Bytes;
use serde_json::Value;

pub(crate) const BODY_LIMIT: usize = 4 * 1024 * 1024;
const FRAME_LIMIT: usize = 512 * 1024;
const EVENT_LIMIT: usize = 4096;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Update {
    Snapshot(Option<Answer>),
    Complete(Answer),
}

pub(crate) struct Decoder {
    wire: BoundedSseDecoder,
    bytes: usize,
    events: usize,
    last: Option<Answer>,
    terminal: bool,
    schematized: schematized::State,
    workflow: workflow::State,
    auto_answer_seen: bool,
}

impl Decoder {
    pub(crate) fn new() -> Self {
        Self {
            wire: BoundedSseDecoder::new_strict(FRAME_LIMIT),
            bytes: 0,
            events: 0,
            last: None,
            terminal: false,
            schematized: schematized::State::default(),
            workflow: workflow::State::default(),
            auto_answer_seen: false,
        }
    }

    #[cfg(test)]
    pub(crate) fn push(&mut self, bytes: Bytes) -> Result<Option<Answer>, GatewayError> {
        self.push_chunk(bytes)?;
        while let Some(event) = self.next_event()? {
            if let Update::Complete(answer) = event {
                return Ok(Some(answer));
            }
        }
        Ok(None)
    }

    pub(crate) fn push_chunk(&mut self, bytes: Bytes) -> Result<(), GatewayError> {
        if self.terminal {
            return Err(failure(
                "perplexity_web_after_terminal",
                "Perplexity Web 已完成，不接收后续事件。",
            ));
        }
        self.bytes = self
            .bytes
            .checked_add(bytes.len())
            .filter(|size| *size <= BODY_LIMIT)
            .ok_or_else(|| {
                failure(
                    "perplexity_web_response_limit",
                    "Perplexity Web 响应超过本地大小预算。",
                )
            })?;
        self.wire.push_chunk(bytes);
        Ok(())
    }

    /// Return one validated frame at a time; a later frame in the same chunk cannot hide this one.
    pub(crate) fn next_event(&mut self) -> Result<Option<Update>, GatewayError> {
        loop {
            match self.wire.decode_next() {
                DecodeStep::NeedInput => return Ok(None),
                DecodeStep::Error(_) => {
                    return Err(failure(
                        "perplexity_web_invalid_sse",
                        "Perplexity Web SSE 编码无效或单事件超过预算。",
                    ))
                }
                DecodeStep::Frame(frame) => {
                    self.events += 1;
                    if self.events > EVENT_LIMIT {
                        return Err(failure(
                            "perplexity_web_event_limit",
                            "Perplexity Web 事件数超过本地预算。",
                        ));
                    }
                    if frame.event_name.as_deref() == Some("end_of_stream") {
                        // A patch snapshot is not a materialized COMPLETED answer.
                        if self.schematized.active() || self.workflow.active() {
                            return Err(incomplete());
                        }
                        // 非空终态 payload 的语义尚未冻结；只允许空 data 或空 JSON 对象。
                        if !frame.data.trim().is_empty()
                            && !serde_json::from_str::<Value>(&frame.data)
                                .ok()
                                .and_then(|v| v.as_object().map(|v| v.is_empty()))
                                .unwrap_or(false)
                        {
                            return Err(failure(
                                "perplexity_web_unsupported_terminal_payload",
                                "Perplexity Web 终态携带未支持的数据；不会用历史 partial 结算。",
                            ));
                        }
                        let answer = self
                            .last
                            .take()
                            .filter(|answer| !answer.text.trim().is_empty())
                            .ok_or_else(incomplete)?;
                        self.terminal = true;
                        return Ok(Some(Update::Complete(answer)));
                    }
                    if frame.event_name.as_deref() == Some("error") {
                        return Err(failure(
                            "perplexity_web_error_event",
                            "Perplexity Web 返回错误事件；正文已省略。",
                        ));
                    }
                    if frame.data.is_empty() {
                        continue;
                    }
                    let raw: Value = serde_json::from_str(&frame.data).map_err(|_| {
                        failure(
                            "perplexity_web_invalid_json",
                            "Perplexity Web SSE 数据不是有效 JSON。",
                        )
                    })?;
                    // Lock the workflow owner before any answer exists; profiles cannot switch.
                    if self.workflow.active() || workflow::State::recognizes(&raw) {
                        if self.auto_answer_seen || self.schematized.active() {
                            return Err(workflow::invalid());
                        }
                        if let Some(answer) = self.workflow.extract(&raw)? {
                            self.terminal = true;
                            return Ok(Some(Update::Complete(answer)));
                        }
                        return Ok(Some(Update::Snapshot(None)));
                    }
                    if self.schematized.active() || schematized::State::recognizes(&raw) {
                        if self.auto_answer_seen {
                            return Err(schematized::invalid());
                        }
                        let (answer, complete) = self.schematized.extract(&raw)?;
                        if complete {
                            self.terminal = true;
                            return Ok(Some(Update::Complete(answer.ok_or_else(incomplete)?)));
                        }
                        return Ok(Some(Update::Snapshot(answer)));
                    }
                    // 严格采用最后一个数据事件的答案；不能用更早的 partial 掩盖最终空事件。
                    self.last = snapshot::extract(&raw)?;
                    self.auto_answer_seen |= self.last.is_some();
                    return Ok(Some(Update::Snapshot(self.last.clone())));
                }
            }
        }
    }

    pub(crate) fn finish(&self) -> Result<(), GatewayError> {
        if self.terminal {
            return Ok(());
        }
        self.wire.finish_strict().map_err(|_| {
            failure(
                "perplexity_web_truncated_sse",
                "Perplexity Web SSE 在事件中途截断。",
            )
        })?;
        Err(incomplete())
    }
}

fn incomplete() -> GatewayError {
    failure(
        "perplexity_web_incomplete",
        "Perplexity Web 缺少显式终态或最后一个事件没有可用答案。",
    )
}
