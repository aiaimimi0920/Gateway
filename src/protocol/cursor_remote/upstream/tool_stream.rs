//! Native MCP tool declarations, `ToolDecoderState`, and duplex tool-turn execution.
use super::{
    connect_stream::TextDecoderState,
    protobuf::{
        connect_frame, decode_struct_map, encode_value, field_bytes, field_str, field_varint,
        fields, oneof, scalar_bytes, scalar_varint_opt, utf8_bounded, Field,
    },
    selection::valid_model_id,
    text_session::{user_message, TextSession},
    transport::{CheckedClient, DuplexRequest, ENDPOINTS},
    ProbeError, RunError, ToolSpec, ValidatedImage, WorkerOutput,
};
use crate::protocol::cursor_remote::runtime::worker::{WorkerHooks, WorkerToolCall};
use bytes::Bytes;
use std::{collections::HashSet, time::Duration};

const PROVIDER: &str = "gateway";
const MAX_CONTROL_MESSAGES: usize = 128;
const MAX_TOOL_CALLS: usize = 64;

struct PendingCall {
    call_id: String,
    identity: Vec<u8>,
}

pub(crate) struct ToolDecoderState {
    base: TextDecoderState,
    defs: Vec<Vec<u8>>,
    declared: HashSet<String>,
    pending: Option<PendingCall>,
    calls: Vec<WorkerToolCall>,
    used_calls: HashSet<String>,
    used_exec: HashSet<Vec<u8>>,
    control_count: usize,
    requested_model: String,
}

impl ToolDecoderState {
    pub(crate) fn new(tools: &[ToolSpec], requested_model: String) -> Result<Self, ProbeError> {
        let defs = encode_definitions(tools)?;
        let declared: HashSet<String> = tools.iter().map(|t| t.name.clone()).collect();
        let ctx = tool_context(&defs)?;
        Ok(Self {
            base: TextDecoderState::with_session(Some(TextSession::new(ctx))),
            defs,
            declared,
            pending: None,
            calls: Vec::new(),
            used_calls: HashSet::new(),
            used_exec: HashSet::new(),
            control_count: 0,
            requested_model,
        })
    }

    pub(crate) fn defs(&self) -> &[Vec<u8>] {
        &self.defs
    }

