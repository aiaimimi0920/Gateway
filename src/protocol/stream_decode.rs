use bytes::Bytes;

use crate::protocol::sse_parse::SseFrame;

pub(super) const MAX_TRANSLATED_SSE_FRAME_BYTES: usize = 64 * 1024 * 1024;
const MAX_RETAINED_LINE_CAPACITY_BYTES: usize = 64 * 1024;
const MIN_BUFFER_GROWTH_BYTES: usize = 1024;

pub(super) enum DecodeStep {
    Frame(SseFrame),
    NeedInput,
    Error(rquest::Error),
}

pub(super) struct BoundedSseDecoder {
    input: Bytes,
    line: Vec<u8>,
    event_name: Option<String>,
    data: String,
    has_data: bool,
    pending_frame_bytes: usize,
    max_frame_bytes: usize,
}

impl BoundedSseDecoder {
    pub(super) fn new(max_frame_bytes: usize) -> Self {
        Self {
            input: Bytes::new(),
            line: Vec::new(),
            event_name: None,
            data: String::new(),
            has_data: false,
            pending_frame_bytes: 0,
            max_frame_bytes,
        }
    }

    pub(super) fn push_chunk(&mut self, chunk: Bytes) {
        debug_assert!(self.input.is_empty());
        self.input = chunk;
    }

    pub(super) fn decode_next(&mut self) -> DecodeStep {
        while !self.input.is_empty() {
            let segment_len = self
                .input
                .iter()
                .position(|byte| *byte == b'\n')
                .map(|position| position + 1)
                .unwrap_or(self.input.len());
            let segment = self.input.split_to(segment_len);
            if let Err(error) = self.append_segment(&segment) {
                self.clear();
                return DecodeStep::Error(error);
            }
            if segment.last() == Some(&b'\n') {
                match self.finish_line() {
                    Ok(Some(frame)) => return DecodeStep::Frame(frame),
                    Ok(None) => {}
                    Err(error) => {
                        self.clear();
                        return DecodeStep::Error(error);
                    }
                }
            }
        }

        DecodeStep::NeedInput
    }

    pub(super) fn flush_pending_frame(&mut self) -> Option<SseFrame> {
        // Preserve the translators' historical EOF behavior: a final line
        // without a newline is ignored, while already parsed fields are flushed.
        self.input = Bytes::new();
        self.line = Vec::new();
        self.pending_frame_bytes = 0;
        self.finish_frame()
    }

    fn append_segment(&mut self, segment: &[u8]) -> Result<(), rquest::Error> {
        let next_len = self
            .pending_frame_bytes
            .checked_add(segment.len())
            .filter(|length| *length <= self.max_frame_bytes)
            .ok_or_else(|| {
                stream_decode_error(format!(
                    "translated SSE frame exceeded the {}-byte limit",
                    self.max_frame_bytes
                ))
            })?;

        let required_line_bytes = self.line.len().checked_add(segment.len()).ok_or_else(|| {
            stream_decode_error("translated SSE line length overflow".to_string())
        })?;
        if required_line_bytes > self.line.capacity() {
            let target_capacity = bounded_growth_capacity(
                self.line.capacity(),
                required_line_bytes,
                self.max_frame_bytes,
            );
            self.line
                .try_reserve_exact(target_capacity.saturating_sub(self.line.len()))
                .map_err(|error| {
                    stream_decode_error(format!(
                        "failed to reserve translated SSE line buffer: {error}"
                    ))
                })?;
        }
        self.line.extend_from_slice(segment);
        self.pending_frame_bytes = next_len;
        Ok(())
    }

    fn finish_line(&mut self) -> Result<Option<SseFrame>, rquest::Error> {
        let mut line = std::mem::take(&mut self.line);
        line.pop();
        if line.last() == Some(&b'\r') {
            line.pop();
        }
        let frame_boundary = line.is_empty();
        let result = match std::str::from_utf8(&line) {
            Ok(line) => self.parse_line(line),
            Err(_) => Ok(None),
        };
        if line.capacity() <= MAX_RETAINED_LINE_CAPACITY_BYTES {
            line.clear();
            self.line = line;
        }
        if frame_boundary {
            self.pending_frame_bytes = 0;
        }
        result
    }

