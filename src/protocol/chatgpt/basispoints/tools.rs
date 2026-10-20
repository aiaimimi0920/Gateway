//! A bounded client-owned tool catalogue. Native Office tools never execute here.
use crate::error::GatewayError;
use serde_json::{json, Value};
use std::collections::HashSet;

#[derive(Clone)]
pub(super) struct Tool {
    pub key: String,
    pub name: String,
    pub namespace: Option<String>,
    pub custom: bool,
    pub definition: Value,
}

pub(super) fn invalid() -> GatewayError {
    GatewayError::bad_request("Unsupported or ambiguous Basispoints client tool contract")
        .with_code("basispoints_tool_contract")
}

pub(super) fn catalogue(source: &Value) -> Result<Vec<Tool>, GatewayError> {
    fn collect(
        items: &[Value],
        namespace: Option<&str>,
        out: &mut Vec<Tool>,
        depth: usize,
    ) -> Result<(), GatewayError> {
        if depth > 8 || items.len() > 256 {
            return Err(invalid());
        }
        for item in items {
            let kind = item["type"].as_str().unwrap_or("");
            if kind == "namespace" && namespace.is_none() {
                let name = item["name"]
                    .as_str()
                    .filter(|s| !s.is_empty() && s.len() <= 128)
                    .ok_or_else(invalid)?;
                collect(
                    item["tools"].as_array().ok_or_else(invalid)?,
                    Some(name),
                    out,
                    depth + 1,
                )?;
                continue;
            }
            if !matches!(kind, "function" | "custom") {
                return Err(invalid());
            }
            let fields = item.get(kind).unwrap_or(item);
            let name = fields["name"]
                .as_str()
                .filter(|s| !s.is_empty() && s.len() <= 128)
                .ok_or_else(invalid)?;
            if matches!(name, "run_officejs" | "functions.run_officejs") {
                return Err(invalid());
            }
            let ns = namespace.or_else(|| item["namespace"].as_str());
            if ns.is_some_and(|s| s.is_empty() || s.len() > 128) {
                return Err(invalid());
            }
            let key = ns
                .map(|ns| format!("{ns}.{name}"))
                .unwrap_or_else(|| name.into());
            if out.iter().any(|t| t.key == key) || out.len() >= 256 {
                return Err(invalid());
            }
            out.push(Tool {
                key,
                name: name.into(),
                namespace: ns.map(str::to_owned),
                custom: kind == "custom",
                definition: item.clone(),
            });
        }
        Ok(())
    }
    let mut tools = Vec::new();
    if let Some(items) = source.get("tools") {
        collect(items.as_array().ok_or_else(invalid)?, None, &mut tools, 0)?;
    }
    Ok(tools)
}

pub(super) fn allowed(tool: &Tool, choice: Option<&Value>) -> bool {
    match choice {
        None => true,
        Some(Value::String(s)) => matches!(s.as_str(), "auto" | "required"),
        Some(c) if c["type"] == "allowed_tools" => {
            let c = c.get("allowed_tools").unwrap_or(c);
            c["tools"]
                .as_array()
                .is_some_and(|list| list.iter().any(|c| selected(tool, c)))
        }
        Some(c) => selected(tool, c),
    }
}

pub(super) fn validate_choice(choice: Option<&Value>) -> Result<(), GatewayError> {
    let Some(choice) = choice else {
        return Ok(());
    };
    match choice {
        Value::String(s) if matches!(s.as_str(), "auto" | "none" | "required") => Ok(()),
        c if matches!(c["type"].as_str(), Some("function" | "custom")) => Ok(()),
        c if c["type"] == "allowed_tools" => {
            let fields = c.get("allowed_tools").unwrap_or(c);
            if matches!(fields["mode"].as_str(), Some("auto" | "required"))
                && fields["tools"].as_array().is_some_and(|v| {
                    v.len() <= 256
                        && v.iter()
                            .all(|t| matches!(t["type"].as_str(), Some("function" | "custom")))
                })
            {
                Ok(())
            } else {
                Err(invalid())
            }
        }
        _ => Err(invalid()),
    }
}

