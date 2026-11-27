//! Bounded in-memory Connect session control and `UserMessage` / `AgentRunRequest` builders.
use super::{
    protobuf::{connect_frame, field_bytes, field_str, field_varint, fields, Field},
    selection::valid_model_id,
    ProbeError, ValidatedImage,
};
use std::collections::HashMap;

const MAX_SESSION_MESSAGES: usize = 128;
const MAX_CONTEXTS: usize = 4;
const MAX_OUTBOUND_BYTES: usize = 320 * 1024;
const MAX_STORED_BLOBS: usize = 64;
const MAX_STORED_BYTES: usize = 256 * 1024;

pub(crate) fn default_text_context() -> Result<Vec<u8>, ProbeError> {
    let mut ctx = field_varint(17, 0)?;
    ctx.extend_from_slice(&field_varint(24, 0)?);
    Ok(ctx)
}

pub(crate) fn user_message(
    prompt: &str,
    mode: u64,
    images: &[ValidatedImage],
) -> Result<Vec<u8>, ProbeError> {
    let mut out = field_str(1, prompt)?;
    out.extend_from_slice(&field_str(2, &uuid::Uuid::new_v4().to_string())?);
    if !images.is_empty() {
        let mut selected_ctx = Vec::new();
        for img in images {
            let mut item = field_str(2, &uuid::Uuid::new_v4().to_string())?;
            item.extend_from_slice(&field_str(7, &img.mime)?);
            item.extend_from_slice(&field_bytes(8, &img.bytes)?);
            selected_ctx.extend_from_slice(&field_bytes(1, &item)?);
        }
        out.extend_from_slice(&field_bytes(3, &selected_ctx)?);
    }
    out.extend_from_slice(&field_varint(4, mode)?);
    Ok(out)
}

pub(crate) fn text_request(
    model: &str,
    prompt: &str,
    max_bytes: usize,
    images: &[ValidatedImage],
) -> Result<Vec<u8>, ProbeError> {
    if !valid_model_id(model) || max_bytes == 0 || max_bytes > 64 * 1024 || prompt.len() > max_bytes
    {
        return Err(ProbeError::new("invalid_text_request"));
    }
    let conversation = uuid::Uuid::new_v4().to_string();
    let user = user_message(prompt, 2, images)?;
    let context = default_text_context()?;
    let mut inner_action = field_bytes(1, &user)?;
    inner_action.extend_from_slice(&field_bytes(2, &context)?);
    let action = field_bytes(1, &inner_action)?;

    let mut run = field_bytes(1, &[])?;
    run.extend_from_slice(&field_bytes(2, &action)?);
    run.extend_from_slice(&field_bytes(4, &[])?);
    run.extend_from_slice(&field_str(5, &conversation)?);
    run.extend_from_slice(&field_bytes(9, &field_str(1, model)?)?);
    run.extend_from_slice(&field_str(16, &conversation)?);
    run.extend_from_slice(&field_str(25, &uuid::Uuid::new_v4().to_string())?);
    run.extend_from_slice(&field_varint(34, 0)?);
    run.extend_from_slice(&field_varint(37, 1)?);
    Ok(connect_frame(&field_bytes(1, &run)?, 0))
}

pub(crate) struct TextSession {
    request_context: Vec<u8>,
    blobs: HashMap<[u8; 32], Vec<u8>>,
    stored_bytes: usize,
    messages: usize,
    contexts: usize,
    outbound_bytes: usize,
    closed: bool,
}

impl TextSession {
    pub(crate) fn new(request_context: Vec<u8>) -> Self {
        Self {
            request_context,
            blobs: HashMap::new(),
            stored_bytes: 0,
            messages: 0,
            contexts: 0,
            outbound_bytes: 0,
            closed: false,
        }
    }

    pub(crate) fn reply(&mut self, envelope: Field<'_>) -> Result<Vec<u8>, ProbeError> {
        let reject = || ProbeError::new("remote_client_action_required");
        if self.closed {
            return Err(reject());
        }
        self.messages += 1;
        if self.messages > MAX_SESSION_MESSAGES {
            return Err(reject());
        }
        let response = match envelope.tag {
            2 => self.handle_context(envelope.bytes)?,
            4 => self.handle_blob(envelope.bytes)?,
            _ => return Err(reject()),
        };
        self.outbound_bytes = self.outbound_bytes.saturating_add(response.len());
        if self.outbound_bytes > MAX_OUTBOUND_BYTES {
            return Err(reject());
        }
        Ok(response)
    }

