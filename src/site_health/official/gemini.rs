//! Public AI Studio status RPC. Bootstrap key is page-owned, never a user credential.
use crate::site_health::transport;
use serde_json::Value;

pub(super) async fn fetch(client: &rquest::Client) -> Result<Value, (String, Option<u16>)> {
    let html = transport::bounded_body(client.get("https://aistudio.google.com/status")).await?;
    let html = std::str::from_utf8(&html).map_err(|_| ("invalid_response".into(), Some(200)))?;
    let key = bootstrap_key(html).ok_or_else(|| ("unsupported_status_schema".into(), Some(200)))?;
    // Never trust a bootstrap-provided host: this second origin is an explicit fixed allowlist.
    let origin = transport::checked_url("https://alkalimakersuite-pa.clients6.google.com/")
        .map_err(|_| ("network_policy_error".into(), None))?;
    let client = transport::client(&origin, false)
        .await
        .map_err(|_| ("network_policy_error".into(), None))?;
    let endpoint = origin.join("$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/ListIncidentsHistory").unwrap();
    let mut header = rquest::header::HeaderValue::from_str(&key)
        .map_err(|_| ("invalid_response".into(), Some(200)))?;
    header.set_sensitive(true);
    let bytes = transport::bounded_body(
        client
            .post(endpoint.as_str())
            .header("Content-Type", "application/json+protobuf")
            .header("Referer", "https://aistudio.google.com/")
            .header("X-Goog-Api-Key", header)
            .body("[]"),
    )
    .await?;
    serde_json::from_slice(&bytes).map_err(|_| ("invalid_json".into(), Some(200)))
}

fn bootstrap_key(html: &str) -> Option<String> {
    for part in html.split("<script").skip(1).take(64) {
        let (tag, contents) = part.split_once('>')?;
        if !tag.contains("type=\"application/json\"") {
            continue;
        }
        let (json, _) = contents.split_once("</script>")?;
        let Ok(value) = serde_json::from_str::<Value>(json) else {
            continue;
        };
        if let Some(key) = value["WIu0Nc"].as_str().filter(|k| {
            k.len() <= 128
                && k.starts_with("AIza")
                && k.bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_-".contains(&c))
        }) {
            return Some(key.into());
        }
    }
    None
}

pub(super) fn parse(body: &Value) -> Option<Vec<super::Observation>> {
    let incidents = body.get(0)?.get(0)?.as_array().filter(|v| v.len() <= 512)?;
    let mut status = "operational";
    for incident in incidents {
        let components = incident.get(5)?.as_array().filter(|v| v.len() <= 32)?;
        // API track 1 only: an AI Studio UI incident must not imply API downtime.
        if !components.iter().any(|v| v.as_u64() == Some(1)) {
            continue;
        }
        let updates = incident
            .get(3)?
            .as_array()
            .filter(|v| !v.is_empty() && v.len() <= 512)?;
        let mut latest = None;
        for update in updates {
            let timestamp = update.get(2)?.get(0)?.as_str()?.parse::<u64>().ok()?;
            let kind = update.get(0)?.as_u64()?;
            if latest.is_none_or(|(t, _)| timestamp >= t) {
                latest = Some((timestamp, kind));
            }
        }
        match latest?.1 {
            4 => (),
            1 | 2 | 5 => status = "degraded_performance",
            _ if status == "operational" => status = "unknown",
            _ => (),
        }
    }
    Some(vec![super::Observation {
        name: "Gemini API".into(),
        status: status.into(),
        model: None,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bootstrap_is_data_not_script_execution_or_redirect() {
        assert_eq!(bootstrap_key(r#"<script type="application/json">{"WIu0Nc":"AIza_public_fixture","CoJqbf":"https://evil.invalid"}</script>"#).as_deref(), Some("AIza_public_fixture"));
        assert!(bootstrap_key(r#"<script>window.WIu0Nc="AIza_script"</script>"#).is_none());
        assert!(bootstrap_key(
            r#"<script type="application/json">{"WIu0Nc":"AIza\r\nheader"}</script>"#
        )
        .is_none());
    }
}
