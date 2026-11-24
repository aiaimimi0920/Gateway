//! Fixed creation tools. Business sessions outlive individual MCP transport connections.
mod facts;
mod image_parameters;
mod options;
mod profiles;
mod questions;
mod reuse;
mod review;
mod video;
mod wire;
pub(crate) use super::McpError as Error;
use super::McpError;
pub(crate) use facts::{assets, cancellation, progress, session, usage};
pub(crate) use options::Options;
pub(crate) use reuse::Reuse;
#[cfg(test)]
pub(crate) use review::review;
pub(crate) use review::{delivered, digest, review_assets, select_review, session_update, Review};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
pub(crate) use wire::{call, endpoint, Tool};

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Request {
    pub kind: Kind,
    pub prompt: String,
    pub budget_credits: u32,
    // Omission keeps the original request hash stable for tasks prepared before options existed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<Options>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse: Option<Reuse>,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum Kind {
    Image,
    Video,
}
impl Kind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
        }
    }
    pub(crate) fn model(self) -> &'static str {
        match self {
            Self::Image => "luma_uni_1",
            Self::Video => "luma_ray3_2",
        }
    }
}
impl Request {
    pub(crate) fn validate(&self) -> Result<(), McpError> {
        if self.prompt.trim().is_empty()
            || self.prompt.chars().count() > 6000
            || !(1..=100_000_000).contains(&self.budget_credits)
        {
            return Err(McpError::InvalidResponse);
        }
        if let Some(options) = &self.options {
            options.pack(self.kind, self.reuse.as_ref())?;
        } else if self.reuse.is_some() {
            Options::default().pack(self.kind, self.reuse.as_ref())?;
        }
        if let Some(reuse) = &self.reuse {
            reuse.assets(self.kind)?;
        }
        Ok(())
    }
    pub(crate) fn model(&self) -> &str {
        self.options
            .as_ref()
            .map_or(self.kind.model(), |options| options.model(self.kind))
    }
    pub(crate) fn operation(&self) -> Result<&'static str, McpError> {
        self.options
            .as_ref()
            .map_or(Ok(self.kind.name()), |options| options.operation(self.kind))
    }
    pub(crate) fn open(&self, key: &str) -> Value {
        json!({"workspace_key":key,"user_id":key,"idempotency_key":format!("{key}-open"),
            "budget_credits":self.budget_credits})
    }
    pub(crate) fn generate(&self, key: &str, session: &str) -> Result<(Tool, Value), McpError> {
        let tool = match self.kind {
            Kind::Image => Tool::Image,
            Kind::Video => Tool::Video,
        };
        let mut body = json!({"model":self.kind.model(),"type":self.kind.name(),"prompt":self.prompt,
            "user_id":key,"idempotency_key":format!("{key}-generate"),"session_id":session});
        if self.options.is_some() || self.reuse.is_some() {
            body.as_object_mut()
                .ok_or(McpError::InvalidResponse)?
                .extend(
                    self.options
                        .clone()
                        .unwrap_or_default()
                        .pack(self.kind, self.reuse.as_ref())?,
                );
        }
        Ok((tool, body))
    }
}

// Luma identifiers may be bare or kind-prefixed. Never accept a pair or an arbitrary URI.
pub(crate) fn reference(raw: &Value, kind: &str) -> Result<String, McpError> {
    let value = raw.as_str().ok_or(McpError::InvalidResponse)?;
    let id = value.strip_prefix(&format!("{kind}:")).unwrap_or(value);
    if id.is_empty()
        || id.len() > 128
        || !id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_-".contains(&b))
    {
        return Err(McpError::InvalidResponse);
    }
    Ok(id.to_owned())
}
pub(crate) fn run(value: &Value) -> Result<String, McpError> {
    if value["kind"] != "generate" || !value["state"].is_string() {
        return Err(McpError::InvalidResponse);
    }
    reference(&value["run_id"], "run")
}
