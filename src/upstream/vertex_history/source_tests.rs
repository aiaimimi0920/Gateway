//! Both directions reject the other valid source; enum recognition is not authorization to restore it.
use super::*;
use crate::protocol::{gemini::api::pack_request, openai::normalize_chat_completions};
#[test]
fn vertex_history_source_is_scoped_to_product_even_when_model_and_visible_text_match() {
    for contract in [EXPRESS, OAUTH, DIM] {
        for source in [EXPRESS.source, OAUTH.source, DIM.source] {
            let req=normalize_chat_completions(json!({"messages":[{"role":"assistant","content":"old"},{"role":"user","content":"new"}],
                "gateway_vertex_history":[{"assistant_turn":0,"turn":{"source":source,"model":"model","parts":[{"text":"old","thought_signature":"YWJj"}]}}]})).unwrap();
            let contents = pack_request(&req, "model", true)["contents"].take();
            let result = normalize(contract, &req, contents, "model");
            if source == contract.source {
                assert_eq!(result.unwrap()[0]["parts"][0]["thoughtSignature"], "YWJj");
            } else {
                assert_eq!(
                    result.unwrap_err().code,
                    Some(format!("{}_invalid_signed_history", contract.code_prefix))
                );
            }
        }
    }
}
