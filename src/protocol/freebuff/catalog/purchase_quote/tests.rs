//! Pure pricing/clock boundaries plus capability expiry. No network or fabricated billing receipt.
use super::*;
use crate::protocol::freebuff::catalog::{tests, DeviceKey, DeviceRegistration, ModelCatalog};
use serde_json::{json, Value};
use std::sync::Arc;
fn at(value: &str) -> OffsetDateTime {
    OffsetDateTime::parse(value, &time::format_description::well_known::Rfc3339).unwrap()
}
fn body() -> Value {
    json!({"status":"none", "freebucks":{"balance":20,"daily":{"remaining":8},"wallet":{"balance":12},"prices":{"open":10}}})
}
fn reply(body: Value) -> BusinessReply {
    BusinessReply { status: 200, body }
}

#[test]
fn freebuff_purchase_unknown_unpriced_insufficient_or_foreign_session_is_not_free() {
    let now = at("2026-11-06T12:00:00Z");
    for (input, expected) in [
        (
            json!({"status":"none"}),
            "freebuff_purchase_price_unavailable",
        ),
        (
            json!({"status":"none","freebucks":null}),
            "freebuff_purchase_price_unavailable",
        ),
        (
            json!({"status":"ended","instanceId":"foreign","freebucks":body()["freebucks"]}),
            "freebuff_purchase_existing_session",
        ),
    ] {
        assert_eq!(
            view(&reply(input), "open", now).err().unwrap().to_string(),
            expected
        );
    }
    let mut input = body();
    input["freebucks"]["prices"] = json!({});
    assert!(matches!(
        view(&reply(input), "open", now),
        Err(PurchaseError::Unavailable)
    ));
    let mut input = body();
    input["freebucks"]["balance"] = json!(1);
    assert!(matches!(
        view(&reply(input), "open", now),
        Err(PurchaseError::InsufficientFunds)
    ));
    let mut input = body();
    input["freebucks"]["balance"] = json!(7);
    input["freebucks"]["soloDiscount"] = json!(3);
    let result = view(&reply(input.clone()), "open", now).unwrap();
    assert_eq!(result.meter.session_price, Some(10.0));
    assert_eq!(result.wallet_spend, 2.0, "not a discounted final debit");
    input["freebucks"]["soloDiscount"] = json!(0);
    input["freebucks"]["claimableGrantFreebucks"] = json!(3);
    assert!(
        view(&reply(input.clone()), "open", now)
            .unwrap()
            .claim_earned
    );
    input["freebucks"]["quotaExempt"] = json!(true);
    assert!(!view(&reply(input), "open", now).unwrap().claim_earned);
    let mut input = body();
    input["freebucks"]["prices"]["open"] = json!(0);
    assert_eq!(view(&reply(input), "open", now).unwrap().wallet_spend, 0.0);
}

#[test]
fn freebuff_purchase_preserves_displayed_price_across_dated_and_recurring_boundaries() {
    let mut input = body();
    input["freebucks"]["priceChanges"] =
        json!([{"at":"2026-11-06T12:00:01Z","modelId":"open","price":11}]);
    let latest = reply(input);
    let frozen = view(&latest, "open", at("2026-11-06T12:00:00Z")).unwrap();
    let current = view(&latest, "open", at("2026-11-06T12:00:01Z")).unwrap();
    assert!(frozen != current);
    assert_eq!(frozen.meter.session_price, Some(10.0));
    let mut input = body();
    input["freebucks"]["offPeak"] =
        json!({"open":{"startHourUtc":22,"endHourUtc":6,"price":2,"regularPrice":10}});
    let latest = reply(input);
    let frozen = view(&latest, "open", at("2026-11-05T21:59:59Z")).unwrap();
    let current = view(&latest, "open", at("2026-11-05T22:00:00Z")).unwrap();
    assert!(frozen != current);
    assert_eq!(frozen.meter.session_price, Some(10.0));
}

#[test]
fn freebuff_purchase_expiry_and_registration_identity_are_not_refreshed_by_revalidation() {
    let mut session = super::super::CatalogSession::new(
        super::super::BusinessOrigin::Codebuff,
        "synthetic",
        ClientSurface::Cli,
    )
    .unwrap();
    session.catalog =
        Some(ModelCatalog::parse(&serde_json::to_vec(&tests::body()).unwrap()).unwrap());
    let (private, public) = tests::pair();
    let device = DeviceRegistration::new(
        &session.binding,
        Arc::new(DeviceKey::from_pkcs8(&private, &public).unwrap()),
        "device".into(),
    );
    let requests = session.business_requests(&device, "open").unwrap();
    let context = PurchaseContext::new("pool", "account", "revision").unwrap();
    let latest = reply(body());
    let frozen = view(&latest, "open", OffsetDateTime::now_utc()).unwrap();
    let mut checked = RevalidatedPurchase {
        confirmation: ConfirmedPurchase(PurchaseQuote {
            context: context.clone(),
            scope: requests.binding.scope,
            device: fingerprint(&requests),
            issued: Instant::now() - LIFETIME,
            view: frozen,
        }),
        latest,
    };
    assert!(matches!(
        checked.check(&context, &requests),
        Err(PurchaseError::Expired)
    ));
    assert!(matches!(
        checked.confirmation.0.pending_lifetime(&context, "open"),
        Err(PurchaseError::Expired)
    ));
    checked.confirmation.0.issued = Instant::now();
    assert!(
        checked
            .confirmation
            .0
            .pending_lifetime(&context, "open")
            .unwrap()
            <= LIFETIME
    );
    assert!(checked
        .confirmation
        .0
        .pending_lifetime(&context, "different-model")
        .is_err());
    assert!(checked.check(&context, &requests).is_ok());
    let replacement = DeviceRegistration::new(
        &session.binding,
        device.key.clone(),
        "different-registration".into(),
    );
    let other = session.business_requests(&replacement, "open").unwrap();
    assert!(matches!(
        checked.check(&context, &other),
        Err(PurchaseError::Mismatch)
    ));
    let mut alternate = super::super::CatalogSession::new(
        super::super::BusinessOrigin::Codebuff,
        "synthetic",
        ClientSurface::Cli,
    )
    .unwrap();
    let mut catalog = tests::body();
    catalog["rows"][0]["key"] = json!("different-model");
    alternate.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
    let other = alternate
        .business_requests(&device, "different-model")
        .unwrap();
    assert!(matches!(
        checked.check(&context, &other),
        Err(PurchaseError::Mismatch)
    ));
    checked.confirmation.0.issued = Instant::now() - LIFETIME;
    assert!(
        matches!(
            checked.check(&context, &requests),
            Err(PurchaseError::Expired)
        ),
        "fresh checks do not renew the original TTL"
    );
}
