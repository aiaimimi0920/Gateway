//! Closed failure receipt carried by the error owner, independent of local billing.
use serde_json::{json, Number, Value};

#[derive(Debug, Clone)]
pub struct LumaGenerationFailure {
    id: String,
    model: String,
    kind: &'static str,
    credits: Option<Number>,
}

impl LumaGenerationFailure {
    /// The product sender calls this only after binding task identity and validating usage.
    pub fn verified(
        id: String,
        model: String,
        kind: &'static str,
        credits: Option<Number>,
    ) -> Self {
        Self {
            id,
            model,
            kind,
            credits,
        }
    }

    pub fn record(&self, trace: &mut Value) -> String {
        trace["lumaAgentsGeneration"] = json!({"id":self.id,"model":self.model,
            "type":self.kind,"state":"failed","usage":{"credits_charged":self.credits}});
        self.id.clone()
    }
}