fn selected(tool: &Tool, choice: &Value) -> bool {
    let fields = choice
        .get("function")
        .or_else(|| choice.get("custom"))
        .unwrap_or(choice);
    choice["type"] == (if tool.custom { "custom" } else { "function" })
        && fields["name"] == tool.name
        && match &tool.namespace {
            Some(ns) => fields["namespace"].as_str() == Some(ns),
            None => fields.get("namespace").is_none_or(Value::is_null),
        }
}

pub(super) fn required(source: &Value) -> bool {
    let c = &source["tool_choice"];
    c == "required"
        || matches!(c["type"].as_str(), Some("function" | "custom"))
        || c["mode"] == "required"
        || c["allowed_tools"]["mode"] == "required"
}

pub(super) fn envelope(tool: &Tool, args: &Value) -> Result<String, GatewayError> {
    if (tool.custom && !args.is_string()) || (!tool.custom && !args.is_object()) {
        return Err(invalid());
    }
    Ok(json!({"summary":"Client tool relay", "extended_summary":"Client tool relay",
        "destructive":false,"references":[],"code":json!({"tool":tool.key,"args":args}).to_string()}).to_string())
}

pub(super) fn find<'a>(tools: &'a [Tool], item: &Value) -> Result<&'a Tool, GatewayError> {
    tools
        .iter()
        .find(|t| {
            item["type"]
                == (if t.custom {
                    "custom_tool_call"
                } else {
                    "function_call"
                })
                && item["name"] == t.name
                && match &t.namespace {
                    Some(ns) => item["namespace"].as_str() == Some(ns),
                    None => item.get("namespace").is_none_or(Value::is_null),
                }
        })
        .ok_or_else(invalid)
}

pub(super) fn convert_output(body: &mut Value, source: &Value) -> Result<(), GatewayError> {
    let tools = catalogue(source)?;
    let items = body["output"].as_array_mut().ok_or_else(invalid)?;
    let mut ids = HashSet::new();
    let mut count = 0;
    for item in items {
        if matches!(item["type"].as_str(), Some("message" | "reasoning")) {
            continue;
        }
        if !matches!(
            item["type"].as_str(),
            Some("function_call" | "custom_tool_call")
        ) {
            return Err(invalid());
        }
        if !matches!(
            item["name"].as_str(),
            Some("run_officejs" | "functions.run_officejs")
        ) {
            return Err(invalid());
        }
        let id = item["call_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(invalid)?
            .to_owned();
        if !ids.insert(id.clone()) {
            return Err(invalid());
        }
        let outer: Value = serde_json::from_str(item["arguments"].as_str().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        let inner: Value = serde_json::from_str(outer["code"].as_str().ok_or_else(invalid)?)
            .map_err(|_| invalid())?;
        let tool = tools
            .iter()
            .find(|t| inner["tool"] == t.key && allowed(t, source.get("tool_choice")))
            .ok_or_else(invalid)?;
        let args = inner
            .get("args")
            .or_else(|| inner.get("input"))
            .ok_or_else(invalid)?;
        envelope(tool, args)?;
        if !tool.custom {
            let definition = tool.definition.get("function").unwrap_or(&tool.definition);
            if definition
                .get("parameters")
                .is_some_and(|schema| !super::schema::accepts(schema, args))
            {
                return Err(invalid());
            }
        }
        *item = json!({"type":if tool.custom {"custom_tool_call"} else {"function_call"},
            "id":item.get("id").cloned().unwrap_or(json!(id)),"call_id":id,"name":tool.name,
            "status":"completed"});
        if let Some(ns) = &tool.namespace {
            item["namespace"] = json!(ns);
        }
        item[if tool.custom { "input" } else { "arguments" }] = if tool.custom {
            args.clone()
        } else {
            json!(args.to_string())
        };
        count += 1;
    }
    if (required(source) && count == 0) || (source["parallel_tool_calls"] == false && count > 1) {
        return Err(invalid());
    }
    Ok(())
}
