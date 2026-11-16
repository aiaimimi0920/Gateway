//! Local consistency witness, not a MAC, credential or verification of the model's opaque signature.
use super::*;
use sha2::{Digest, Sha256};
use std::io::Write;

const SOURCE: &str = "aws_bedrock_converse";
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Continuation {
    source: String,
    pub(crate) model: String,
    request_sha256: String,
}

struct HashWriter(Sha256);
impl Write for HashWriter {
    fn write(&mut self, data: &[u8]) -> std::io::Result<usize> {
        self.0.update(data);
        Ok(data.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
#[derive(Serialize)]
struct Prefix<'a> {
    messages: &'a [Value],
    system: Option<&'a Value>,
}

fn digest(messages: &[Value], system: Option<&Value>) -> Result<String, GatewayError> {
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(&mut writer, &Prefix { messages, system }).map_err(|_| invalid())?;
    Ok(format!("{:x}", writer.0.finalize()))
}
impl Continuation {
    pub(crate) fn validate(&self) -> Result<(), GatewayError> {
        if self.source != SOURCE
            || self.model.is_empty()
            || self.model.len() > 2048
            || self.request_sha256.len() != 64
            || !self
                .request_sha256
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        {
            return Err(invalid());
        }
        Ok(())
    }
    pub(crate) fn from_body(body: &Value, model: &str) -> Result<Self, GatewayError> {
        let messages = body
            .get("messages")
            .and_then(Value::as_array)
            .ok_or_else(invalid)?;
        let scope = Self {
            source: SOURCE.into(),
            model: model.into(),
            request_sha256: digest(messages, body.get("system"))?,
        };
        scope.validate()?;
        Ok(scope)
    }
    pub(crate) fn matches(
        &self,
        messages: &[Value],
        system: Option<&Value>,
        model: &str,
    ) -> Result<bool, GatewayError> {
        self.validate()?;
        Ok(self.model == model && self.request_sha256 == digest(messages, system)?)
    }
}

pub(crate) fn bind_response(
    response: &mut crate::protocol::canonical::CanonicalRelayResponse,
    body: &Value,
    model: &str,
) -> Result<(), GatewayError> {
    if let Some(turn) = &mut response.bedrock_turn {
        turn.continuation = Some(Continuation::from_body(body, model)?);
        super::wire::validate(turn)?;
    }
    Ok(())
}
