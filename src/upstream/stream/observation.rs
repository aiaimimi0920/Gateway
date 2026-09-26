//! Bounded line admission that leaves each observer's SSE interpretation intact.

const MAX_FRAME_BYTES: usize = 64 * 1024 * 1024;
const MAX_FRAME_LINES: usize = 65_536;
const MAX_RETAINED_LINE_BYTES: usize = 64 * 1024;
const MIN_GROWTH_BYTES: usize = 1024;

#[derive(Clone, Copy)]
pub(super) enum BoundaryRule {
    Empty,
    TrimCarriageReturns,
}

pub(super) enum ObservationLine<'a> {
    Text(&'a str),
    Reset,
}

#[derive(Debug)]
pub(super) struct ObservationLines {
    line: Vec<u8>,
    frame_bytes: usize,
    frame_lines: usize,
    line_length: usize,
    carriage_returns_only: bool,
    discarding: bool,
    max_bytes: usize,
    max_lines: usize,
}

impl Default for ObservationLines {
    fn default() -> Self {
        Self::new(MAX_FRAME_BYTES, MAX_FRAME_LINES)
    }
}

impl ObservationLines {
    fn new(max_bytes: usize, max_lines: usize) -> Self {
        Self {
            line: Vec::new(),
            frame_bytes: 0,
            frame_lines: 0,
            line_length: 0,
            carriage_returns_only: true,
            discarding: false,
            max_bytes,
            max_lines,
        }
    }

    pub(super) fn feed(
        &mut self,
        mut chunk: &[u8],
        boundary_rule: BoundaryRule,
        mut observe: impl FnMut(ObservationLine<'_>),
    ) {
        while !chunk.is_empty() {
            let length = chunk
                .iter()
                .position(|byte| *byte == b'\n')
                .map_or(chunk.len(), |position| position + 1);
            let (segment, remaining) = chunk.split_at(length);
            chunk = remaining;
            let terminated = segment.last() == Some(&b'\n');
            let content = if terminated {
                &segment[..segment.len() - 1]
            } else {
                segment
            };
            // These bounded flags find delimiters even while oversized lines are discarded.
            self.line_length = self.line_length.saturating_add(content.len()).min(2);
            self.carriage_returns_only =
                self.carriage_returns_only && content.iter().all(|byte| *byte == b'\r');
            if !self.discarding && !self.admit(content, segment.len(), terminated) {
                self.line = Vec::new();
                self.frame_bytes = 0;
                self.frame_lines = 0;
                self.discarding = true;
                observe(ObservationLine::Reset);
            }
            if !terminated {
                continue;
            }
            let boundary = self.carriage_returns_only
                && (matches!(boundary_rule, BoundaryRule::TrimCarriageReturns)
                    || self.line_length <= 1);
            if !self.discarding {
                if self.line.last() == Some(&b'\r') {
                    self.line.pop();
                }
                if let Ok(line) = std::str::from_utf8(&self.line) {
                    observe(ObservationLine::Text(line));
                }
            }
            self.line.clear();
            if self.line.capacity() > MAX_RETAINED_LINE_BYTES {
                self.line = Vec::new();
            }
            self.line_length = 0;
            self.carriage_returns_only = true;
            if boundary {
                self.frame_bytes = 0;
                self.frame_lines = 0;
                self.discarding = false;
            }
        }
    }

    fn admit(&mut self, content: &[u8], raw_bytes: usize, terminated: bool) -> bool {
        let Some(bytes) = self
            .frame_bytes
            .checked_add(raw_bytes)
            .filter(|bytes| *bytes <= self.max_bytes)
        else {
            return false;
        };
        let Some(lines) = self
            .frame_lines
            .checked_add(usize::from(terminated))
            .filter(|lines| *lines <= self.max_lines)
        else {
            return false;
        };
        let Some(required) = self.line.len().checked_add(content.len()) else {
            return false;
        };
        // The raw frame check also bounds this partial line before geometric allocation.
        if required > self.line.capacity() {
            let target = self
                .line
                .capacity()
                .saturating_mul(2)
                .max(required)
                .max(MIN_GROWTH_BYTES)
                .min(self.max_bytes);
            if self
                .line
                .try_reserve_exact(target.saturating_sub(self.line.len()))
                .is_err()
            {
                return false;
            }
        }
        self.line.extend_from_slice(content);
        self.frame_bytes = bytes;
        self.frame_lines = lines;
        true
    }
}

#[cfg(test)]
mod tests;
