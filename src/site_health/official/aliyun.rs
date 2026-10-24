//! China-portal public announcements: no confirmed Model Studio product identity or regional coverage.
use super::Observation;
use serde_json::Value;

pub(super) fn parse(body: &Value) -> Option<Vec<Observation>> {
    // HTTP 200 may contain a business-level denial; never treat it as an empty incident list.
    if body["success"] != true || body["code"] != 200 {
        return None;
    }
    let rows = body["data"].as_array().filter(|v| v.len() <= 512)?;
    let total = body["total"].as_u64()?;
    if total < rows.len() as u64 {
        return None;
    }
    let mut out = vec![Observation {
        name: format!("Alibaba Cloud public notices: {total}; Model Studio coverage unconfirmed"),
        status: if rows.is_empty() && total == 0 {
            "no_public_incidents"
        } else {
            "coverage_unknown"
        }
        .into(),
        model: None,
    }];
    // The official current-events view only guarantees a title. Do not assume history/detail fields.
    for (index, row) in rows.iter().enumerate() {
        let title = row["title"]
            .as_str()
            .filter(|v| !v.trim().is_empty() && v.len() <= 512)?;
        if index < 32 {
            out.push(Observation {
                name: title.into(),
                status: "public_notice".into(),
                model: None,
            });
        }
    }
    Some(out)
}