    fn handle_context(&mut self, raw: &[u8]) -> Result<Vec<u8>, ProbeError> {
        let reject = || ProbeError::new("remote_client_action_required");
        self.contexts += 1;
        if self.contexts > MAX_CONTEXTS {
            return Err(reject());
        }
        let values = selected(raw, &[1, 10, 15, 19, 57])?;
        let arg_bytes = req_bytes(values.get(&10), 4096)?;
        let args = selected(arg_bytes, &[2, 3, 4, 5, 7, 8])?;
        for item in args.values() {
            if matches!(item.tag, 7 | 8) {
                if opt_u32(Some(item))? > 1 {
                    return Err(reject());
                }
            } else {
                let _ = req_bytes(Some(item), 1024)?;
            }
        }
        if let Some(item) = values.get(&19) {
            let _ = req_bytes(Some(item), 4096)?;
        }
        if let Some(item) = values.get(&57) {
            let _ = req_bytes(Some(item), 256)?;
        }
        let mut payload = field_varint(1, u64::from(opt_u32(values.get(&1))?))?;
        if let Some(exec_item) = values.get(&15) {
            let id = req_bytes(Some(exec_item), 256)?;
            if std::str::from_utf8(id).is_err() {
                return Err(reject());
            }
            payload.extend_from_slice(&field_bytes(15, id)?);
        }
        let wrapped = field_bytes(
            10,
            &field_bytes(1, &field_bytes(1, &self.request_context)?)?,
        )?;
        payload.extend_from_slice(&wrapped);
        Ok(connect_frame(&field_bytes(2, &payload)?, 0))
    }

    fn handle_blob(&mut self, raw: &[u8]) -> Result<Vec<u8>, ProbeError> {
        let reject = || ProbeError::new("remote_client_action_required");
        let values = selected(raw, &[1, 2, 3, 4])?;
        let has_get = values.contains_key(&2);
        let has_set = values.contains_key(&3);
        if has_get == has_set {
            return Err(reject());
        }
        if let Some(item) = values.get(&4) {
            let _ = req_bytes(Some(item), 4096)?;
        }
        let inner_raw = req_bytes(values.get(if has_set { &3 } else { &2 }), 260 * 1024)?;
        let args = selected(inner_raw, if has_set { &[1, 2] } else { &[1] })?;
        let id_slice = req_bytes(args.get(&1), 32)?;
        let key: [u8; 32] = id_slice.try_into().map_err(|_| reject())?;

        let result_field = if has_set {
            let data = match args.get(&2) {
                Some(item) => req_bytes(Some(item), MAX_STORED_BYTES)?,
                None => &[],
            };
            if let Some(existing) = self.blobs.get(&key) {
                if existing.as_slice() != data {
                    return Err(reject());
                }
            } else {
                if self.blobs.len() >= MAX_STORED_BLOBS
                    || self.stored_bytes + data.len() > MAX_STORED_BYTES
                {
                    return Err(reject());
                }
                self.stored_bytes += data.len();
                self.blobs.insert(key, data.to_vec());
            }
            field_bytes(3, &[])?
        } else {
            let Some(stored) = self.blobs.get(&key) else {
                return Err(reject());
            };
            field_bytes(2, &field_bytes(1, stored)?)?
        };
        let mut payload = field_varint(1, u64::from(opt_u32(values.get(&1))?))?;
        payload.extend_from_slice(&result_field);
        Ok(connect_frame(&field_bytes(3, &payload)?, 0))
    }

    pub(crate) fn close(&mut self) {
        self.closed = true;
        for blob in self.blobs.values_mut() {
            blob.fill(0);
        }
        self.blobs.clear();
        self.stored_bytes = 0;
    }
}

impl Drop for TextSession {
    fn drop(&mut self) {
        self.close();
    }
}

fn selected<'a>(raw: &'a [u8], allowed: &[u32]) -> Result<HashMap<u32, Field<'a>>, ProbeError> {
    let reject = || ProbeError::new("remote_client_action_required");
    let mut map = HashMap::new();
    for item in fields(raw)? {
        if !allowed.contains(&item.tag) || map.contains_key(&item.tag) {
            return Err(reject());
        }
        map.insert(item.tag, item);
    }
    Ok(map)
}

fn req_bytes<'a>(item: Option<&Field<'a>>, max: usize) -> Result<&'a [u8], ProbeError> {
    let Some(item) = item else {
        return Err(ProbeError::new("remote_client_action_required"));
    };
    if item.wire != 2 || item.bytes.len() > max {
        return Err(ProbeError::new("remote_client_action_required"));
    }
    Ok(item.bytes)
}

fn opt_u32(item: Option<&Field<'_>>) -> Result<u32, ProbeError> {
    let Some(item) = item else {
        return Ok(0);
    };
    if item.wire != 0 {
        return Err(ProbeError::new("remote_client_action_required"));
    }
    u32::try_from(item.varint).map_err(|_| ProbeError::new("remote_client_action_required"))
}
