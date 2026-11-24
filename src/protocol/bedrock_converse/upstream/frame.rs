//! Admit one frame at a time. Validate the prelude before trusting lengths or allocating.
use bytes::Bytes;
use crc::{Crc, CRC_32_ISO_HDLC};

use super::{headers, invalid};
use crate::protocol::stream_error::ProtocolStreamError;

// This is Gateway's local retained-byte budget, NOT Smithy's service-only 24 MiB / 128 KiB rules.
const MAX_FRAME: usize = crate::protocol::upstream_body::MAX_ACCUMULATED_UPSTREAM_BODY_BYTES;
static CRC32: Crc<u32> = Crc::<u32>::new(&CRC_32_ISO_HDLC);

#[derive(Default)]
pub(super) struct Decoder {
    input: Bytes,
    frame: Vec<u8>,
    length: Option<usize>,
    strict_json: bool,
}

impl Decoder {
    pub fn strict_json() -> Self {
        Self {
            strict_json: true,
            ..Self::default()
        }
    }
    pub fn push(&mut self, input: Bytes) {
        debug_assert!(self.input.is_empty());
        self.input = input;
    }

    pub fn next(&mut self) -> Result<Option<headers::Event>, ProtocolStreamError> {
        while !self.input.is_empty() {
            let target = self.length.unwrap_or(12);
            let count = (target - self.frame.len()).min(self.input.len());
            let required = self.frame.len() + count;
            if required > self.frame.capacity() {
                let capacity = self
                    .frame
                    .capacity()
                    .saturating_mul(2)
                    .max(required)
                    .min(MAX_FRAME);
                self.frame
                    .try_reserve_exact(capacity - self.frame.len())
                    .map_err(|_| invalid("frame allocation failed"))?;
            }
            self.frame.extend_from_slice(&self.input.split_to(count));
            if self.frame.len() < target {
                continue;
            }
            if self.length.is_none() {
                let total = number(&self.frame[..4]);
                let headers = number(&self.frame[4..8]);
                if CRC32.checksum(&self.frame[..8]) != number(&self.frame[8..12]) as u32 {
                    return Err(invalid("prelude CRC mismatch"));
                }
                if !(16..=MAX_FRAME).contains(&total) || headers > total - 16 {
                    return Err(invalid(
                        "invalid frame structure or Gateway frame byte budget exceeded",
                    ));
                }
                self.length = Some(total);
                continue;
            }
            let frame = std::mem::take(&mut self.frame);
            self.length = None;
            let end = frame.len() - 4;
            if CRC32.checksum(&frame[..end]) != number(&frame[end..]) as u32 {
                return Err(invalid("message CRC mismatch"));
            }
            let headers_end = 12 + number(&frame[4..8]);
            return headers::parse(
                &frame[12..headers_end],
                &frame[headers_end..end],
                self.strict_json,
            )
            .map(Some);
        }
        Ok(None)
    }

    pub fn finish(&self) -> Result<(), ProtocolStreamError> {
        if !self.frame.is_empty() || !self.input.is_empty() {
            return Err(invalid("truncated frame at EOF"));
        }
        Ok(())
    }
}

fn number(bytes: &[u8]) -> usize {
    u32::from_be_bytes(bytes.try_into().expect("fixed four-byte wire field")) as usize
}