    pub(crate) fn push(&mut self, chunk: &[u8]) -> Result<(), ProbeError> {
        let defs = &self.defs;
        let declared = &self.declared;
        let pending = &mut self.pending;
        let calls = &mut self.calls;
        let used_calls = &mut self.used_calls;
        let used_exec = &mut self.used_exec;
        let control_count = &mut self.control_count;
        let requested_model = &self.requested_model;

        self.base.push_with(chunk, |base, bytes| {
            let Some(envelope) = oneof(bytes, &[(8, 2)])? else {
                return Ok(());
            };
            if envelope.tag == 2 {
                let raw = fields(envelope.bytes)?;
                let action = oneof(
                    envelope.bytes,
                    &[(1, 0), (15, 2), (19, 2), (55, 0), (57, 2), (58, 0), (59, 0)],
                )?;
                if let Some(act) = action.filter(|a| matches!(a.tag, 11 | 36 | 42)) {
                    *control_count += 1;
                    if base.turn_ended || *control_count > MAX_CONTROL_MESSAGES {
                        return Err(ProbeError::new("invalid_mcp_lifecycle"));
                    }
                    let args = fields(act.bytes)?;
                    let identity = exec_identity(&raw)?;
                    if act.tag == 36 {
                        if args.iter().any(|v| !matches!(v.tag, 1 | 2 | 3))
                            || args.iter().filter(|v| v.tag == 1).any(|v| {
                                v.wire != 2
                                    || utf8_bounded(v.bytes, 256).map_or(true, |s| s != PROVIDER)
                            })
                            || scalar_varint_opt(&args, 2, 0)? > 1
                            || scalar_varint_opt(&args, 3, 0)? > 2
                        {
                            return Err(ProbeError::new("unsupported_mcp_state_request"));
                        }
                        let mut server = field_str(1, PROVIDER)?;
                        server.extend_from_slice(&field_str(2, PROVIDER)?);
                        for def in defs {
                            server.extend_from_slice(&field_bytes(5, def)?);
                        }
                        let mut reply = identity;
                        reply.extend_from_slice(&field_bytes(
                            36,
                            &field_bytes(1, &field_bytes(1, &server)?)?,
                        )?);
                        base.replies
                            .push(connect_frame(&field_bytes(2, &reply)?, 0));
                        return Ok(());
                    }
                    if act.tag == 42 {
                        let p = utf8_bounded(
                            scalar_bytes(&args, 1)
                                .ok_or_else(|| ProbeError::new("invalid_mcp_message"))?,
                            65_536,
                        )?;
                        let t = utf8_bounded(
                            scalar_bytes(&args, 2)
                                .ok_or_else(|| ProbeError::new("invalid_mcp_message"))?,
                            65_536,
                        )?;
                        let allowed = p == PROVIDER && declared.contains(&t);
                        let mut reply = identity;
                        reply.extend_from_slice(&field_bytes(
                            42,
                            &field_varint(1, u64::from(allowed))?,
                        )?);
                        base.replies
                            .push(connect_frame(&field_bytes(2, &reply)?, 0));
                        return Ok(());
                    }
                    let fail_mcp = || ProbeError::new("invalid_mcp_message");
                    let name = utf8_bounded(scalar_bytes(&args, 5).ok_or_else(fail_mcp)?, 64)?;
                    let call_id = utf8_bounded(scalar_bytes(&args, 3).ok_or_else(fail_mcp)?, 256)?;
                    let supplied_name =
                        utf8_bounded(scalar_bytes(&args, 1).ok_or_else(fail_mcp)?, 65_536)?;
                    let supplied_provider =
                        utf8_bounded(scalar_bytes(&args, 4).ok_or_else(fail_mcp)?, 65_536)?;
                    let server = match opt_bytes(&args, 9)? {
                        Some(raw_srv) => {
                            let s = utf8_bounded(raw_srv, 256)?;
                            if s.is_empty() {
                                supplied_provider.clone()
                            } else {
                                s
                            }
                        }
                        None => supplied_provider.clone(),
                    };
                    let alias = declared
                        .iter()
                        .find(|key| wire_name(key) == supplied_name)
                        .map(String::as_str);
                    let approval_only = scalar_varint_opt(&args, 7, 0)?;

                    if !declared.contains(&name)
                        || supplied_provider != PROVIDER
                        || server != PROVIDER
                        || alias.is_some_and(|a| a != name)
                        || call_id.trim().is_empty()
                        || pending.is_some()
                        || used_calls.contains(&call_id)
                        || used_calls.len() >= MAX_TOOL_CALLS
                        || used_exec.contains(&identity)
                        || args
                            .iter()
                            .any(|v| !matches!(v.tag, 1 | 2 | 3 | 4 | 5 | 7 | 8 | 9 | 11 | 12))
                        || approval_only != 0
                    {
                        return Err(ProbeError::new("undeclared_or_duplicate_tool"));
                    }
                    let decoded_args = decode_struct_map(act.bytes, 2, 0)?;
                    let arguments = serde_json::to_string(&decoded_args)
                        .map_err(|_| ProbeError::new("invalid_mcp_message"))?;
                    if arguments.len() > 65_536 {
                        return Err(ProbeError::new("tool_arguments_too_large"));
                    }
                    used_calls.insert(call_id.clone());
                    used_exec.insert(identity.clone());
                    *pending = Some(PendingCall {
                        call_id: call_id.clone(),
                        identity,
                    });
                    let phase = base.take_phase();
                    calls.push(WorkerToolCall {
                        call_id,
                        name,
                        arguments,
                        text: phase.text,
                        thinking: phase.thinking,
                        requested_model: requested_model.clone(),
                    });
                    return Ok(());
                }
            }
            if envelope.tag == 1 {
                if let Some(update) = oneof(envelope.bytes, &[(25, 0)])? {
                    if matches!(update.tag, 2 | 3 | 7 | 15 | 27) {
                        if base.turn_ended {
                            return Err(ProbeError::new("invalid_mcp_lifecycle"));
                        }
                        return Ok(());
                    }
                    if update.tag == 14 && pending.is_some() {
                        return Err(ProbeError::new("tool_pending_at_turn_end"));
                    }
                }
            }
            base.decode_server_message(bytes)
        })
    }

    pub(crate) fn take_calls(&mut self) -> Vec<WorkerToolCall> {
        std::mem::take(&mut self.calls)
    }

    pub(crate) fn take_replies(&mut self) -> Result<Vec<Vec<u8>>, ProbeError> {
        self.base.take_replies()
    }

