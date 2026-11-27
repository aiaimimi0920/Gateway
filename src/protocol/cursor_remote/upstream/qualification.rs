//! Account identity verification, WorkOS session cookie encoding, and Free-plan guards.
use super::{
    json_wire::{
        is_canonical_int_string, parse_json, scalar_bool, scalar_bool_opt, scalar_i32,
        scalar_i32_opt, scalar_i64_safe, wire_fields,
    },
    transport::{CheckedClient, ENDPOINTS},
    ProbeError,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};
use subtle::ConstantTimeEq;

const MAX_SAFE_INT: i64 = 9_007_199_254_740_991;
const MIN_SAFE_INT: i64 = -9_007_199_254_740_991;

pub(crate) async fn verify_identity(
    token: &str,
    email_hash: &str,
    client: &CheckedClient<'_>,
) -> Result<String, ProbeError> {
    let resp = client.post_connect_json(ENDPOINTS.me, token, "{}").await?;
    let me = parse_json(resp.status, &resp.content_type, &resp.body)?;
    let claims = token_claims(token)?;
    bind_identity(&claims, &me, email_hash)
}

struct Claims {
    sub: String,
}

fn token_claims(token: &str) -> Result<Claims, ProbeError> {
    if token.len() > 32_768 {
        return Err(ProbeError::new("invalid_login_token"));
    }
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() != 3
        || parts.iter().any(|p| {
            p.is_empty()
                || !p
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        })
    {
        return Err(ProbeError::new("invalid_login_token"));
    }
    let decoded = URL_SAFE_NO_PAD
        .decode(parts[1])
        .map_err(|_| ProbeError::new("invalid_token_claims"))?;
    let payload: Value =
        serde_json::from_slice(&decoded).map_err(|_| ProbeError::new("invalid_token_claims"))?;
    let Some(obj) = payload.as_object() else {
        return Err(ProbeError::new("invalid_or_expired_token"));
    };
    let Some(sub) = obj.get("sub").and_then(Value::as_str) else {
        return Err(ProbeError::new("invalid_or_expired_token"));
    };
    if sub.is_empty()
        || sub.len() > 256
        || !sub
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"_|.-".contains(&b))
    {
        return Err(ProbeError::new("invalid_or_expired_token"));
    }
    let Some(exp) = obj
        .get("exp")
        .and_then(Value::as_f64)
        .filter(|n| n.is_finite())
    else {
        return Err(ProbeError::new("invalid_or_expired_token"));
    };
    let now_ms = (time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000) as f64;
    if exp * 1000.0 < now_ms + 60_000.0 {
        return Err(ProbeError::new("invalid_or_expired_token"));
    }
    Ok(Claims {
        sub: sub.to_owned(),
    })
}

fn bind_identity(
    claims: &Claims,
    me: &Map<String, Value>,
    expected_email_hash: &str,
) -> Result<String, ProbeError> {
    let valid_hash = expected_email_hash.len() == 64
        && expected_email_hash
            .bytes()
            .all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'));
    let auth_id = me.get("authId").and_then(Value::as_str);
    let email = me
        .get("email")
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty());
    if !valid_hash || auth_id != Some(claims.sub.as_str()) || email.is_none() {
        return Err(ProbeError::new("account_identity_mismatch"));
    }
    let normalized = email.unwrap().trim().to_lowercase();
    let actual_hex = format!("{:x}", Sha256::digest(normalized.as_bytes()));
    if actual_hex
        .as_bytes()
        .ct_eq(expected_email_hash.as_bytes())
        .unwrap_u8()
        == 0
    {
        return Err(ProbeError::new("account_identity_mismatch"));
    }
    let parts: Vec<&str> = claims.sub.split('|').collect();
    let user_id = match parts.as_slice() {
        [only] => *only,
        [_, second] => *second,
        _ => "",
    };
    if user_id.is_empty()
        || !user_id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b'-'))
    {
        return Err(ProbeError::new("unsupported_account_subject"));
    }
    Ok(user_id.to_owned())
}

