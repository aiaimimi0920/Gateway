//! Responses item/event envelopes; terminal status follows the existing framework converter.
use super::*;
impl Projection {
    pub(super) fn start_responses(&mut self, out: &mut Vec<u8>) -> Result<(), BusinessError> {
        let response = json!({"id":self.id,"object":"response","model":self.model,"status":"in_progress","output":[]});
        self.emit("response.created", json!({"response":response}), out)?;
        self.emit("response.in_progress", json!({"response":response}), out)
    }
    pub(super) fn text_start_responses(
        &mut self,
        index: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        let id = self.item_id(index);
        self.emit("response.output_item.added", json!({"output_index":index,"item":{"id":id,"type":"message","role":"assistant","status":"in_progress","content":[]}}), out)?;
        self.emit("response.content_part.added", json!({"output_index":index,"item_id":id,"content_index":0,"part":{"type":"output_text","text":""}}), out)
    }
    pub(super) fn text_delta_responses(
        &mut self,
        index: usize,
        text: &str,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        self.emit("response.output_text.delta", json!({"output_index":index,"item_id":self.item_id(index),"content_index":0,"delta":text}), out)
    }
    pub(super) fn tool_start_responses(
        &mut self,
        index: usize,
        tool: &Value,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        self.emit("response.output_item.added", json!({"output_index":index,"item":{"id":self.item_id(index),"type":"function_call","status":"in_progress","call_id":tool["id"],"name":tool["function"]["name"],"arguments":""}}), out)
    }
    pub(super) fn tool_delta_responses(
        &mut self,
        index: usize,
        args: &str,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        self.emit(
            "response.function_call_arguments.delta",
            json!({"output_index":index,"item_id":self.item_id(index),"delta":args}),
            out,
        )
    }
    pub(super) fn part_done_responses(
        &mut self,
        index: usize,
        part: &Value,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        let id = self.item_id(index);
        if part["type"] == "message" {
            let text = &part["content"][0]["text"];
            self.emit(
                "response.output_text.done",
                json!({"output_index":index,"item_id":id,"content_index":0,"text":text}),
                out,
            )?;
            self.emit("response.content_part.done", json!({"output_index":index,"item_id":id,"content_index":0,"part":part["content"][0]}), out)?;
        } else {
            self.emit(
                "response.function_call_arguments.done",
                json!({"output_index":index,"item_id":id,"arguments":part["arguments"]}),
                out,
            )?;
        }
        self.emit(
            "response.output_item.done",
            json!({"output_index":index,"item":part}),
            out,
        )
    }
    pub(super) fn finish_responses(
        &mut self,
        response: Value,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        let event = match response["status"].as_str() {
            Some("incomplete") => "response.incomplete",
            Some("failed" | "cancelled") => "response.failed",
            _ => "response.completed",
        };
        self.emit(event, json!({"response":response}), out)
    }
}
