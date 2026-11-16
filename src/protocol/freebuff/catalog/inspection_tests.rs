//! Synthetic display snapshots; no admission or financial authority is created.
use super::*;
use serde_json::{json, Value};
fn parse(body: Value, at: &str) -> Result<Value, BusinessError> {
    SessionInspection::parse(
        BusinessReply { status: 200, body },
        "open",
        time::OffsetDateTime::parse(at, &time::format_description::well_known::Rfc3339).unwrap(),
    )
    .map(|value| serde_json::to_value(value).unwrap())
}
const NOW: &str = "2026-11-06T12:00:00Z";
fn meter() -> Value {
    json!({"balance":7,"daily":{"remaining":5},"wallet":{"balance":2},"prices":{"open":10},
        "quotaExempt":false,"spendWithoutAsking":false,"soloDiscount":0,"claimableGrantFreebucks":0})
}
#[test]
fn freebuff_inspection_meter_preserves_absent_null_unpriced_zero_and_optional_flags() {
    assert_eq!(
        parse(json!({"status":"none"}), NOW).unwrap()["meter"],
        json!({"state":"absent"})
    );
    assert_eq!(
        parse(json!({"status":"none","freebucks":null}), NOW).unwrap()["meter"],
        json!({"state":"unavailable"})
    );
    let mut block = meter();
    block["prices"] = json!({});
    let result = parse(json!({"status":"none","freebucks":block}), NOW).unwrap();
    assert!(result["meter"]["sessionPrice"].is_null());
    assert_eq!(result["meter"]["quotaExempt"], false);
    assert_eq!(result["meter"]["soloDiscount"], 0.0);
    block["prices"]["open"] = json!(0);
    assert_eq!(
        parse(json!({"status":"none","freebucks":block}), NOW).unwrap()["meter"]["sessionPrice"],
        0.0
    );
}
#[test]
fn freebuff_inspection_projects_dated_changes_once_in_time_order_without_mutating_balance_or_discounting(
) {
    let mut block = meter();
    block["priceChanges"] = json!([
        {"at":"2026-11-06T11:00:00Z","modelId":"open","price":20},
        {"at":"2026-11-06T09:00:00Z","modelId":"open","price":15},
        {"at":"2026-11-06T13:00:00Z","modelId":"open","price":30},
        {"at":"2026-11-06T11:00:00Z","modelId":"open","price":21},
        {"at":"2026-11-06T11:00:00Z","modelId":"other","price":99}]);
    block["soloDiscount"] = json!(3);
    let result = parse(json!({"status":"none","freebucks":block.clone()}), NOW).unwrap();
    assert_eq!(result["meter"]["sessionPrice"], 21.0);
    assert_eq!(result["meter"]["balance"], 7.0);
    let later = parse(
        json!({"status":"none","freebucks":block.clone()}),
        "2026-11-06T14:00:00Z",
    )
    .unwrap();
    assert_eq!(later["meter"]["sessionPrice"], 30.0);
    assert_eq!(
        result["meter"]["sessionPrice"], 21.0,
        "prior snapshot is immutable"
    );
    block["prices"] = json!({});
    assert!(
        parse(json!({"status":"none","freebucks":block}), NOW).unwrap()["meter"]["sessionPrice"]
            .is_null()
    );
}
#[test]
fn freebuff_inspection_recurring_policy_handles_utc_overnight_boundaries_and_beijing_weekend() {
    let mut block = meter();
    block["offPeak"] = json!({"open":{"startHourUtc":22,"endHourUtc":6,"price":2,"regularPrice":10,"weekendsOffPeak":true}});
    for (at, expected) in [
        ("2026-11-05T21:59:59Z", 10),
        ("2026-11-05T22:00:00Z", 2),
        ("2026-11-06T05:59:59Z", 2),
        ("2026-11-06T06:00:00Z", 10),
        ("2026-11-06T16:00:00Z", 2),
        ("2026-11-08T16:00:00Z", 10),
        ("2026-11-06T06:00:00+08:00", 2),
    ] {
        assert_eq!(
            parse(json!({"status":"none","freebucks":block}), at).unwrap()["meter"]["sessionPrice"]
                .as_f64(),
            Some(expected as f64),
            "{at}"
        );
    }
    block["offPeak"]["open"]["startHourUtc"] = json!(8);
    block["offPeak"]["open"]["endHourUtc"] = json!(12);
    assert_eq!(
        parse(
            json!({"status":"none","freebucks":block}),
            "2026-11-05T10:00:00Z"
        )
        .unwrap()["meter"]["sessionPrice"],
        2.0
    );
}
#[test]
fn freebuff_inspection_invalid_meter_and_price_policies_do_not_fall_back_to_zero() {
    for (field, bad) in [
        ("balance", json!(-1)),
        ("balance", json!("7")),
        ("daily", json!({})),
        ("wallet", json!(null)),
        ("prices", json!({"open":-1})),
        ("prices", json!(null)),
        ("soloDiscount", json!(-1)),
        (
            "priceChanges",
            json!([{"at":"bad","modelId":"open","price":1}]),
        ),
        (
            "offPeak",
            json!({"open":{"startHourUtc":24,"endHourUtc":6,"price":2,"regularPrice":10}}),
        ),
    ] {
        let mut block = meter();
        block[field] = bad;
        assert_eq!(
            parse(json!({"status":"none","freebucks":block}), NOW).err(),
            Some(BusinessError::InvalidResponse),
            "{field}"
        );
    }
}
fn active() -> Value {
    json!({"status":"active","accessTier":"limited","instanceId":"foreign-instance-secret","model":"other",
        "admittedAt":"2026-11-06T11:00:00Z","expiresAt":"2026-11-06T13:00:00Z","remainingMs":3600000,
        "freebucks":meter(),"unknownSecret":"never-export"})
}
#[test]
fn freebuff_inspection_checks_full_active_but_never_exports_instance_or_claims_ownership() {
    let result = parse(active(), NOW).unwrap();
    assert_eq!(result["session"]["status"], "active");
    assert_eq!(result["session"]["modelKey"], "other");
    assert!(
        !result.to_string().contains("foreign-instance-secret")
            && !result.to_string().contains("never-export")
    );
    for field in [
        "accessTier",
        "instanceId",
        "model",
        "admittedAt",
        "expiresAt",
        "remainingMs",
    ] {
        let mut body = active();
        body.as_object_mut().unwrap().remove(field);
        assert!(parse(body, NOW).is_err(), "missing {field}");
    }
    for (field, bad) in [
        ("remainingMs", json!(-1)),
        ("expiresAt", json!("bad")),
        ("expiresAt", json!("2026-11-05T00:00:00Z")),
        ("model", json!("fbm1.secret")),
        ("instanceId", json!("bad\ninstance")),
    ] {
        let mut body = active();
        body[field] = bad;
        assert!(parse(body, NOW).is_err(), "{field}");
    }
}
#[test]
fn freebuff_inspection_ended_grace_is_not_new_prompt_permission_or_refund_receipt() {
    for (body, grace) in [
        (json!({"status":"ended"}), false),
        (
            json!({"status":"ended","instanceId":"foreign","freebucksRefundPending":true,"freebucksRefund":7}),
            true,
        ),
    ] {
        assert_eq!(
            parse(body, NOW).unwrap()["session"],
            json!({"status":"ended","inGrace":grace})
        );
    }
    assert!(parse(json!({"status":"ended","instanceId":null}), NOW).is_err());
    for status in ["queued", "error", "model_locked", "unknown"] {
        assert!(parse(json!({"status":status}), NOW).is_err());
    }
    assert!(SessionInspection::parse(
        BusinessReply {
            status: 401,
            body: active()
        },
        "open",
        time::OffsetDateTime::now_utc()
    )
    .is_err());
}
