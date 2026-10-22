//! Full messages can be replayed across providers without foreign resource IDs.
use serde_json::Value;

pub(crate) fn sanitize_replayed_message_ids(body: &mut Value) {
    let Some(items) = body.get_mut("input").and_then(Value::as_array_mut) else {
        return;
    };
    for item in items {
        let message = match item.get("type").and_then(Value::as_str) {
            Some("message") => true,
            None => item.get("role").and_then(Value::as_str).is_some(),
            _ => false,
        };
        // Only inline messages are self-contained. Do not rewrite references,
        // reasoning IDs or call_id links, which carry upstream state semantics.
        if message
            && item.get("content").is_some()
            && item
                .get("id")
                .and_then(Value::as_str)
                .is_some_and(|id| !id.starts_with("msg"))
        {
            if let Some(object) = item.as_object_mut() {
                object.remove("id");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn replayed_message_ids_drop_foreign_ids_only() {
        let mut body = json!({"input":[
            {"type":"message","role":"assistant","id":"item_bc380a9108fc9f0b708b5b9e","content":[{"type":"output_text","text":"hello"}]},
            {"role":"user","id":"item_user","content":"next"},
            {"type":"message","role":"assistant","id":"msg_valid","content":[]},
            {"type":"item_reference","id":"item_ref"},
            {"type":"function_call","id":"fc_1","call_id":"item_call"},
            {"type":"reasoning","id":"rs_1","summary":[]}
        ]});
        let original = body.clone();
        sanitize_replayed_message_ids(&mut body);
        assert!(body["input"][0].get("id").is_none());
        assert!(body["input"][1].get("id").is_none());
        assert_eq!(body["input"][0]["content"], original["input"][0]["content"]);
        for index in 2..6 {
            assert_eq!(body["input"][index], original["input"][index]);
        }
        let once = body.clone();
        sanitize_replayed_message_ids(&mut body);
        assert_eq!(body, once);
    }
}
