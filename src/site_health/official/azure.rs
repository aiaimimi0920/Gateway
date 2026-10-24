//! Read only the published Azure OpenAI region matrix; never infer a subscription/deployment's health.
use super::Observation;
use regex::Regex;
use std::{collections::HashSet, sync::LazyLock};

static TABLE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<table\b([^>]*)>(.*?)</table\s*>").unwrap());
static HEAD: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<thead\b[^>]*>(.*?)</thead\s*>").unwrap());
static HEADER: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<th\b[^>]*>(.*?)</th\s*>").unwrap());
static ROW: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<tr\b([^>]*)>(.*?)</tr\s*>").unwrap());
static CELL: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?is)<td\b([^>]*)>(.*?)</td\s*>").unwrap());
static TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?s)<[^>]*>").unwrap());
static NON_DATA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?is)<script\b[^>]*>.*?</script\s*>|<svg\b[^>]*>.*?</svg\s*>|<!--.*?-->").unwrap()
});
static ATTRIBUTE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"([a-zA-Z][a-zA-Z0-9-]*)\s*=\s*["']([^"']*)["']"#).unwrap());

pub(super) async fn fetch(
    client: &rquest::Client,
    endpoint: &str,
) -> Result<Option<Vec<Observation>>, (String, Option<u16>)> {
    let bytes = crate::site_health::transport::regional_html(client.get(endpoint)).await?;
    tokio::task::spawn_blocking(move || parse(&bytes))
        .await
        .map_err(|_| ("invalid_response".into(), Some(200)))
}

pub(super) fn parse(bytes: &[u8]) -> Option<Vec<Observation>> {
    if bytes.len() > 8_388_608 {
        return None;
    }
    let source = std::str::from_utf8(bytes).ok()?;
    // Ignore script/comment markup and SVG labels before reading bounded table cells. No JS execution.
    let html = NON_DATA.replace_all(source, "");
    let mut out = Vec::new();
    let mut names = HashSet::new();
    for (index, table) in TABLE.captures_iter(&html).enumerate() {
        if index >= 32 {
            return None;
        }
        if !has_class(&table[1], "region-status-table") {
            continue;
        }
        let zone = attribute(&table[1], "data-zone-name")?;
        if zone.is_empty() || zone.len() > 64 {
            return None;
        }
        let mut matches = 0;
        for (row_index, row) in ROW.captures_iter(&table[2]).enumerate() {
            if row_index >= 2048 {
                return None;
            }
            if !has_class(&row[1], "capability-row") {
                continue;
            }
            let mut cells = CELL.captures_iter(&row[2]);
            let first = cells.next()?;
            if text(&first[2])? != "Azure OpenAI Service" {
                continue;
            }
            matches += 1;
            if matches != 1 {
                return None;
            }
            let head = HEAD.captures(&table[2])?;
            if merged_cells(&head[1]) || merged_cells(&row[2]) {
                return None;
            }
            let regions: Vec<String> = HEADER
                .captures_iter(&head[1])
                .take(129)
                .map(|h| text(&h[1]))
                .collect::<Option<_>>()?;
            if regions.len() < 2 || regions.len() > 128 || regions[0] != "Products and services" {
                return None;
            }
            let mut count = 0;
            for cell in cells {
                count += 1;
                if !has_class(&cell[1], "status-cell") || count >= regions.len() {
                    return None;
                }
                let name = format!("Azure OpenAI / {zone} / {}", regions[count]);
                if !names.insert(name.clone()) || out.len() >= 512 {
                    return None;
                }
                let labels: Vec<_> = ATTRIBUTE
                    .captures_iter(&cell[2])
                    .filter(|a| &a[1] == "data-label")
                    .take(2)
                    .map(|a| a[2].to_owned())
                    .collect();
                if labels.len() != 1 {
                    return None;
                }
                let status = match labels[0].as_str() {
                    "Good" => "operational",
                    "Information" => "information",
                    "Warning" => "warning",
                    "Critical" => "major_outage",
                    "Not available" | "N/A" => "not_available",
                    _ => "unknown",
                };
                out.push(Observation {
                    name,
                    status: status.into(),
                    model: None,
                });
            }
            if count + 1 != regions.len() {
                return None;
            }
        }
    }
    Some(out)
}

fn attribute(source: &str, name: &str) -> Option<String> {
    ATTRIBUTE
        .captures_iter(source)
        .find(|a| &a[1] == name)
        .map(|a| a[2].to_owned())
}

fn has_class(source: &str, name: &str) -> bool {
    attribute(source, "class").is_some_and(|v| v.split_ascii_whitespace().any(|c| c == name))
}

fn text(html: &str) -> Option<String> {
    if html.len() > 8192 {
        return None;
    }
    let plain = TAG.replace_all(html, " ");
    let mut value = String::new();
    for word in plain.split_whitespace() {
        let separator = usize::from(!value.is_empty());
        if value.len() + separator + word.len() > 128 {
            return None;
        }
        if separator != 0 {
            value.push(' ');
        }
        value.push_str(word);
    }
    (!value.is_empty()).then_some(value)
}

fn merged_cells(html: &str) -> bool {
    ATTRIBUTE
        .captures_iter(html)
        .any(|a| matches!(&a[1], "colspan" | "rowspan") && &a[2] != "1")
}
