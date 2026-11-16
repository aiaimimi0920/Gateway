//! Raw CLI approval receipts are not legacy API payloads. Only management login may save this new context.
use crate::error::GatewayError;
use serde_json::Value;

pub(super) fn reject_unhandled_receipt(surface: &str, raw: &Value) -> Result<(), GatewayError> {
    let typed = raw.get("freebuff_session").is_some();
    let raw_receipt = surface == "freebuff-compatible"
        && [
            Some(raw),
            raw.get("user"),
            raw.get("extraBody"),
            raw.get("extra_body"),
            raw.get("headers"),
        ]
        .into_iter()
        .flatten()
        .any(|value| {
            [
                "fingerprintId",
                "fingerprintHash",
                "fingerprint_id",
                "fingerprint_hash",
            ]
            .iter()
            .any(|name| value.get(name).is_some())
        });
    if typed || raw_receipt {
        return Err(GatewayError::bad_request("FreeBuff CLI receipts require management login; folder receipt import is not supported")
            .with_code("freebuff_receipt_import_not_supported"));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn freebuff_raw_cli_receipts_cannot_passthrough_or_cross_import_into_another_product() {
        for raw in [
            json!({"fingerprintId":"private-device","authToken":"private-token"}),
            json!({"user":{"fingerprintHash":"private-hash","authToken":"private-token"}}),
            json!({"extra_body":{"fingerprint_id":"private-device"}}),
            json!({"freebuff_session":{"fingerprint_id":"private-device","fingerprint_hash":"private-hash"}}),
        ] {
            let error = reject_unhandled_receipt("freebuff-compatible", &raw).unwrap_err();
            assert_eq!(
                error.code.as_deref(),
                Some("freebuff_receipt_import_not_supported")
            );
            assert!(!error.message.contains("private-"));
        }
        assert!(reject_unhandled_receipt("openai", &json!({"freebuff_session":{}})).is_err());
        assert!(reject_unhandled_receipt(
            "freebuff-compatible",
            &json!({"api_key":"legacy-token","extra_body":{"freebuffAgentId":"agent"}})
        )
        .is_ok());
    }
}
