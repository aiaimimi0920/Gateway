//! In-process pure-Rust Cursor execution owner with two-phase authorization and uncertainty tracking.
use super::{Authorize, Completed, Sent, ToolCall};
use crate::{
    error::GatewayError,
    protocol::cursor_remote::{
        thinking::{PublicThinking, MAX_PHASE, MAX_RUN},
        tool_schema::ToolKind,
        upstream::{self, transport_live::LiveTransport, Input},
    },
};
use serde_json::{json, Value};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::{
    sync::oneshot,
    time::{timeout_at, Duration, Instant},
};

#[derive(Debug, Clone)]
pub(crate) struct WorkerToolCall {
    pub(crate) call_id: String,
    pub(crate) name: String,
    pub(crate) arguments: String,
    pub(crate) text: String,
    pub(crate) thinking: Option<String>,
    pub(crate) requested_model: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkerToolResult {
    pub(crate) call_id: String,
    pub(crate) output: String,
}

#[derive(Debug, Clone)]
pub(crate) struct WorkerOutput {
    pub(crate) text: String,
    pub(crate) thinking: Option<String>,
    pub(crate) requested_model: String,
    pub(crate) usage: Option<crate::protocol::canonical::TokenUsage>,
}

pub(crate) struct WorkerHooks {
    authorize: Option<Authorize>,
    sent: Option<Sent>,
    granted: bool,
    uncertain: Arc<AtomicBool>,
    thinking_total: usize,
    bridge: Option<super::tools::Bridge>,
}

impl WorkerHooks {
    pub(crate) fn new(
        authorize: Authorize,
        sent: Sent,
        uncertain: Arc<AtomicBool>,
        bridge: Option<super::tools::Bridge>,
    ) -> Self {
        Self {
            authorize: Some(authorize),
            sent: Some(sent),
            granted: false,
            uncertain,
            thinking_total: 0,
            bridge,
        }
    }

    pub(crate) fn has_tool_bridge(&self) -> bool {
        self.bridge.is_some()
    }

    pub(crate) fn is_uncertain(&self) -> bool {
        self.uncertain.load(Ordering::SeqCst)
    }

    pub(crate) async fn authorize(&mut self) -> Result<(), GatewayError> {
        if self.granted {
            return Err(super::super::error("worker_protocol"));
        }
        let auth = self
            .authorize
            .take()
            .ok_or_else(|| super::super::error("worker_protocol"))?;
        auth().await?;
        self.uncertain.store(true, Ordering::SeqCst);
        self.granted = true;
        Ok(())
    }

    pub(crate) fn sent(&mut self) -> Result<(), GatewayError> {
        if !self.granted {
            return Err(super::super::error("worker_protocol"));
        }
        let notify = self
            .sent
            .take()
            .ok_or_else(|| super::super::error("worker_protocol"))?;
        notify();
        self.uncertain.store(false, Ordering::SeqCst);
        Ok(())
    }

    pub(crate) fn continued(&mut self) -> Result<(), GatewayError> {
        self.sent()
    }

    pub(crate) async fn emit_tool_call(
        &mut self,
        call: WorkerToolCall,
    ) -> Result<String, GatewayError> {
        let fail = || super::super::error("worker_protocol");
        if !self.granted || self.sent.is_some() || self.bridge.is_none() {
            return Err(fail());
        }
        let expected_id = call.call_id.clone();
        let phase = build_completed(
            call.text,
            call.thinking,
            call.requested_model,
            Some((call.call_id, call.name, call.arguments)),
            &mut self.thinking_total,
        )?;
        let bridge = self.bridge.as_mut().ok_or_else(fail)?;
        bridge.events.send(Ok(phase)).await.map_err(|_| fail())?;
        Ok(expected_id)
    }

    pub(crate) async fn recv_tool_result(
        &mut self,
        expected_call_id: &str,
    ) -> Result<WorkerToolResult, GatewayError> {
        let fail = || super::super::error("worker_protocol");
        let bridge = self.bridge.as_mut().ok_or_else(fail)?;
        let round = bridge.commands.recv().await.ok_or_else(fail)?;
        let wire_len = serde_json::to_vec(&round.wire()).map_err(|_| fail())?.len() + 1;
        if round.wire()["callId"] != expected_call_id {
            return Err(fail());
        }
        if wire_len > super::super::text_output::MAX_RESULT_IPC {
            return Err(super::super::error("worker_input_limit"));
        }
        (round.authorize)().await?;
        self.uncertain = round.uncertain;
        self.sent = Some(round.sent);
        self.uncertain.store(true, Ordering::SeqCst);
        Ok(WorkerToolResult {
            call_id: round.call_id,
            output: round.output,
        })
    }

