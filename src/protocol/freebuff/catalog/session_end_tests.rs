//! End status, final refund receipt and grace are distinct; no inferred refund or authority.
use super::*;
use serde_json::json;

fn claim() -> SessionClaim {
    let attempt = uuid::Uuid::new_v4().to_string();
    SessionClaim {
        scope: [0; 32],
        instance: format!("cli:{attempt}"),
        attempt,
    }
}
fn parse(
    body: serde_json::Value,
    claim: &SessionClaim,
) -> Result<SessionEndReceipt, BusinessError> {
    SessionEndReceipt::parse(BusinessReply { status: 200, body }, claim)
}

#[test]
fn freebuff_end_receipt_requires_explicit_final_amount_and_preserves_zero_and_sources() {
    let claim = claim();
    for amount in [0.0, 4.25] {
        let receipt = parse(
            json!({"status":"ended", "freebucksRefund":amount,
            "freebucksRefundSources":{"daily":amount, "wallet":0}}),
            &claim,
        )
        .unwrap();
        assert!(receipt.is_final());
        assert_eq!(receipt.refund, Some(amount));
        assert_eq!(receipt.refund_sources.unwrap().daily, amount);
    }
    for body in [
        json!({"status":"ended"}),
        json!({"status":"ended", "freebucksRefund":4, "freebucksRefundPending":true}),
        json!({"status":"ended", "freebucksRefund":0, "instanceId":claim.instance_id()}),
    ] {
        assert!(!parse(body, &claim).unwrap().is_final());
    }
}

#[test]
fn freebuff_end_receipt_rejects_wrong_status_scope_and_malformed_money() {
    let claim = claim();
    for body in [
        json!(null),
        json!([]),
        json!({"status":"active", "freebucksRefund":0}),
        json!({"status":"none", "freebucksRefund":0}),
        json!({"status":"ended", "instanceId":"foreign", "freebucksRefund":0}),
        json!({"status":"ended", "instanceId":null, "freebucksRefund":0}),
        json!({"status":"ended", "freebucksRefund":-1}),
        json!({"status":"ended", "freebucksRefund":"4"}),
        json!({"status":"ended", "freebucksRefund":null}),
        json!({"status":"ended", "freebucksRefund":0, "freebucksRefundPending":null}),
        json!({"status":"ended", "freebucksRefund":0, "freebucksRefundSources":{"daily":-1,"wallet":1}}),
        json!({"status":"ended", "freebucksRefund":0, "freebucksRefundSources":{"daily":0}}),
    ] {
        assert_eq!(
            parse(body, &claim).err(),
            Some(BusinessError::InvalidResponse)
        );
    }
    for status in [201, 204, 403, 404, 429, 500] {
        assert_eq!(
            SessionEndReceipt::parse(
                BusinessReply {
                    status,
                    body: json!({"status":"ended","freebucksRefund":0})
                },
                &claim
            )
            .err(),
            Some(BusinessError::InvalidResponse)
        );
    }
}
