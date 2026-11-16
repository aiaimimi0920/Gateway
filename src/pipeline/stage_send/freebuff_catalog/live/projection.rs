//! Only protocol envelopes vary; every mode retains the same paid source and final FINISH boundary.
use super::*;
use serde_json::Value;
pub(super) enum Projection {
    Chat(chat::Projection),
    Projected(projected::Projection),
}
impl Projection {
    pub fn new(ctx: &PipelineContext) -> Self {
        if ctx.canonical_req.endpoint_kind == EndpointKind::ChatCompletions {
            Self::Chat(chat::Projection::new())
        } else {
            Self::Projected(projected::Projection::new(
                ctx.canonical_req.endpoint_kind,
                ctx.canonical_req
                    .requested_model
                    .as_deref()
                    .unwrap_or("unknown")
                    .to_owned(),
                ctx.req_id.to_string(),
            ))
        }
    }
    pub fn frame(&mut self, frame: Value) -> Result<Bytes, BusinessError> {
        match self {
            Self::Chat(projection) => projection.frame(frame),
            Self::Projected(projection) => projection.frame(frame),
        }
    }
    pub fn finish(&mut self, body: Value) -> Result<Bytes, BusinessError> {
        match self {
            Self::Chat(projection) => projection.finish(body),
            Self::Projected(projection) => projection.finish(body),
        }
    }
}
