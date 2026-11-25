//! Pause only the metered side of a Vercel key after an upstream credit rejection.
use super::common::RequestPlan;
use crate::{
    error::{classify_upstream_error, retry_after_from_headers, GatewayError},
    protocol::{upstream_body::collect_bounded_upstream_charset_text_with_provider, vercel_free},
    routing::candidate::ProviderAccountPayload,
};
use futures::StreamExt;
use rquest::{header::HeaderValue, Client, Response};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::HashSet,
    sync::OnceLock,
    time::{Duration, Instant, SystemTime},
};
use tokio::sync::Mutex;

const MODELS: &str = "https://ai-gateway.vercel.sh/v1/models";
const CREDITS: &str = "https://ai-gateway.vercel.sh/v1/credits";
const MAX_CREDIT: f64 = 5.0;
const TTL: Duration = Duration::from_secs(30);
const NEGATIVE_TTL: Duration = Duration::from_secs(5);
const RECHECK: Duration = Duration::from_secs(3600);
#[path = "vercel_credit_ledger.rs"]
mod ledger;
use ledger::{Ledger, Permit};

#[derive(Default)]
struct Guard {
    catalog: Mutex<Option<(Instant, Option<HashSet<String>>)>>,
    ledger: Ledger,
}

type Key = [u8; 32];

fn guard() -> &'static Guard {
    static GUARD: OnceLock<Guard> = OnceLock::new();
    GUARD.get_or_init(Guard::default)
}

pub(crate) struct Check {
    http: Client,
    model: String,
    metered: Option<(Key, HeaderValue)>,
}

impl Check {
    pub(crate) fn for_plan(
        http: &Client,
        payload: &ProviderAccountPayload,
        plan: &RequestPlan,
    ) -> Result<Option<Self>, GatewayError> {
        if !vercel_free::owns_payload(payload) {
            return Ok(None);
        }
        let body = plan
            .body
            .as_ref()
            .and_then(Value::as_object)
            .ok_or_else(unavailable)?;
        let model = body
            .get("model")
            .and_then(Value::as_str)
            .ok_or_else(unavailable)?;
        let metered = if vercel_free::zero_model(model) {
            None
        } else {
            if !vercel_free::monthly_credit(payload) {
                return Err(GatewayError::bad_request(
                    "Vercel metered models require this account to opt in to monthly free credit.",
                )
                .with_code("vercel_monthly_credit_not_enabled"));
            }
            let key = payload.api_key.trim();
            let mut bearer =
                HeaderValue::from_str(&format!("Bearer {key}")).map_err(|_| unavailable())?;
            bearer.set_sensitive(true);
            Some((Sha256::digest(key.as_bytes()).into(), bearer))
        };
        Ok(Some(Self {
            http: http.clone(),
            model: model.into(),
            metered,
        }))
    }

    pub(crate) async fn admit(&self) -> Result<Option<Permit<'static>>, GatewayError> {
        let Some((key, bearer)) = &self.metered else {
            return guard()
                .check_zero(&self.http, &self.model)
                .await
                .map(|_| None);
        };
        guard()
            .ledger
            .admit(*key, Instant::now(), || {
                read_balance(&self.http, bearer.clone())
            })
            .await
            .map(Some)
    }
}

pub(crate) async fn payment_error(
    response: Response,
    provider: &str,
    permit: Permit<'_>,
) -> GatewayError {
    let delay = retry_after_from_headers(response.headers(), SystemTime::now());
    let body = match collect_bounded_upstream_charset_text_with_provider(
        response,
        "Vercel credit rejection",
        provider,
    )
    .await
    {
        Ok(body) => body,
        Err(error)
            if matches!(
                error.code.as_deref(),
                Some("upstream_body_too_large" | "upstream_body_buffer_allocation_failed")
            ) =>
        {
            return error
        }
        Err(_) => String::from("<unreadable body>"),
    };
    let upstream =
        || classify_upstream_error(402, &body, Some(provider)).with_retry_after_ms(delay);
    if !is_depletion(&body) {
        return upstream();
    }
    permit.pause(Instant::now());
    // Unlike a local rejection, this response must be attributed to the account that sent it.
    let mut error = paused()
        .with_code("vercel_monthly_credit_exhausted")
        .with_provider(provider);
    error.upstream_retry_after_ms = delay;
    error
}

impl Guard {
    async fn check_zero(&self, http: &Client, id: &str) -> Result<(), GatewayError> {
        let mut slot = self.catalog.lock().await;
        let refresh = slot.as_ref().is_none_or(|(when, cached)| {
            when.elapsed() >= if cached.is_some() { TTL } else { NEGATIVE_TTL }
        });
        if refresh {
            let fetched = get(http, MODELS, None, 4 * 1024 * 1024)
                .await
                .and_then(|json| {
                    let rows = json.get("data")?.as_array()?;
                    Some(
                        rows.iter()
                            .filter_map(|row| {
                                let id = row.get("id")?.as_str()?;
                                (vercel_free::zero_model(id) && zero_price(row))
                                    .then(|| id.to_owned())
                            })
                            .collect::<HashSet<String>>(),
                    )
                });
            *slot = Some((Instant::now(), fetched));
        }
        slot.as_ref()
            .and_then(|(_, prices)| prices.as_ref())
            .filter(|prices| prices.contains(id))
            .map(|_| ())
            .ok_or_else(unavailable)
    }
}

