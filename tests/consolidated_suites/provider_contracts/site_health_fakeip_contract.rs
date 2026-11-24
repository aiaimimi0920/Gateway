//! Opt-in public health proof in the actual Fake-IP/proxy environment; never invokes models.
use neuro_gateway::site_health::{discovery, ProbePolicy};
use serde_json::json;

#[tokio::test]
#[ignore = "Explicit public status fetch using the workstation network; no account credentials"]
async fn configured_official_transport_survives_fake_ip_dns() {
    let adapters = [
        "nvidia-status",
        "openrouter-status",
        "you-status",
        "exa-status",
        "openai-status",
        "jina-status",
        "tavily-status",
        "linkup-status",
    ];
    let mut failures = Vec::new();
    for adapter in adapters {
        let policy: ProbePolicy = serde_json::from_value(json!({"adapter": adapter})).unwrap();
        match discovery::discover("fixture", "https://provider.example", &policy).await {
            Ok(snapshot) => {
                println!("{adapter}: {}", serde_json::to_string(&snapshot).unwrap());
                // This regression proves HTTP reachability, not upstream health. A 403 challenge
                // remains unknown; packaged business acceptance checks supported observations separately.
                if snapshot.probes.is_empty()
                    || snapshot
                        .probes
                        .iter()
                        .any(|probe| probe.http_status.is_none())
                {
                    failures.push(format!("{adapter}: did not reach an HTTP response"));
                }
                assert!(
                    snapshot.models.is_empty(),
                    "must not fabricate model percentages"
                );
            }
            Err(error) => failures.push(format!("{adapter}: {error}")),
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
