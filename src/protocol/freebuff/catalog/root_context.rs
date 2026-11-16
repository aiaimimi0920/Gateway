//! Root-only provider-usage calibration and runtime budget, separate from credits or model limits.
pub(super) mod estimate;
mod request;
use super::{completion_decode::MAX_SAFE_INTEGER, BusinessError};
pub(crate) use request::RootRequest;
use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ContextForecast {
    pub(crate) estimated_tokens: u64,
    pub(crate) tokens: u64,
    pub(crate) limit: u64,
    pub(crate) calibrated: bool,
}
#[derive(Clone, Copy)]
pub(super) struct RootPreview {
    pub(super) forecast: ContextForecast,
    pub(super) reported_tokens: Option<u64>,
    pub(super) fixed_tokens: u64,
}
struct Baseline {
    fixed: [u8; 32],
    prefix: Vec<[u8; 32]>,
    tokens: u64,
    estimated: u64,
    scale: f64,
}
#[derive(Default)]
pub(super) struct ContextState {
    baseline: Option<Baseline>,
    pending: Option<RootRequest>,
    forecast: Option<ContextForecast>,
}
impl ContextState {
    pub(super) fn begin(&mut self) {
        self.pending = None;
        self.forecast = None;
    }
    pub(super) fn reset(&mut self) {
        *self = Self::default();
    }
    pub(super) fn forecast(&self) -> Option<ContextForecast> {
        self.forecast
    }
    /// Read-only decisions use only a matching work receipt, never an auxiliary summary's usage.
    pub(super) fn preview(&self, request: &RootRequest) -> RootPreview {
        let baseline = self
            .baseline
            .as_ref()
            .filter(|old| old.fixed == request.fixed && request.history.starts_with(&old.prefix));
        let adjusted = baseline.map(|old| {
            (old.tokens as f64
                + (request.estimated as f64 - old.estimated as f64) * old.scale
                + 0.5)
                .floor()
        });
        let calibrated =
            adjusted.is_some_and(|n| n.is_finite() && (0.0..=MAX_SAFE_INTEGER as f64).contains(&n));
        let tokens = if calibrated {
            adjusted.unwrap() as u64
        } else {
            request.estimated
        };
        RootPreview {
            forecast: ContextForecast {
                estimated_tokens: request.estimated,
                tokens,
                limit: request.limit,
                calibrated,
            },
            reported_tokens: baseline.map(|old| old.tokens),
            fixed_tokens: request.fixed_tokens,
        }
    }
    pub(super) fn prepare(&mut self, request: RootRequest) -> Result<(), BusinessError> {
        let preview = self.preview(&request);
        if preview.reported_tokens.is_none() {
            self.baseline = None;
        }
        self.pending = None;
        self.forecast = Some(preview.forecast);
        if preview.forecast.tokens > request.limit {
            return Err(BusinessError::ContextBudgetExceeded);
        }
        self.pending = Some(request);
        Ok(())
    }
    pub(super) fn observe(&mut self, completion: Option<&Value>) {
        let Some(mut request) = self.pending.take() else {
            return;
        };
        let Some(body) = completion else {
            return;
        };
        let input = body
            .pointer("/usage/prompt_tokens")
            .and_then(Value::as_u64)
            .filter(|n| *n > 0 && *n <= MAX_SAFE_INTEGER);
        let output = body
            .pointer("/usage/completion_tokens")
            .and_then(Value::as_u64)
            .filter(|n| *n <= MAX_SAFE_INTEGER);
        let (Some(input), Some(output)) = (input, output) else {
            return;
        };
        // Cached tokens are already part of prompt_tokens. Do not add them or cumulative totals.
        let Some(tokens) = input.checked_add(output).filter(|n| *n <= MAX_SAFE_INTEGER) else {
            return;
        };
        let Some(choices) = body
            .get("choices")
            .and_then(Value::as_array)
            .filter(|c| c.len() == 1)
        else {
            return;
        };
        let message = &choices[0]["message"];
        if message["role"] != "assistant" {
            return;
        }
        let Ok(output_estimate) = estimate::message(message) else {
            return;
        };
        let Some(estimated) = request
            .estimated
            .checked_add(output_estimate)
            .filter(|n| *n <= MAX_SAFE_INTEGER)
        else {
            return;
        };
        request.history.push(request::fingerprint(message));
        self.baseline = Some(Baseline {
            fixed: request.fixed,
            prefix: request.history,
            tokens,
            estimated,
            scale: if request.estimated > 0 {
                (input as f64 / request.estimated as f64).clamp(0.5, 2.5)
            } else {
                1.0
            },
        });
    }
}
// Scalar diagnostics only. No fingerprints, request text, schemas or provider receipts in Debug.
impl std::fmt::Debug for ContextState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FreeBuffRootContext { facts: <redacted> }")
    }
}
#[cfg(test)]
mod estimate_tests;
#[cfg(test)]
mod tests;