// Vercel's FAQ defines a gateway 402 other than quota_for_entity_exceeded
// as a team balance rejection. Require a Vercel-shaped JSON error (or an
// exact balance message) so an unrecognized intermediary error won't latch a key.
fn is_depletion(body: &str) -> bool {
    let json = serde_json::from_str::<Value>(body).unwrap_or(Value::Null);
    let error = json.get("error").filter(|v| v.is_object()).unwrap_or(&json);
    let fields = ["code", "type"].map(|field| error.get(field).and_then(Value::as_str));
    if fields.contains(&Some("quota_for_entity_exceeded")) {
        return false;
    }
    let message = error
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or(body.trim());
    fields.into_iter().flatten().any(|code| {
        matches!(
            code,
            "insufficient_balance" | "insufficient_credits" | "insufficient_funds"
        )
    }) || (json.get("error").is_some_and(Value::is_object)
        && fields
            .into_iter()
            .flatten()
            .any(|value| !value.trim().is_empty())
        && error
            .get("message")
            .and_then(Value::as_str)
            .is_some_and(|message| !message.trim().is_empty()))
        || [
            "insufficient balance",
            "insufficient credits",
            "insufficient credit balance",
        ]
        .iter()
        .any(|text| message.trim_end_matches('.').eq_ignore_ascii_case(text))
}

async fn read_balance(http: &Client, bearer: HeaderValue) -> Option<f64> {
    let credits = get(http, CREDITS, Some(bearer), 16 * 1024).await?;
    recognized_balance(&credits)
}

fn recognized_balance(credits: &Value) -> Option<f64> {
    let balance = dollars(credits.get("balance")?)?;
    (balance <= MAX_CREDIT && credits.get("token_pass").is_none_or(Value::is_null))
        .then_some(balance)
}

async fn get(http: &Client, url: &str, bearer: Option<HeaderValue>, limit: usize) -> Option<Value> {
    let request = http
        .get(url)
        .default_headers(false)
        .redirect(rquest::redirect::Policy::none())
        .timeout(Duration::from_secs(5));
    let request = if let Some(bearer) = bearer {
        request.header(rquest::header::AUTHORIZATION, bearer)
    } else {
        request
    };
    let response = request.send().await.ok()?;
    if !response.status().is_success() {
        return None;
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.ok()?;
        if chunk.len() > limit - body.len() {
            return None;
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).ok()
}

fn dollars(value: &Value) -> Option<f64> {
    let text = value.as_str()?;
    let number = text.parse::<f64>().ok()?;
    (number.is_finite()
        && (number > 0.0
            || (number == 0.0 && text.bytes().all(|byte| matches!(byte, b'0' | b'.')))))
    .then_some(number)
}

fn zero_price(row: &Value) -> bool {
    row.get("type").and_then(Value::as_str) == Some("language")
        && row
            .get("pricing")
            .and_then(Value::as_object)
            .is_some_and(|pricing| {
                pricing.contains_key("input")
                    && pricing.contains_key("output")
                    && pricing.iter().all(|(key, value)| {
                        matches!(
                            key.as_str(),
                            "input" | "output" | "input_cache_read" | "input_cache_write"
                        ) && dollars(value) == Some(0.0)
                    })
            })
}

fn paused() -> GatewayError {
    let mut error = GatewayError::bad_request(
        "Vercel metered models are paused for this key after a credit rejection; zero-price models remain eligible.",
    ).with_code("vercel_monthly_credit_paused");
    error.http_status = Some(402);
    error.fallback_hint = crate::error::FallbackHint::FallbackProvider {
        reason: "Vercel monthly credit is paused for this key".into(),
    };
    error
}

fn unavailable() -> GatewayError {
    let mut error = GatewayError::service_unavailable(
        "Vercel zero pricing or metered admission could not be verified; inference was not sent.",
    )
    .with_code("vercel_credit_unverified");
    error.retryable = false;
    error.fallback_hint = crate::error::FallbackHint::FallbackProvider {
        reason: "Vercel credit admission could not be verified".into(),
    };
    error
}

pub(crate) fn local_denial(error: &GatewayError) -> bool {
    matches!(
        error.code.as_deref(),
        Some(
            "vercel_monthly_credit_paused"
                | "vercel_monthly_credit_not_enabled"
                | "vercel_credit_unverified"
        )
    )
}

#[cfg(test)]
#[path = "vercel_credit_guard_tests.rs"]
mod tests;
