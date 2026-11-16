//! Preserve declared Command Code routes; unknown routes never imply executable capability.
use super::ModelCatalog;
use crate::error::GatewayError;

const ROUTES: [&str; 4] = ["/chat/completions", "/responses", "/messages", "/systemone"];

pub(super) fn read(item: &serde_json::Value) -> Result<Vec<String>, GatewayError> {
    let routes = item["supported_endpoints"]
        .as_array()
        .ok_or_else(super::unavailable)?;
    let mut result = Vec::new();
    for route in routes {
        let route = route.as_str().ok_or_else(super::unavailable)?;
        if ROUTES.contains(&route) && !result.iter().any(|existing| existing == route) {
            result.push(route.to_owned());
        }
    }
    Ok(result)
}

pub(super) fn validate(catalog: &ModelCatalog, preset: Option<&str>) -> anyhow::Result<()> {
    // Missing metadata remains valid for previously saved catalogs and other products.
    if catalog.supported_endpoints.is_empty() {
        return Ok(());
    }
    anyhow::ensure!(
        preset == Some(crate::protocol::commandcode::PRESET)
            && catalog.source_url == crate::protocol::commandcode::SOURCE
            && catalog.supported_endpoints.len() == catalog.models.len(),
        "Endpoint declarations must match the Command Code catalog."
    );
    for model in &catalog.models {
        let routes = catalog.supported_endpoints.get(model);
        anyhow::ensure!(
            routes.is_some_and(|routes| {
                !routes.is_empty()
                    && routes.len() <= ROUTES.len()
                    && routes.iter().all(|route| ROUTES.contains(&route.as_str()))
                    && routes
                        .iter()
                        .collect::<std::collections::HashSet<_>>()
                        .len()
                        == routes.len()
            }),
            "Invalid model endpoint declarations."
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn commandcode_endpoint_metadata_is_optional_bound_and_roundtrips() {
        let old = json!({"sourceUrl":crate::protocol::commandcode::SOURCE,
            "checkedAt":"2026-11-13T00:00:00Z","models":["model"]});
        let mut catalog: ModelCatalog = serde_json::from_value(old.clone()).unwrap();
        catalog
            .validate(
                Some(crate::protocol::commandcode::PRESET),
                crate::protocol::commandcode::BASE,
            )
            .unwrap();
        assert_eq!(serde_json::to_value(&catalog).unwrap(), old);
        catalog.supported_endpoints.insert(
            "model".into(),
            vec!["/chat/completions".into(), "/responses".into()],
        );
        validate(&catalog, Some(crate::protocol::commandcode::PRESET)).unwrap();
        assert!(validate(&catalog, Some("openai")).is_err());
        let roundtrip: ModelCatalog =
            serde_json::from_value(serde_json::to_value(&catalog).unwrap()).unwrap();
        assert_eq!(roundtrip.supported_endpoints, catalog.supported_endpoints);
        for routes in [vec![], vec!["/future"], vec!["/responses", "/responses"]] {
            catalog.supported_endpoints.insert(
                "model".into(),
                routes.into_iter().map(str::to_owned).collect(),
            );
            assert!(validate(&catalog, Some(crate::protocol::commandcode::PRESET)).is_err());
        }
        catalog.supported_endpoints.clear();
        catalog
            .supported_endpoints
            .insert("other".into(), vec!["/responses".into()]);
        assert!(validate(&catalog, Some(crate::protocol::commandcode::PRESET)).is_err());
    }

    #[test]
    fn commandcode_endpoint_parser_preserves_known_routes_without_guessing_unknowns() {
        assert_eq!(
            read(&json!({"supported_endpoints":["/responses","/future","/responses","/messages"]}))
                .unwrap(),
            ["/responses", "/messages"]
        );
        assert!(read(&json!({"supported_endpoints":["/chat/completions",42]})).is_err());
    }
}