pub(crate) fn encode_workos_cookie(user_id: &str, token: &str) -> String {
    let raw = format!("{user_id}::{token}");
    let mut encoded = String::with_capacity(raw.len() + 16);
    for &byte in raw.as_bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            encoded.push(byte as char);
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    format!("WorkosCursorSessionToken={encoded}")
}

pub(crate) async fn verify_free_account(
    token: &str,
    email_hash: &str,
    client: &CheckedClient<'_>,
) -> Result<(), ProbeError> {
    let user_id = verify_identity(token, email_hash, client).await?;
    let cookie = encode_workos_cookie(&user_id, token);
    let usage_resp = client.get_cookie_json(ENDPOINTS.usage, &cookie).await?;
    let usage = parse_json(
        usage_resp.status,
        &usage_resp.content_type,
        &usage_resp.body,
    )?;

    let membership_ok = usage
        .get("membershipType")
        .and_then(Value::as_str)
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("free"));
    let limit_ok = usage
        .get("limitType")
        .and_then(Value::as_str)
        .is_some_and(|s| matches!(s, "user" | "individual"));
    let individual = usage.get("individualUsage").and_then(Value::as_object);
    let on_demand = individual
        .and_then(|i| i.get("onDemand"))
        .and_then(Value::as_object);
    let on_demand_disabled = on_demand
        .and_then(|d| d.get("enabled"))
        .and_then(Value::as_bool)
        == Some(false);
    let on_demand_zero = on_demand
        .and_then(|d| d.get("used"))
        .and_then(Value::as_f64)
        .is_some_and(|u| u.is_finite() && u == 0.0);
    let percent_ok = individual
        .and_then(|i| i.get("plan"))
        .and_then(Value::as_object)
        .and_then(|p| p.get("totalPercentUsed"))
        .and_then(Value::as_f64)
        .is_some_and(|p| p.is_finite() && (0.0..100.0).contains(&p));

    let (grants_res, stripe_res, teams_res, limit_res) = tokio::join!(
        read_credit_grants(token, client),
        read_stripe(&cookie, client),
        read_teams(token, client),
        read_hard_limit(token, client),
    );
    for code in [
        grants_res.as_ref().err().map(|e| e.code),
        stripe_res.as_ref().err().map(|e| e.code),
        teams_res.as_ref().err().map(|e| e.code),
        limit_res.as_ref().err().map(|e| e.code),
    ]
    .into_iter()
    .flatten()
    {
        if code == "cancelled" {
            return Err(ProbeError::new("cancelled"));
        }
    }
    let (Ok(grants_ok), Ok(stripe_ok), Ok(teams_ok), Ok(limit_ok_personal)) =
        (grants_res, stripe_res, teams_res, limit_res)
    else {
        return Err(ProbeError::new("free_conditions_not_met"));
    };
    if !(membership_ok
        && limit_ok
        && on_demand_disabled
        && on_demand_zero
        && percent_ok
        && grants_ok
        && stripe_ok
        && teams_ok
        && limit_ok_personal)
    {
        return Err(ProbeError::new("free_conditions_not_met"));
    }
    Ok(())
}

async fn read_credit_grants(token: &str, client: &CheckedClient<'_>) -> Result<bool, ProbeError> {
    let resp = client
        .post_connect_json(
            ENDPOINTS.grants,
            token,
            r#"{"includeAllProducts":true,"includePersonalTeam":true}"#,
        )
        .await?;
    let map = parse_json(resp.status, &resp.content_type, &resp.body)?;
    let fields = wire_fields(
        &map,
        &[
            "hasCreditGrants",
            "creditBalanceCents",
            "totalCents",
            "usedCents",
        ],
    )?;
    let has_grants = scalar_bool(&fields, "hasCreditGrants", false)?;
    let balance = scalar_i64_safe(&fields, "creditBalanceCents", 0)?;
    let total = scalar_i64_safe(&fields, "totalCents", 0)?;
    let used = scalar_i64_safe(&fields, "usedCents", 0)?;
    Ok(!has_grants && balance == 0 && total == 0 && used == 0)
}

