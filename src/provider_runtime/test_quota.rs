//! Bounded uncached account observations, never request-attributed billing or token estimates.
use crate::{routing::config::CredentialProbeTarget, state::AppState};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::Duration;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TestQuotaObservation {
    pub status: String,
    pub unit: Option<String>,
    pub consumed: Option<f64>,
    pub before: Option<f64>,
    pub after: Option<f64>,
    pub source: Option<String>,
}

pub(super) struct Snapshot {
    remaining: f64,
    unit: String,
    window: Option<i64>,
    source: &'static str,
}

pub(super) async fn capture(state: &AppState, target: &CredentialProbeTarget) -> Option<Snapshot> {
    let payload = &target.payload;
    let codex = crate::protocol::chatgpt::official_api::is_chatgpt_codex_backend_payload(payload);
    let url = if codex {
        "https://chatgpt.com/backend-api/wham/usage".to_string()
    } else {
        balance_url(&payload.base_url, payload.balance_path.as_deref()?)?.to_string()
    };
    tokio::time::timeout(Duration::from_secs(2), async {
        let response = state
            .upstream_client
            .client()
            .get(url)
            // 自动观测不跟随跳转，避免非标准认证头泄露到供应商声明范围之外。
            .redirect(rquest::redirect::Policy::none())
            .headers(crate::upstream::headers::build_upstream_headers(payload))
            .send()
            .await
            .ok()?;
        if !response.status().is_success() {
            return None;
        }
        let stream = response.bytes_stream();
        futures::pin_mut!(stream);
        let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.ok()?;
            if bytes.len() + chunk.len() > 65_536 {
                return None;
            }
            bytes.extend_from_slice(&chunk);
        }
        parse(&serde_json::from_slice::<Value>(&bytes).ok()?, codex)
    })
    .await
    .ok()
    .flatten()
}

// 额度观测只能复用同一来源的凭据；跨来源账单接口需要独立认证合同。
fn balance_url(base_url: &str, path: &str) -> Option<url::Url> {
    if path.trim().is_empty() {
        return None;
    }
    let base = url::Url::parse(&format!("{}/", base_url.trim_end_matches('/'))).ok()?;
    let endpoint = base.join(path).ok()?;
    (matches!(base.scheme(), "http" | "https")
        && base.origin() == endpoint.origin()
        && base.username().is_empty()
        && base.password().is_none()
        && endpoint.username().is_empty()
        && endpoint.password().is_none()
        && endpoint.fragment().is_none())
    .then_some(endpoint)
}

fn number(value: &Value) -> Option<f64> {
    value
        .as_f64()
        .or_else(|| value.as_str()?.parse().ok())
        .filter(|n| n.is_finite() && *n >= 0.0)
}

fn parse(body: &Value, codex: bool) -> Option<Snapshot> {
    if codex {
        let window = body.get("rate_limit")?.get("primary_window")?;
        let used = number(window.get("used_percent")?)?;
        let seconds = window.get("limit_window_seconds")?.as_i64()?;
        if used > 100.0 || seconds <= 0 {
            return None;
        }
        return Some(Snapshot {
            remaining: 100.0 - used,
            unit: format!("quota-pp/{seconds}s"),
            window: Some(window.get("reset_at")?.as_i64()?),
            source: "codex-primary-window",
        });
    }
    // Do not recursively guess a numeric field: nested/token/ambiguous balances are unavailable.
    let (remaining, unit) = if let Some(value) = body.get("remaining_credits") {
        (number(value)?, "credits".to_string())
    } else {
        let unit = body.get("currency")?.as_str()?;
        if !matches!(unit, "USD" | "EUR" | "CNY" | "GBP" | "JPY") {
            return None;
        }
        (
            number(body.get("remaining").or_else(|| body.get("balance"))?)?,
            unit.to_string(),
        )
    };
    Some(Snapshot {
        remaining,
        unit,
        window: None,
        source: "provider-balance",
    })
}

pub(super) fn observation(
    before: Option<Snapshot>,
    after: Option<Snapshot>,
) -> TestQuotaObservation {
    let mut result = TestQuotaObservation {
        status: "unavailable".into(),
        unit: None,
        consumed: None,
        before: None,
        after: None,
        source: None,
    };
    let (Some(before), Some(after)) = (before, after) else {
        return result;
    };
    if before.unit != after.unit || before.source != after.source {
        return result;
    }
    result.unit = Some(before.unit);
    result.source = Some(before.source.into());
    result.before = Some(before.remaining);
    result.after = Some(after.remaining);
    let delta = before.remaining - after.remaining;
    result.status = if before.window != after.window || delta < 0.0 {
        "reset-or-replenished"
    } else if delta == 0.0 {
        "no-visible-change"
    } else {
        result.consumed = Some(delta);
        "account-observed"
    }
    .into();
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn balance_url_preserves_same_origin_relative_and_absolute_paths() {
        let base = "https://provider.example/v1";
        for path in [
            "balance",
            "/balance",
            "https://provider.example:443/balance",
        ] {
            let resolved = balance_url(base, path).expect("same-origin balance URL");
            assert_eq!(resolved.origin(), url::Url::parse(base).unwrap().origin());
        }
        assert_eq!(
            balance_url("http://127.0.0.1:4321/v1", "/balance")
                .unwrap()
                .as_str(),
            "http://127.0.0.1:4321/balance"
        );
    }

    #[test]
    fn balance_url_rejects_origin_changes_userinfo_and_empty_paths() {
        for path in [
            "https://other.example/balance",
            "//other.example/balance",
            "http://provider.example/balance",
            "https://provider.example:8443/balance",
            "https://user:secret@provider.example/balance",
            "/balance#fragment",
            "",
            "   ",
        ] {
            assert!(
                balance_url("https://provider.example/v1", path).is_none(),
                "{path}"
            );
        }
        assert!(balance_url("https://user:secret@provider.example", "/balance").is_none());
        assert!(balance_url("file:///tmp/", "balance").is_none());
    }

    #[test]
    fn observes_real_credit_delta_but_does_not_invent_free_or_request_attribution() {
        let snapshot = |n| parse(&json!({"remaining_credits": n}), false);
        let value = observation(snapshot(20), snapshot(17));
        assert_eq!(value.consumed, Some(3.0));
        assert_eq!(value.status, "account-observed");
        assert!(observation(snapshot(20), snapshot(20)).consumed.is_none());
        assert!(observation(snapshot(20), snapshot(21)).consumed.is_none());
        assert!(parse(&json!({"usage":{"total_tokens":12},"balance": 10}), false).is_none());
        assert!(parse(&json!({"remaining_credits": "NaN"}), false).is_none());
    }
    #[test]
    fn rejects_reset_windows_and_different_currency() {
        let window = |reset| {
            parse(
                &json!({"rate_limit":{"primary_window":{
            "used_percent":10,"reset_at":reset,"limit_window_seconds":18000}}}),
                true,
            )
        };
        assert_eq!(
            observation(window(1), window(2)).status,
            "reset-or-replenished"
        );
        let cash = |unit| parse(&json!({"currency":unit,"balance":12}), false);
        assert_eq!(observation(cash("USD"), cash("EUR")).status, "unavailable");
    }
}