    #[cfg(test)]
    pub(crate) async fn tool(
        &mut self,
        call: WorkerToolCall,
    ) -> Result<WorkerToolResult, GatewayError> {
        let id = self.emit_tool_call(call).await?;
        self.recv_tool_result(&id).await
    }

    fn finish(&mut self, output: WorkerOutput) -> Result<Completed, GatewayError> {
        if !self.granted || self.sent.is_some() {
            return Err(super::super::error("worker_protocol"));
        }
        let mut completed = build_completed(
            output.text,
            output.thinking,
            output.requested_model,
            None,
            &mut self.thinking_total,
        )?;
        completed.usage = output.usage;
        Ok(completed)
    }
}

pub(crate) fn build_completed(
    text: String,
    thinking: Option<String>,
    requested_model: String,
    tool: Option<(String, String, String)>,
    thinking_total: &mut usize,
) -> Result<Completed, GatewayError> {
    let fail = || super::super::error("worker_protocol");
    let check_str = |s: &str, max: usize, allow_empty: bool| -> Result<(), GatewayError> {
        if s.len() <= max && (allow_empty || !s.trim().is_empty()) {
            Ok(())
        } else {
            Err(fail())
        }
    };
    let is_tool = tool.is_some();
    check_str(&text, 65_536, is_tool)?;
    check_str(&requested_model, 160, false)?;

    let call = if let Some((id, name, arguments)) = tool {
        check_str(&id, 256, false)?;
        check_str(&name, 64, false)?;
        check_str(&arguments, 65_536, false)?;
        if !serde_json::from_str::<Value>(&arguments).is_ok_and(|v| v.is_object()) {
            return Err(fail());
        }
        Some(ToolCall {
            id,
            name,
            arguments,
            kind: ToolKind::Function,
        })
    } else {
        None
    };

    let public_thinking = match thinking {
        None => None,
        Some(raw) => {
            if raw.len() > MAX_PHASE {
                return Err(fail());
            }
            let event = json!({ "thinking": raw });
            PublicThinking::from_worker(&event)?
        }
    };
    *thinking_total += public_thinking.as_ref().map_or(0, |v| v.text().len());
    if *thinking_total > MAX_RUN {
        return Err(fail());
    }

    Ok(Completed {
        text,
        thinking: public_thinking,
        selector: requested_model,
        call,
        publication: None,
        usage: None,
    })
}

pub(super) async fn run(
    proxy_port: u16,
    input: Value,
    #[cfg(test)] fixture: Option<super::TestWorker>,
    deadline: Instant,
    authorize: Authorize,
    sent: Sent,
    mut cancelled: oneshot::Receiver<()>,
    uncertain: Arc<AtomicBool>,
    bridge: Option<super::tools::Bridge>,
) -> Result<Completed, GatewayError> {
    if deadline <= Instant::now()
        || cancelled.try_recv() != Err(oneshot::error::TryRecvError::Empty)
    {
        return Err(super::super::error("cancelled"));
    }
    let has_tools = bridge.is_some();
    let watchdog = Duration::from_secs(if has_tools { 300 } else { 120 });
    let effective_deadline = deadline.min(Instant::now() + watchdog);

    let mut hooks = WorkerHooks::new(authorize, sent, uncertain, bridge);
    let result = {
        let exchange = async {
            #[cfg(test)]
            if let Some(handler) = fixture {
                let output = handler(input, &mut hooks).await?;
                return hooks.finish(output);
            }
            let parsed =
                Input::parse(&input, proxy_port).map_err(|e| super::super::error(e.code))?;
            let transport = LiveTransport::default();
            let output = upstream::execute(parsed, &mut hooks, &transport)
                .await
                .map_err(upstream::RunError::into_gateway)?;
            hooks.finish(output)
        };
        tokio::select! {
            biased;
            _ = &mut cancelled => Err(super::super::error("cancelled")),
            res = timeout_at(effective_deadline, exchange) => {
                res.unwrap_or_else(|_| Err(super::super::error("deadline")))
            }
        }
    };
    if result.is_err() && hooks.is_uncertain() {
        return Err(super::super::error("delivery_unknown"));
    }
    result
}