async fn read_stripe(cookie: &str, client: &CheckedClient<'_>) -> Result<bool, ProbeError> {
    let resp = client.get_cookie_json(ENDPOINTS.stripe, cookie).await?;
    let stripe = parse_json(resp.status, &resp.content_type, &resp.body)?;
    let free = stripe
        .get("membershipType")
        .and_then(Value::as_str)
        .is_some_and(|s| s.trim().eq_ignore_ascii_case("free"));
    let balance = match stripe.get("customerBalance") {
        Some(Value::Number(num)) => num
            .as_i64()
            .filter(|&n| (MIN_SAFE_INT..=MAX_SAFE_INT).contains(&n)),
        Some(Value::String(s)) if is_canonical_int_string(s, 15) => s
            .parse::<i64>()
            .ok()
            .filter(|&n| (MIN_SAFE_INT..=MAX_SAFE_INT).contains(&n)),
        _ => None,
    };
    Ok(free && balance == Some(0))
}

async fn read_teams(token: &str, client: &CheckedClient<'_>) -> Result<bool, ProbeError> {
    let resp = client
        .post_connect_json(ENDPOINTS.teams, token, r#"{"activeOnly":false}"#)
        .await?;
    let map = parse_json(resp.status, &resp.content_type, &resp.body)?;
    let fields = wire_fields(&map, &["teams"])?;
    let count = match fields.get("teams") {
        None => 0,
        Some(Value::Array(items)) if items.iter().all(Value::is_object) => items.len(),
        _ => return Err(ProbeError::new("invalid_team_list")),
    };
    Ok(count == 0)
}

async fn read_hard_limit(token: &str, client: &CheckedClient<'_>) -> Result<bool, ProbeError> {
    let resp = client
        .post_connect_json(ENDPOINTS.hard_limit, token, r#"{"personalScope":true}"#)
        .await?;
    let map = parse_json(resp.status, &resp.content_type, &resp.body)?;
    let fields = wire_fields(
        &map,
        &[
            "hardLimit",
            "noUsageBasedAllowed",
            "hardLimitPerUser",
            "perUserMonthlyLimitDollars",
            "isDynamicTeamLimit",
            "hasPendingSetupOnboardingCreditClaim",
            "isSetupPromoActive",
            "setupPromoAmountCents",
            "setupPromoCreditValidityDays",
            "onDemandSpendDisabledByOrganization",
            "perUserFirstPartyModelsAdditionalBudgetDollars",
            "perUserFirstPartyModelsAdditionalBudgetUnlimited",
        ],
    )?;
    let no_usage_allowed = scalar_bool(&fields, "noUsageBasedAllowed", false)?;
    let _org_disabled = scalar_bool(&fields, "onDemandSpendDisabledByOrganization", false)?;
    let dynamic_team = scalar_bool(&fields, "isDynamicTeamLimit", false)?;
    let pending_claim = scalar_bool(&fields, "hasPendingSetupOnboardingCreditClaim", false)?;
    let promo_active = scalar_bool(&fields, "isSetupPromoActive", false)?;
    let monthly = scalar_i32(&fields, "perUserMonthlyLimitDollars", 0)?;
    let hard_limit = scalar_i32(&fields, "hardLimit", 0)?;
    let per_user = scalar_i32_opt(&fields, "hardLimitPerUser")?;
    let promo_cents = scalar_i32_opt(&fields, "setupPromoAmountCents")?;
    let promo_days = scalar_i32_opt(&fields, "setupPromoCreditValidityDays")?;
    let extra_budget = scalar_i32_opt(&fields, "perUserFirstPartyModelsAdditionalBudgetDollars")?;
    let extra_unlimited =
        scalar_bool_opt(&fields, "perUserFirstPartyModelsAdditionalBudgetUnlimited")?;

    let alternate = dynamic_team
        || pending_claim
        || promo_active
        || monthly != 0
        || hard_limit != 0
        || [per_user, promo_cents, promo_days, extra_budget]
            .into_iter()
            .any(|v| v.is_some_and(|n| n != 0))
        || extra_unlimited == Some(true);
    Ok(no_usage_allowed && !alternate)
}