    fn parse_line(&mut self, line: &str) -> Result<Option<SseFrame>, rquest::Error> {
        if line.is_empty() {
            return Ok(self.finish_frame());
        }
        if line.starts_with(':') {
            return Ok(None);
        }

        let (field, value) = if let Some(colon_position) = line.find(':') {
            let value = &line[colon_position + 1..];
            (
                &line[..colon_position],
                value.strip_prefix(' ').unwrap_or(value),
            )
        } else {
            (line, "")
        };

        match field {
            "data" => {
                let separator_len = usize::from(self.has_data);
                let additional_bytes = separator_len.saturating_add(value.len());
                let required_data_bytes = self
                    .data
                    .len()
                    .checked_add(additional_bytes)
                    .ok_or_else(|| {
                        stream_decode_error("translated SSE data length overflow".to_string())
                    })?;
                if required_data_bytes > self.data.capacity() {
                    let target_capacity = bounded_growth_capacity(
                        self.data.capacity(),
                        required_data_bytes,
                        self.max_frame_bytes,
                    );
                    self.data
                        .try_reserve_exact(target_capacity.saturating_sub(self.data.len()))
                        .map_err(|error| {
                            stream_decode_error(format!(
                                "failed to reserve translated SSE data buffer: {error}"
                            ))
                        })?;
                }
                if self.has_data {
                    self.data.push('\n');
                }
                self.data.push_str(value);
                self.has_data = true;
            }
            "event" => {
                let mut event_name = String::new();
                event_name.try_reserve_exact(value.len()).map_err(|error| {
                    stream_decode_error(format!(
                        "failed to reserve translated SSE event buffer: {error}"
                    ))
                })?;
                event_name.push_str(value);
                self.event_name = Some(event_name);
            }
            "id" | "retry" => {}
            _ => {}
        }

        Ok(None)
    }

    fn finish_frame(&mut self) -> Option<SseFrame> {
        if !self.has_data && self.event_name.is_none() {
            return None;
        }
        self.has_data = false;
        Some(SseFrame {
            event_name: self.event_name.take(),
            data: std::mem::take(&mut self.data),
        })
    }

    pub(super) fn clear(&mut self) {
        self.input = Bytes::new();
        self.line = Vec::new();
        self.event_name = None;
        self.data = String::new();
        self.has_data = false;
        self.pending_frame_bytes = 0;
    }
}

fn bounded_growth_capacity(
    current_capacity: usize,
    required_capacity: usize,
    max_capacity: usize,
) -> usize {
    current_capacity
        .saturating_mul(2)
        .max(MIN_BUFFER_GROWTH_BYTES)
        .max(required_capacity)
        .min(max_capacity)
}

