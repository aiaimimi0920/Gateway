//! The same inline admission and no-replay owner also serves WS response.create.
use super::*;
use crate::protocol::cursor_remote::{
    inline_images::tests::{message, url},
    upstream::validate_images,
};

#[test]
fn cursor_remote_websocket_inline_image_same_connection_tool_continuation() {
    run(|| async {
        let state = app_worker(
            true,
            Runtime::fixture(|input, hooks| {
                Box::pin(async move {
                    let images = validate_images(input.get("images"))
                        .map_err(|e| crate::protocol::cursor_remote::error(e.code))?;
                    if images.len() != 1 {
                        return Err(crate::protocol::cursor_remote::error("worker_failed"));
                    }
                    hooks.authorize().await?;
                    hooks.sent()?;
                    let output = hooks
                        .tool(WorkerToolCall {
                            call_id: "call_image_ws".into(),
                            name: "echo".into(),
                            arguments: "{}".into(),
                            text: String::new(),
                            thinking: None,
                            requested_model: "default".into(),
                        })
                        .await?;
                    if output.output != "ok" {
                        return Err(crate::protocol::cursor_remote::error("worker_failed"));
                    }
                    hooks.continued()?;
                    Ok(WorkerOutput {
                        usage: None,
                        text: "image done".into(),
                        thinking: None,
                        requested_model: "default".into(),
                    })
                })
            }),
        )
        .await;
        let token = key(&state, "responses", "cursor-remote").await;
        let server = Server::start(state.clone()).await;
        let mut socket = server.connect(&token).await;
        let original = message(&url(image::ImageFormat::Png, 10), false);
        let mut history = vec![original];
        let event = turn(&mut socket, body(json!(history), false)).await;
        assert_eq!(event["type"], "response.completed", "{event}");
        assert!(event.to_string().contains("call_image_ws"), "{event}");
        history.push(
            json!({"type":"function_call","name":"echo","call_id":"call_image_ws","arguments":"{}"}),
        );
        history
            .push(json!({"type":"function_call_output","call_id":"call_image_ws","output":"ok"}));
        let mut changed = history.clone();
        changed[0] = message(&url(image::ImageFormat::Png, 20), false);
        let rejected = turn(&mut socket, body(json!(changed), false)).await;
        assert_eq!(rejected["type"], "error", "{rejected}");
        assert!(rejected
            .to_string()
            .contains("Cursor remote execution did not complete"));
        let completed = turn(&mut socket, body(json!(history), false)).await;
        assert_eq!(completed["type"], "response.completed", "{completed}");
        assert!(completed.to_string().contains("image done"));
        socket.close(None).await.unwrap();
        drop(socket);
        state.lifecycle.cursor_remote.join().await;
        drop(server);
        state.local_runtime.as_ref().unwrap().close().await;
    });
}
