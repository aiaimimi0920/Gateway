//! 固定模型目录夹具；不记录身份材料，不通过网络取得模型。
use serde_json::{json, Value};

pub fn response(request: &Value) -> Value {
    let params = &request["params"];
    assert!(params.get("selectionId").is_none());
    let result = match params["gitHubToken"].as_str() {
        Some("catalog-a") => json!({"models":[{
            "id":"model-a", "name":"Account A", "capabilities":{"limits":{"max_output_tokens":4096}},
            "billing":{"multiplier":2.0}, "policy":{"state":"disabled","terms":"fixture terms"}
        }]}),
        Some("catalog-b") => {
            json!({"models":[{"id":"model-b","name":"Account B","capabilities":{}}]})
        }
        Some("catalog-empty") => json!({"models":[]}),
        Some("catalog-malformed") => json!({"models":[{"id":"missing-required-fields"}]}),
        _ => {
            return json!({"jsonrpc":"2.0","id":request["id"],"error":{
                "code":-32000,"message":"fixture denied; never expose catalog-denied"
            }})
        }
    };
    json!({"jsonrpc":"2.0","id":request["id"],"result":result})
}
