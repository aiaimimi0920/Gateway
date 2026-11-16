//! Messages indexed blocks. Native reasoning stays an explicit extension, never forged thinking.
use super::*;
impl Projection {
    pub(super) fn start_messages(&mut self, out: &mut Vec<u8>) -> Result<(), BusinessError> {
        self.emit("message_start", json!({"message":{"id":self.id,"type":"message","role":"assistant","model":self.model,"content":[],"stop_reason":null,"stop_sequence":null}}), out)
    }
    pub(super) fn text_start_messages(
        &mut self,
        index: usize,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        self.emit(
            "content_block_start",
            json!({"index":index,"content_block":{"type":"text","text":""}}),
            out,
        )
    }
    pub(super) fn text_delta_messages(
        &mut self,
        index: usize,
        text: &str,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        self.emit(
            "content_block_delta",
            json!({"index":index,"delta":{"type":"text_delta","text":text}}),
            out,
        )
    }
    pub(super) fn tool_start_messages(
        &mut self,
        index: usize,
        tool: &Value,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        self.emit("content_block_start", json!({"index":index,"content_block":{"type":"tool_use","id":tool["id"],"name":tool["function"]["name"],"input":{}}}), out)
    }
    pub(super) fn tool_delta_messages(
        &mut self,
        index: usize,
        args: &str,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        self.emit(
            "content_block_delta",
            json!({"index":index,"delta":{"type":"input_json_delta","partial_json":args}}),
            out,
        )
    }
    pub(super) fn part_done_messages(
        &mut self,
        index: usize,
        _part: &Value,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        self.emit("content_block_stop", json!({"index":index}), out)
    }
    pub(super) fn finish_messages(
        &mut self,
        response: Value,
        out: &mut Vec<u8>,
    ) -> Result<(), BusinessError> {
        let mut delta = json!({"delta":{"stop_reason":response["stop_reason"],"stop_sequence":response["stop_sequence"]},"gateway_freebuff":response["gateway_freebuff"]});
        if let Some(usage) = response.get("usage") {
            delta["usage"] = usage.clone();
        }
        self.emit("message_delta", delta, out)?;
        self.emit("message_stop", json!({}), out)
    }
}
