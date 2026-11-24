//! Bounded incremental Server-Sent Events (SSE) frame decoder for streaming provider adapters.

/// Parsed SSE event frame with optional `event:` type and joined `data:` payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEventFrame {
    pub event: Option<String>,
    pub data: String,
}

/// Incremental SSE byte buffer enforcing a strict per-frame byte ceiling to prevent OOM.
#[derive(Debug)]
pub struct BoundedSseDecoder {
    buffer: Vec<u8>,
    max_buffer_bytes: usize,
}

impl BoundedSseDecoder {
    pub fn new(max_buffer_bytes: usize) -> Self {
        Self {
            buffer: Vec::new(),
            max_buffer_bytes: max_buffer_bytes.max(1024),
        }
    }

    /// Push a chunk of upstream bytes and drain all complete SSE frames.
    pub fn push_chunk(&mut self, chunk: &[u8]) -> Result<Vec<SseEventFrame>, String> {
        if self.buffer.len().saturating_add(chunk.len()) > self.max_buffer_bytes {
            self.buffer.clear();
            return Err(format!(
                "SSE frame exceeded maximum buffer of {} bytes",
                self.max_buffer_bytes
            ));
        }
        self.buffer.extend_from_slice(chunk);
        let mut frames = Vec::new();
        while let Some((frame_end, delimiter_len)) = find_frame_boundary(&self.buffer) {
            let raw = self.buffer[..frame_end].to_vec();
            self.buffer.drain(..frame_end + delimiter_len);
            if let Some(parsed) = parse_raw_frame(&raw)? {
                frames.push(parsed);
            }
        }
        Ok(frames)
    }

    /// Flush trailing frame data at EOF when the upstream omits a final blank line.
    pub fn finish(&mut self) -> Result<Option<SseEventFrame>, String> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        let raw = std::mem::take(&mut self.buffer);
        parse_raw_frame(&raw)
    }
}

fn find_frame_boundary(buf: &[u8]) -> Option<(usize, usize)> {
    let mut idx = 0;
    while idx + 1 < buf.len() {
        if buf[idx] == b'\n' && buf[idx + 1] == b'\n' {
            return Some((idx, 2));
        }
        if idx + 3 < buf.len()
            && buf[idx] == b'\r'
            && buf[idx + 1] == b'\n'
            && buf[idx + 2] == b'\r'
            && buf[idx + 3] == b'\n'
        {
            return Some((idx, 4));
        }
        idx += 1;
    }
    None
}

fn parse_raw_frame(raw: &[u8]) -> Result<Option<SseEventFrame>, String> {
    let text = std::str::from_utf8(raw).map_err(|_| "invalid UTF-8 in SSE frame".to_string())?;
    let mut event_name: Option<String> = None;
    let mut data_lines: Vec<&str> = Vec::new();
    for raw_line in text.split('\n') {
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() || line.starts_with(':') {
            continue;
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "event" => event_name = Some(value.to_string()),
            "data" => data_lines.push(value),
            _ => {}
        }
    }
    if event_name.is_none() && data_lines.is_empty() {
        return Ok(None);
    }
    Ok(Some(SseEventFrame {
        event: event_name,
        data: data_lines.join("\n"),
    }))
}