    pub(crate) fn complete(&mut self, call_id: &str, output: &str) -> Result<Vec<u8>, ProbeError> {
        if self.base.end_stream {
            return Err(ProbeError::new("unexpected_tool_result"));
        }
        let Some(pending) = self.pending.take() else {
            return Err(ProbeError::new("unexpected_tool_result"));
        };
        if pending.call_id != call_id {
            return Err(ProbeError::new("unexpected_tool_result"));
        }
        if output.len() > 65_536 {
            return Err(ProbeError::new("invalid_mcp_message"));
        }
        let nested = field_bytes(
            11,
            &field_bytes(
                1,
                &field_bytes(1, &field_bytes(1, &field_str(1, output)?)?)?,
            )?,
        )?;
        let mut payload = pending.identity;
        payload.extend_from_slice(&nested);
        Ok(connect_frame(&field_bytes(2, &payload)?, 0))
    }

    pub(crate) fn finish(mut self) -> Result<WorkerOutput, ProbeError> {
        let model = std::mem::take(&mut self.requested_model);
        self.base.finish(model)
    }
}

pub(crate) async fn execute_tools(
    token: &str,
    model: String,
    prompt: &str,
    images: &[ValidatedImage],
    tools: &[ToolSpec],
    client: &CheckedClient<'_>,
    hooks: &mut WorkerHooks,
) -> Result<WorkerOutput, RunError> {
    if !hooks.has_tool_bridge() {
        return Err(ProbeError::new("tool_channel_required").into());
    }
    let mut decoder = ToolDecoderState::new(tools, model.clone())?;
    let body = tool_request(&model, prompt, decoder.defs(), 2, images)?;
    let auth_header = format!("Bearer {token}");
    let headers = [
        ("authorization", auth_header.as_str()),
        ("content-type", "application/connect+proto"),
        ("accept", "application/connect+proto"),
        ("connect-protocol-version", "1"),
        ("connect-accept-encoding", "identity"),
    ];
    let mut stream = client
        .open_duplex(
            DuplexRequest {
                url: ENDPOINTS.run,
                headers: &headers,
                body: Bytes::from(body),
                timeout: Duration::from_secs(300),
            },
            hooks,
        )
        .await?;

    while let Some(chunk) = stream.next_chunk().await? {
        if stream.status() != 200 {
            continue;
        }
        decoder.push(&chunk)?;
        for reply in decoder.take_replies()? {
            stream.send_frame(Bytes::from(reply))?;
        }
        let mut calls = decoder.take_calls();
        if calls.len() > 1 {
            return Err(ProbeError::new("concurrent_tool_call").into());
        }
        if let Some(call) = calls.pop() {
            let expected_id = hooks
                .emit_tool_call(call)
                .await
                .map_err(RunError::Gateway)?;
            let resumed = {
                // Receiving a round consumes it before async authorization finishes.
                // Keep that future alive when an inbound frame wins the select.
                let result = hooks.recv_tool_result(&expected_id);
                tokio::pin!(result);
                loop {
                    tokio::select! {
                        biased;
                        inbound = stream.next_chunk() => {
                            let Some(extra) = inbound? else {
                                return Err(ProbeError::new("remote_end_during_tool_wait").into());
                            };
                            if stream.status() == 200 {
                                decoder.push(&extra)?;
                                for reply in decoder.take_replies()? {
                                    stream.send_frame(Bytes::from(reply))?;
                                }
                                if !decoder.take_calls().is_empty() {
                                    return Err(ProbeError::new("concurrent_tool_call").into());
                                }
                            }
                        }
                        res = &mut result => {
                            break res.map_err(RunError::Gateway)?;
                        }
                    }
                }
            };
            let frame = decoder.complete(&resumed.call_id, &resumed.output)?;
            stream.send_frame(Bytes::from(frame))?;
            hooks.continued().map_err(RunError::Gateway)?;
        }
    }
    if stream.status() != 200 {
        return Err(ProbeError::with_status("inference_http_error", stream.status()).into());
    }
    decoder.finish().map_err(RunError::Probe)
}

fn wire_name(name: &str) -> String {
    format!("mcp_{PROVIDER}_{name}")
}

fn valid_tool_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    let Some(first) = bytes.next() else {
        return false;
    };
    name.len() <= 64
        && (first.is_ascii_alphabetic() || first == b'_')
        && bytes.all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

