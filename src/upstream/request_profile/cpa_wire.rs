//! HTTP envelope measured from the isolated CPA 20c9095 executable, not TLS emulation.
use super::*;
use crate::upstream::common::RequestPlan;
use rquest::header::{HeaderMap, HeaderName, OrigHeaderMap};

pub(crate) fn plan(
    payload: &ProviderAccountPayload,
    req: &CanonicalRelayRequest,
    model: &str,
    plan: &mut RequestPlan,
) {
    if !selected(payload).is_cpa()
        || !applies(payload, req)
        || !uses_claude(payload, req, model)
        || payload.canonical_adapter() != "anthropic_compatible"
    {
        return;
    }
    // Explicit endpoint query rules are operator-owned; never duplicate or override beta.
    let has_beta = plan.query.iter().any(|(key, _)| key == "beta")
        || url::Url::parse(&plan.url)
            .ok()
            .is_some_and(|url| url.query_pairs().any(|(key, _)| key == "beta"));
    if !has_beta {
        plan.query.push(("beta".into(), "true".into()));
    }
}

fn canonical_name(name: &str) -> String {
    let mut upper = true;
    name.bytes()
        .map(|byte| {
            let out = if upper {
                byte.to_ascii_uppercase()
            } else {
                byte.to_ascii_lowercase()
            };
            upper = byte == b'-';
            out as char
        })
        .collect()
}

pub(crate) fn order(headers: HeaderMap) -> (HeaderMap, OrigHeaderMap) {
    // Go Request.Write emits these first; the rest follows canonical-name order.
    let mut names = vec![
        "Host".to_owned(),
        "User-Agent".to_owned(),
        "Content-Length".to_owned(),
    ];
    let mut rest: Vec<_> = headers
        .keys()
        .map(|key| canonical_name(key.as_str()))
        .filter(|key| !names.contains(key))
        .collect();
    rest.sort_unstable();
    names.extend(rest);
    let mut ordered = HeaderMap::new();
    let mut original = OrigHeaderMap::new();
    for name in names {
        if let Ok(key) = HeaderName::from_bytes(name.as_bytes()) {
            // HeaderMap::remove returns only the first value; ordering must not drop repeats.
            for value in headers.get_all(&key) {
                ordered.append(key.clone(), value.clone());
            }
        }
        original.insert(name);
    }
    (ordered, original)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cpa_order_retains_repeated_values_and_secret_markers() {
        let mut headers = HeaderMap::new();
        for value in ["a=1", "b=2"] {
            let mut value = rquest::header::HeaderValue::from_str(value).unwrap();
            value.set_sensitive(true);
            headers.append("cookie", value);
        }
        headers.append("x-fixture", "first".parse().unwrap());
        headers.append("x-fixture", "second".parse().unwrap());
        let (ordered, _) = order(headers);
        assert_eq!(ordered.get_all("cookie").iter().count(), 2);
        assert!(ordered.get_all("cookie").iter().all(|v| v.is_sensitive()));
        assert_eq!(
            ordered
                .get_all("x-fixture")
                .iter()
                .map(|v| v.to_str().unwrap())
                .collect::<Vec<_>>(),
            vec!["first", "second"]
        );
    }
}
