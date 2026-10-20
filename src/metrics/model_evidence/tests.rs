use super::*;
use serde_json::json;

#[test]
fn ring_evicts_oldest_without_creating_per_model_series() {
    let metrics = GatewayMetrics::new();
    for n in 0..150 {
        metrics.observe_model_declaration(
            ModelObservationMeta::new("provider", Some(&format!("alias-{n}")), "sent", false)
                .finish(None, "missing_or_unobserved", "buffered"),
        );
    }
    let rows = serde_json::to_value(metrics.model_observations()).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 100);
    assert_eq!(rows[0]["requestedModel"], "alias-50");
    assert_eq!(rows[99]["requestedModel"], "alias-149");
    assert!(!metrics.render_prometheus().contains("alias-149"));
}

#[test]
fn invalid_labels_are_unknown_not_truncated_or_echoed() {
    for value in [
        "",
        "private prompt",
        "line\nbreak",
        "sk-secret",
        "eyJcredential",
        &"a".repeat(257),
    ] {
        let declaration = ModelDeclaration {
            value: value.into(),
            source: ModelDeclarationSource::Model,
        };
        let row = ModelObservationMeta::new(value, Some(value), value, false).finish(
            Some(&declaration),
            "declared",
            "buffered",
        );
        let row = serde_json::to_value(row).unwrap();
        for key in [
            "providerId",
            "requestedModel",
            "upstreamModel",
            "declaredModel",
            "declarationSource",
        ] {
            assert_eq!(row[key], json!(null), "{key}: {value:?}");
        }
        assert_eq!(row["relation"], "unknown");
        assert_eq!(row["verified"], false);
    }
}

#[test]
fn aliasing_is_independent_of_unverified_provider_comparison() {
    let declaration = ModelDeclaration {
        value: "sent".into(),
        source: ModelDeclarationSource::Model,
    };
    let row = serde_json::to_value(
        ModelObservationMeta::new("p", Some("alias"), "sent", true).finish(
            Some(&declaration),
            "declared",
            "eof",
        ),
    )
    .unwrap();
    assert_eq!(row["aliasApplied"], true);
    assert_eq!(row["relation"], "declared_matches");
    assert_eq!(row["verified"], false);
}

#[test]
fn concurrent_writers_share_one_bounded_ring() {
    let metrics = std::sync::Arc::new(GatewayMetrics::new());
    let workers = (0..8)
        .map(|_| {
            let metrics = metrics.clone();
            std::thread::spawn(move || {
                for _ in 0..150 {
                    metrics.observe_model_declaration(
                        ModelObservationMeta::new("p", Some("alias"), "sent", true).finish(
                            None,
                            "missing_or_unobserved",
                            "eof",
                        ),
                    );
                }
            })
        })
        .collect::<Vec<_>>();
    for worker in workers {
        worker.join().unwrap();
    }
    assert_eq!(metrics.model_observations().len(), 100);
}
