//! Restricted DAV multistatus parsing. DTDs, entity definitions and deep trees fail closed.
use super::path::href_key;
use crate::credential_pool_storage::{MAX_LIST_ENTRIES, MAX_OBJECT_BYTES};
use anyhow::{anyhow, bail, ensure, Result};
use std::collections::{BTreeSet, HashSet};
use url::Url;
use xmlparser::{ElementEnd, Token, Tokenizer};

#[derive(Default)]
struct Entry {
    href: Option<String>,
    direct_status: Option<bool>,
    property_ok: bool,
    failed_status: Option<u16>,
    collection: bool,
}

struct Node<'a> {
    prefix: &'a str,
    local: &'a str,
    dav: bool,
    namespaces: Vec<(&'a str, String)>,
    attributes: HashSet<(&'a str, &'a str)>,
    text: Option<String>,
}

pub(super) fn list_keys(bytes: &[u8], namespace: &Url) -> Result<Vec<String>> {
    ensure!(
        bytes.len() <= MAX_OBJECT_BYTES,
        "WebDAV XML exceeds the size limit"
    );
    let source = std::str::from_utf8(bytes).map_err(|_| anyhow!("invalid WebDAV XML encoding"))?;
    let mut stack: Vec<Node<'_>> = Vec::new();
    let mut pending: Option<Node<'_>> = None;
    let mut entry = Entry::default();
    let mut keys = BTreeSet::new();
    let (mut roots, mut responses) = (0, 0);
    for token in Tokenizer::from(source) {
        match token.map_err(|_| anyhow!("invalid WebDAV XML"))? {
            Token::ElementStart { prefix, local, .. } => {
                ensure!(
                    pending.is_none() && stack.len() < 32,
                    "invalid WebDAV XML nesting"
                );
                ensure!(
                    stack.last().is_none_or(|node| node.text.is_none()),
                    "invalid WebDAV XML value"
                );
                pending = Some(Node {
                    prefix: prefix.as_str(),
                    local: local.as_str(),
                    dav: false,
                    namespaces: Vec::new(),
                    attributes: HashSet::new(),
                    text: None,
                });
            }
            Token::Attribute {
                prefix,
                local,
                value,
                ..
            } => {
                let node = pending
                    .as_mut()
                    .ok_or_else(|| anyhow!("invalid WebDAV XML attribute"))?;
                ensure!(
                    node.attributes.len() < 64
                        && node.attributes.insert((prefix.as_str(), local.as_str())),
                    "invalid WebDAV XML attributes"
                );
                let value = unescape(value.as_str())?;
                if prefix.as_str() == "xmlns" {
                    node.namespaces.push((local.as_str(), value));
                } else if prefix.is_empty() && local.as_str() == "xmlns" {
                    node.namespaces.push(("", value));
                }
            }
            Token::ElementEnd { end, .. } => {
                match end {
                    ElementEnd::Open | ElementEnd::Empty => {
                        let mut node = pending
                            .take()
                            .ok_or_else(|| anyhow!("invalid WebDAV XML element"))?;
                        let binding = node
                            .namespaces
                            .iter()
                            .rev()
                            .chain(
                                stack
                                    .iter()
                                    .rev()
                                    .flat_map(|parent| parent.namespaces.iter().rev()),
                            )
                            .find(|(prefix, _)| *prefix == node.prefix)
                            .map(|(_, uri)| uri.as_str());
                        ensure!(
                            node.prefix.is_empty() || binding.is_some(),
                            "invalid WebDAV XML namespace"
                        );
                        node.dav = binding == Some("DAV:");
                        if stack.is_empty() {
                            roots += 1;
                            ensure!(
                                roots == 1 && node.dav && node.local == "multistatus",
                                "invalid WebDAV multistatus"
                            );
                        }
                        stack.push(node);
                        if is_path(&stack, &["multistatus", "response"]) {
                            responses += 1;
                            ensure!(
                                responses <= MAX_LIST_ENTRIES + 1,
                                "WebDAV listing exceeds the entry limit"
                            );
                            entry = Entry::default();
                        }
                        if is_path(&stack, &["multistatus", "response", "href"])
                            || is_path(&stack, &["multistatus", "response", "status"])
                            || is_path(&stack, &["multistatus", "response", "propstat", "status"])
                        {
                            stack.last_mut().unwrap().text = Some(String::new());
                        }
                        if is_path(
                            &stack,
                            &[
                                "multistatus",
                                "response",
                                "propstat",
                                "prop",
                                "resourcetype",
                                "collection",
                            ],
                        ) {
                            entry.collection = true;
                        }
                    }
                    ElementEnd::Close(prefix, local) => {
                        ensure!(
                            pending.is_none()
                                && stack
                                    .last()
                                    .is_some_and(|node| node.prefix == prefix.as_str()
                                        && node.local == local.as_str()),
                            "mismatched WebDAV XML element"
                        );
                    }
                }
                if end != ElementEnd::Open {
                    finish_node(&mut stack, &mut entry, namespace, &mut keys)?;
                }
            }
            Token::Text { text } => append_text(&mut stack, &unescape(text.as_str())?)?,
            Token::Cdata { text, .. } => append_text(&mut stack, text.as_str())?,
            Token::Declaration { encoding, .. } => {
                ensure!(
                    encoding.is_none_or(|name| name.as_str().eq_ignore_ascii_case("UTF-8")),
                    "unsupported WebDAV XML encoding"
                );
            }
            Token::Comment { .. } => {}
            _ => bail!("unsupported WebDAV XML declaration"),
        }
    }
    ensure!(
        roots == 1 && stack.is_empty() && pending.is_none(),
        "incomplete WebDAV multistatus"
    );
    Ok(keys.into_iter().collect())
}

