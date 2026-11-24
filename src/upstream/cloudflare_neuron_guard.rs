//! Workers AI account-wide analytics gate. Refresh on demand; unknown usage is not zero.
use crate::{error::GatewayError, protocol::cloudflare_workers_ai as cf};
use futures::StreamExt;
use serde::Deserialize;
use std::{collections::HashMap, sync::Arc, time::Duration};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tokio::sync::Mutex;

const URL: &str = "https://api.cloudflare.com/client/v4/graphql";
const STOP: f64 = 9_000.0;
const TTL: time::Duration = time::Duration::seconds(30);
const QUERY: &str = r#"query WorkersAiUsage($account: string!, $start: Time!, $end: Time!) {
    viewer { accounts(filter: {accountTag: $account}) {
        total: aiInferenceAdaptiveGroups(
            filter: {datetime_geq: $start, datetime_leq: $end}, limit: 1
        ) { sum { totalNeurons } }
    } }
}"#;

#[derive(Default)]
pub(crate) struct Guard {
    accounts: Mutex<HashMap<String, Arc<Mutex<Option<Snapshot>>>>>,
}

struct Snapshot {
    checked_at: OffsetDateTime,
    usage: Option<f64>,
}

pub(crate) struct Check {
    guard: Arc<Guard>,
    http: rquest::Client,
    account: String,
    token: String,
    #[cfg(test)]
    fixture_url: Option<String>,
}

impl Check {
    pub(crate) fn for_payload(
        guard: &Arc<Guard>,
        http: &rquest::Client,
        payload: &crate::routing::candidate::ProviderAccountPayload,
    ) -> Result<Option<Self>, GatewayError> {
        if !cf::owns_payload(payload) {
            return Ok(None);
        }
        let endpoint = cf::Endpoint::parse(&payload.base_url).ok_or_else(cf::invalid)?;
        Ok(Some(Self {
            guard: guard.clone(),
            http: http.clone(),
            account: endpoint.account_id().into(),
            token: payload.api_key.clone(),
            #[cfg(test)]
            fixture_url: None,
        }))
    }

    pub(crate) async fn admit(&self) -> Result<(), GatewayError> {
        let cell = {
            let mut accounts = self.guard.accounts.lock().await;
            if accounts.len() >= 1024 && !accounts.contains_key(&self.account) {
                return Err(unavailable());
            }
            accounts.entry(self.account.clone()).or_default().clone()
        };
        let mut last = cell.lock().await;
        let now = OffsetDateTime::now_utc();
        if let Some(snapshot) = &*last {
            if let Some(decision) = snapshot.cached_decision(now) {
                return decision;
            }
        }
        let usage = self.read_usage(now).await;
        let updated = OffsetDateTime::now_utc();
        if updated.date() != now.date() {
            return Err(unavailable());
        }
        let snapshot = Snapshot {
            checked_at: updated,
            usage,
        };
        let decision = snapshot.decision(updated);
        *last = Some(snapshot);
        decision
    }

    async fn read_usage(&self, now: OffsetDateTime) -> Option<f64> {
        let start = now.date().midnight().assume_utc().format(&Rfc3339).ok()?;
        let end = now.format(&Rfc3339).ok()?;
        let url = URL;
        #[cfg(test)]
        let url = self.fixture_url.as_deref().unwrap_or(url);
        let response = self
            .http
            .post(url)
            .default_headers(false)
            .redirect(rquest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .bearer_auth(&self.token)
            .json(&serde_json::json!({
                "query": QUERY, "variables": {"account": self.account, "start": start, "end": end}
            }))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.ok()?;
            if chunk.len() > 64 * 1024 - body.len() {
                return None;
            }
            body.extend_from_slice(&chunk);
        }
        parse_usage(&body)
    }
}

impl Snapshot {
    fn cached_decision(&self, now: OffsetDateTime) -> Option<Result<(), GatewayError>> {
        let age = now - self.checked_at;
        if now.date() != self.checked_at.date() || age.is_negative() {
            return None;
        }
        let ttl = if self.usage.is_some() {
            TTL
        } else {
            time::Duration::seconds(5)
        };
        (self.usage.is_some_and(|n| n >= STOP) || age < ttl).then(|| self.decision(now))
    }

    fn decision(&self, now: OffsetDateTime) -> Result<(), GatewayError> {
        match self.usage {
            Some(n) if n < STOP => Ok(()),
            Some(_) => {
                let reset = (self.checked_at.date() + time::Duration::days(1))
                    .midnight()
                    .assume_utc();
                let retry = (reset - now).whole_milliseconds().max(0) as u64;
                let reset = reset
                    .format(&Rfc3339)
                    .expect("UTC quota reset is representable");
                Err(GatewayError::rate_limited(
                    format!("Cloudflare Workers AI reached the 9000 Neuron safety threshold; resumes after {reset}."),
                    retry,
                ).with_code("cloudflare_neuron_budget_paused"))
            }
            None => Err(unavailable()),
        }
    }
}

fn unavailable() -> GatewayError {
    GatewayError::service_unavailable(
        "Cloudflare Workers AI usage could not be verified; inference was not sent. Check Account Analytics Read permission.",
    )
    .with_code("cloudflare_neuron_usage_unavailable")
}

#[derive(Deserialize)]
struct Analytics {
    data: Option<Data>,
    // Cloudflare emits `errors: null` on successful queries.
    errors: Option<Vec<serde_json::Value>>,
}
#[derive(Deserialize)]
struct Data {
    viewer: Viewer,
}
#[derive(Deserialize)]
struct Viewer {
    accounts: Vec<Account>,
}
#[derive(Deserialize)]
struct Account {
    total: Vec<Aggregate>,
}
#[derive(Deserialize)]
struct Aggregate {
    sum: Totals,
}
#[derive(Deserialize)]
struct Totals {
    #[serde(rename = "totalNeurons")]
    total_neurons: f64,
}

fn parse_usage(body: &[u8]) -> Option<f64> {
    let response: Analytics = serde_json::from_slice(body).ok()?;
    if response.errors.is_some_and(|errors| !errors.is_empty()) {
        return None;
    }
    match response.data?.viewer.accounts.as_slice() {
        // An explicit empty aggregate in an accessible account means no samples.
        [Account { total }] if total.is_empty() => Some(0.0),
        [Account { total }] if total.len() == 1 => {
            let value = total[0].sum.total_neurons;
            (value.is_finite() && value >= 0.0).then_some(value)
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests;
