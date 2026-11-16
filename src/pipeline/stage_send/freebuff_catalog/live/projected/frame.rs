//! Only already-validated native frames arrive here. Tool association reuses the native decoder.
use super::*;
impl Projection {
    pub fn frame(&mut self, frame: Value) -> Result<Bytes, BusinessError> {
        let choices = frame["choices"]
            .as_array()
            .ok_or(BusinessError::InvalidResponse)?;
        if choices.len() > 1
            || choices
                .first()
                .is_some_and(|c| c.get("index").is_some_and(|i| i.as_u64() != Some(0)))
        {
            return Err(BusinessError::InvalidResponse);
        }
        self.observed = true;
        let mut out = Vec::new();
        self.start(&mut out)?;
        if let Some(choice) = choices.first() {
            self.delta(&choice["delta"], &mut out)?;
        }
        // Explicit native observation, never a thinking/signature event or a successful terminal.
        self.emit(
            "gateway_freebuff.delta",
            json!({"version":1,"frame":frame}),
            &mut out,
        )?;
        Ok(Bytes::from(out))
    }
    pub(super) fn delta(&mut self, delta: &Value, out: &mut Vec<u8>) -> Result<(), BusinessError> {
        if let Some(text) = delta["content"].as_str().filter(|t| !t.is_empty()) {
            self.text_delta(text, out)?;
        }
        if let Some(tools) = delta["tool_calls"].as_array() {
            for call in tools {
                self.tool(call, out)?;
            }
        }
        Ok(())
    }
    fn tool(&mut self, call: &Value, out: &mut Vec<u8>) -> Result<(), BusinessError> {
        let native = crate::protocol::freebuff::catalog::resolve_stream_tool_index(
            call,
            self.tools
                .iter()
                .map(|(index, tool)| (*index, &tool.identity)),
        )?;
        let tool = self.tools.entry(native).or_insert_with(|| Tool {
            identity: json!({"function":{"name":call["function"]["name"]}}),
            pending: String::new(),
            output: None,
        });
        if let Some(id) = call["id"].as_str() {
            tool.identity["id"] = json!(id);
        }
        if let Some(args) = call["function"]["arguments"].as_str() {
            tool.pending.push_str(args);
        }
        // A native ID can arrive after arguments. Retain only those not yet deliverable; do not
        // invent a call ID that cannot be used by the caller's next tool-result request.
        if tool.identity["id"].as_str().is_none() {
            return Ok(());
        }
        let added = tool.output.is_none();
        let index = *tool.output.get_or_insert_with(|| {
            let index = self.order.len();
            self.order.push(Slot::Tool(native));
            index
        });
        let identity = tool.identity.clone();
        let args = std::mem::take(&mut tool.pending);
        if added {
            self.tool_start(index, &identity, out)?;
        }
        if !args.is_empty() {
            self.tool_delta(index, &args, out)?;
        }
        Ok(())
    }
}