fn is_path(stack: &[Node<'_>], path: &[&str]) -> bool {
    stack.len() == path.len()
        && stack
            .iter()
            .zip(path)
            .all(|(node, name)| node.dav && node.local == *name)
}

fn append_text(stack: &mut [Node<'_>], text: &str) -> Result<()> {
    if let Some(value) = stack.last_mut().and_then(|node| node.text.as_mut()) {
        ensure!(
            value.len() + text.len() <= 8192,
            "WebDAV XML value exceeds the size limit"
        );
        value.push_str(text);
    } else if stack.is_empty() {
        ensure!(text.trim().is_empty(), "invalid WebDAV XML text");
    }
    Ok(())
}

fn finish_node(
    stack: &mut Vec<Node<'_>>,
    entry: &mut Entry,
    namespace: &Url,
    keys: &mut BTreeSet<String>,
) -> Result<()> {
    let response = is_path(stack, &["multistatus", "response"]);
    let depth = stack.len();
    let node = stack
        .pop()
        .ok_or_else(|| anyhow!("invalid WebDAV XML close"))?;
    if let Some(text) = node.text {
        if node.local == "href" {
            ensure!(entry.href.is_none(), "duplicate WebDAV resource href");
            entry.href = Some(text);
        } else {
            let mut parts = text.split_ascii_whitespace();
            let protocol = parts.next().unwrap_or("");
            let code = parts.next().and_then(|code| code.parse::<u16>().ok());
            ensure!(
                matches!(protocol, "HTTP/1.1" | "HTTP/1.0" | "HTTP/2") && code.is_some(),
                "invalid WebDAV resource status"
            );
            let ok = code.is_some_and(|code| (200..300).contains(&code));
            if !ok {
                entry.failed_status = code;
            }
            if depth == 3 {
                ensure!(
                    entry.direct_status.is_none(),
                    "duplicate WebDAV resource status"
                );
                entry.direct_status = Some(ok);
            } else {
                entry.property_ok |= ok;
            }
        }
    }
    if response {
        let href = entry.href.as_deref().unwrap_or("");
        let key = href_key(href, namespace);
        // Reuse the same strict href admission for the collection itself by testing
        // an immediate synthetic child. This performs no network or storage I/O.
        let collection_child = format!(
            "{}/__listing_scope__.json",
            href.strip_suffix('/').unwrap_or(href)
        );
        let requested_collection =
            href_key(&collection_child, namespace).as_deref() == Some("__listing_scope__.json");
        if key.is_some() || requested_collection {
            if let Some(status) = entry.failed_status {
                bail!("WebDAV listing resource returned HTTP {status}");
            }
            ensure!(
                entry.direct_status.unwrap_or(entry.property_ok),
                "WebDAV listing resource has no successful status"
            );
        }
        if let Some(key) = key.filter(|_| !entry.collection) {
            keys.insert(key);
            ensure!(
                keys.len() <= MAX_LIST_ENTRIES,
                "WebDAV listing exceeds the entry limit"
            );
        }
    }
    Ok(())
}

fn unescape(value: &str) -> Result<String> {
    let mut result = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(index) = rest.find('&') {
        result.push_str(&rest[..index]);
        rest = &rest[index + 1..];
        let end = rest
            .find(';')
            .ok_or_else(|| anyhow!("invalid WebDAV XML entity"))?;
        let entity = &rest[..end];
        let decoded = match entity {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            _ if entity.starts_with("#x") => u32::from_str_radix(&entity[2..], 16)
                .ok()
                .and_then(char::from_u32),
            _ if entity.starts_with('#') => {
                entity[1..].parse::<u32>().ok().and_then(char::from_u32)
            }
            _ => None,
        }
        .filter(|ch| matches!(*ch, '\t' | '\r' | '\n') || !ch.is_control());
        result.push(decoded.ok_or_else(|| anyhow!("unsupported WebDAV XML entity"))?);
        rest = &rest[end + 1..];
    }
    result.push_str(rest);
    Ok(result)
}

#[cfg(test)]
#[path = "webdav_xml_tests.rs"]
mod tests;
