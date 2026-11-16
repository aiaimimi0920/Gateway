//! Request-local estimate and compact history witnesses; no text, secrets or serialized buffers kept.
use super::{estimate, BusinessError};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::{self, Write};

pub(crate) struct RootRequest {
    pub(super) estimated: u64,
    pub(super) limit: u64,
    pub(super) fixed: [u8; 32],
    pub(super) history: Vec<[u8; 32]>,
    pub(super) fixed_tokens: u64,
}
impl RootRequest {
    pub(in crate::protocol::freebuff::catalog) fn new(
        body: &Value,
        history: &[usize],
        limit: u64,
    ) -> Result<Self, BusinessError> {
        let estimated = estimate::request(body)?;
        // Same estimator as the full request. Generated reminders are fixed, not canonical history.
        let history_tokens = history.iter().try_fold(0u64, |sum, index| {
            estimate::message(&body["messages"][*index]).map(|count| sum + count)
        })?;
        Ok(Self {
            estimated,
            fixed_tokens: estimated - history_tokens,
            limit,
            fixed: fingerprint(&(&body["messages"][0], &body["tools"])),
            history: history
                .iter()
                .map(|index| fingerprint(&body["messages"][*index]))
                .collect(),
        })
    }
}
struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
pub(super) fn fingerprint(value: &impl Serialize) -> [u8; 32] {
    let mut writer = HashWriter(Sha256::new());
    // Native Values and an infallible writer cannot produce a serialization failure.
    serde_json::to_writer(&mut writer, value).expect("native Value hash writer cannot fail");
    writer.0.finalize().into()
}