fn stream_decode_error(message: String) -> rquest::Error {
    let source = std::io::Error::new(std::io::ErrorKind::InvalidData, message);
    rquest::Error::from(serde_json::Error::io(source))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_frame(step: DecodeStep) -> SseFrame {
        match step {
            DecodeStep::Frame(frame) => frame,
            DecodeStep::NeedInput => panic!("expected frame, got input request"),
            DecodeStep::Error(error) => panic!("expected frame, got {error}"),
        }
    }

    fn expect_error(step: DecodeStep) -> String {
        match step {
            DecodeStep::Error(error) => error.to_string(),
            DecodeStep::Frame(frame) => panic!("expected error, got {frame:?}"),
            DecodeStep::NeedInput => panic!("expected error, got input request"),
        }
    }

    #[test]
    fn accepts_exact_limit_across_chunks() {
        let mut decoder = BoundedSseDecoder::new(8);
        decoder.push_chunk(Bytes::from_static(b"data:"));
        assert!(matches!(decoder.decode_next(), DecodeStep::NeedInput));
        decoder.push_chunk(Bytes::from_static(b"x\n\n"));

        assert_eq!(expect_frame(decoder.decode_next()).data, "x");
        assert!(matches!(decoder.decode_next(), DecodeStep::NeedInput));
    }

    #[test]
    fn lazily_decodes_multiple_frames_from_one_chunk() {
        let mut decoder = BoundedSseDecoder::new(8);
        decoder.push_chunk(Bytes::from_static(b"data:a\n\ndata:b\n\n"));

        assert_eq!(expect_frame(decoder.decode_next()).data, "a");
        assert_eq!(expect_frame(decoder.decode_next()).data, "b");
        assert!(matches!(decoder.decode_next(), DecodeStep::NeedInput));
    }

    #[test]
    fn rejects_unterminated_frame_across_chunks() {
        let mut decoder = BoundedSseDecoder::new(8);
        decoder.push_chunk(Bytes::from_static(b"1234"));
        assert!(matches!(decoder.decode_next(), DecodeStep::NeedInput));
        decoder.push_chunk(Bytes::from_static(b"56789"));

        assert!(expect_error(decoder.decode_next()).contains("8-byte limit"));
    }

    #[test]
    fn counts_lines_retained_until_frame_boundary() {
        let mut decoder = BoundedSseDecoder::new(15);
        decoder.push_chunk(Bytes::from_static(b"data:a\ndata:b\n"));
        assert!(matches!(decoder.decode_next(), DecodeStep::NeedInput));
        decoder.push_chunk(Bytes::from_static(b"x\n"));

        assert!(expect_error(decoder.decode_next()).contains("15-byte limit"));
    }

    #[test]
    fn preserves_multiline_data_crlf_and_event_name() {
        let mut decoder = BoundedSseDecoder::new(64);
        decoder.push_chunk(Bytes::from_static(
            b"event: update\r\ndata: first\r\ndata:second\r\n\r\n",
        ));

        let frame = expect_frame(decoder.decode_next());
        assert_eq!(frame.event_name.as_deref(), Some("update"));
        assert_eq!(frame.data, "first\nsecond");
    }

    #[test]
    fn ignores_invalid_utf8_line_without_resetting_frame() {
        let mut decoder = BoundedSseDecoder::new(64);
        decoder.push_chunk(Bytes::from_static(b"data:a\n\xff\ndata:b\n\n"));

        assert_eq!(expect_frame(decoder.decode_next()).data, "a\nb");
    }

    #[test]
    fn eof_ignores_unterminated_line_but_flushes_parsed_fields() {
        let mut decoder = BoundedSseDecoder::new(64);
        decoder.push_chunk(Bytes::from_static(b"event: update\ndata:a\nunfinished"));
        assert!(matches!(decoder.decode_next(), DecodeStep::NeedInput));

        let frame = decoder
            .flush_pending_frame()
            .expect("parsed fields before the unfinished line should flush");
        assert_eq!(frame.event_name.as_deref(), Some("update"));
        assert_eq!(frame.data, "a");
    }

    #[test]
    fn yields_completed_frame_before_later_same_chunk_overflow() {
        let mut decoder = BoundedSseDecoder::new(8);
        decoder.push_chunk(Bytes::from_static(b"data:a\n\n123456789"));

        assert_eq!(expect_frame(decoder.decode_next()).data, "a");
        assert!(expect_error(decoder.decode_next()).contains("8-byte limit"));
    }

    #[test]
    fn bounded_growth_is_geometric_and_never_exceeds_limit() {
        assert_eq!(bounded_growth_capacity(0, 1, 4096), 1024);
        assert_eq!(bounded_growth_capacity(1024, 1025, 4096), 2048);
        assert_eq!(bounded_growth_capacity(2048, 4095, 4096), 4096);
        assert_eq!(bounded_growth_capacity(4096, 4096, 4096), 4096);
    }
}