pub(crate) fn encode_definitions(tools: &[ToolSpec]) -> Result<Vec<Vec<u8>>, ProbeError> {
    let fail = || ProbeError::new("invalid_mcp_message");
    if tools.is_empty() || tools.len() > 64 {
        return Err(fail());
    }
    let mut names = HashSet::with_capacity(tools.len());
    let mut out = Vec::with_capacity(tools.len());
    for tool in tools {
        if !valid_tool_name(&tool.name)
            || !names.insert(tool.name.as_str())
            || tool.description.chars().count() > 32_768
            || !tool.parameters.is_object()
        {
            return Err(fail());
        }
        let schema_json = serde_json::to_string(&tool.parameters).map_err(|_| fail())?;
        if schema_json.len() > 32_768 {
            return Err(fail());
        }
        let mut def = field_str(1, &wire_name(&tool.name))?;
        def.extend_from_slice(&field_str(2, &tool.description)?);
        def.extend_from_slice(&field_bytes(3, &encode_value(&tool.parameters, 0)?)?);
        def.extend_from_slice(&field_str(4, PROVIDER)?);
        def.extend_from_slice(&field_str(5, &tool.name)?);
        def.extend_from_slice(&field_str(6, &schema_json)?);
        out.push(def);
    }
    Ok(out)
}

pub(crate) fn tool_context(defs: &[Vec<u8>]) -> Result<Vec<u8>, ProbeError> {
    let mut out = field_varint(17, 0)?;
    out.extend_from_slice(&field_varint(24, 0)?);
    out.extend_from_slice(&field_varint(32, 0)?);
    out.extend_from_slice(&field_varint(35, 0)?);
    out.extend_from_slice(&field_varint(36, 1)?);
    for def in defs {
        out.extend_from_slice(&field_bytes(7, def)?);
    }
    Ok(out)
}

pub(crate) fn tool_request(
    model: &str,
    prompt: &str,
    defs: &[Vec<u8>],
    mode: u64,
    images: &[ValidatedImage],
) -> Result<Vec<u8>, ProbeError> {
    if !valid_model_id(model) || !matches!(mode, 1 | 2) || prompt.len() > 65_536 {
        return Err(ProbeError::new("invalid_mcp_message"));
    }
    let conversation = uuid::Uuid::new_v4().to_string();
    let user = user_message(prompt, mode, images)?;
    let ctx = tool_context(defs)?;
    let mut inner = field_bytes(1, &user)?;
    inner.extend_from_slice(&field_bytes(2, &ctx)?);
    let action = field_bytes(1, &inner)?;

    let mut mcp_tools = Vec::new();
    for def in defs {
        mcp_tools.extend_from_slice(&field_bytes(1, def)?);
    }
    let mut run = field_bytes(1, &[])?;
    run.extend_from_slice(&field_bytes(2, &action)?);
    run.extend_from_slice(&field_bytes(4, &mcp_tools)?);
    run.extend_from_slice(&field_str(5, &conversation)?);
    run.extend_from_slice(&field_bytes(9, &field_str(1, model)?)?);
    run.extend_from_slice(&field_str(16, &conversation)?);
    run.extend_from_slice(&field_str(25, &uuid::Uuid::new_v4().to_string())?);
    run.extend_from_slice(&field_varint(34, 0)?);
    run.extend_from_slice(&field_varint(37, 1)?);
    Ok(connect_frame(&field_bytes(1, &run)?, 0))
}

fn exec_identity(raw: &[Field<'_>]) -> Result<Vec<u8>, ProbeError> {
    let fail = || ProbeError::new("invalid_mcp_message");
    let id = scalar_varint_opt(raw, 1, 0)?;
    if id > u64::from(u32::MAX) {
        return Err(fail());
    }
    let mut out = field_varint(1, id)?;
    if let Some(exec_bytes) = opt_bytes(raw, 15)? {
        let s = utf8_bounded(exec_bytes, 256)?;
        out.extend_from_slice(&field_str(15, &s)?);
    }
    Ok(out)
}

fn opt_bytes<'a>(raw: &[Field<'a>], tag: u32) -> Result<Option<&'a [u8]>, ProbeError> {
    let mut iter = raw.iter().filter(|f| f.tag == tag);
    let Some(first) = iter.next() else {
        return Ok(None);
    };
    if iter.next().is_some() || first.wire != 2 {
        return Err(ProbeError::new("invalid_mcp_message"));
    }
    Ok(Some(first.bytes))
}
