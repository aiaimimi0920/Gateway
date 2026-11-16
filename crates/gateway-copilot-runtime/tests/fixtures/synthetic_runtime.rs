//! 仅实验对端：拒绝未知 RPC，不调用网络、不执行工具、不读取本机账号。
use serde_json::{json, Value};
use std::io::{BufRead, Read, Write};
mod catalog;
mod text_turn;

fn write_frame(output: &mut impl Write, response: Value) {
    let bytes = serde_json::to_vec(&response).unwrap();
    write!(output, "Content-Length: {}\r\n\r\n", bytes.len()).unwrap();
    output.write_all(&bytes).unwrap();
    output.flush().unwrap();
}

fn main() {
    let allowed = std::path::PathBuf::from(std::env::var_os("GATEWAY_PROBE_ROOT").unwrap());
    let home = std::path::PathBuf::from(std::env::var_os("COPILOT_HOME").unwrap());
    let root = home.parent().unwrap().to_path_buf();
    assert!(root
        .canonicalize()
        .unwrap()
        .starts_with(allowed.canonicalize().unwrap()));
    let args: Vec<String> = std::env::args().collect();
    assert!(args.iter().any(|v| v == "--stdio"));
    assert!(args.iter().any(|v| v == "--no-auto-login"));
    assert!(!args.iter().any(|v| v.contains("synthetic-token")));
    for name in ["GH_TOKEN", "GITHUB_TOKEN", "COPILOT_GITHUB_TOKEN"] {
        assert!(std::env::var_os(name).is_none());
    }
    assert_eq!(std::env::var_os("COPILOT_HOME").unwrap(), root.join("home"));
    std::fs::write(root.join("child.pid"), std::process::id().to_string()).unwrap();
    // Independent files avoid interleaved writes from parallel fixture children.
    std::fs::write(
        allowed.join(format!("child-{}.json", std::process::id())),
        serde_json::to_vec(&json!({"pid":std::process::id(),"directory":root})).unwrap(),
    )
    .unwrap();
    let mut log = std::fs::File::create(root.join("rpc.jsonl")).unwrap();
    let mut audit =
        std::fs::File::create(allowed.join(format!("child-{}-rpc.jsonl", std::process::id())))
            .unwrap();
    let mut input = std::io::stdin().lock();
    let mut output = std::io::stdout().lock();
    let mut shutdown = false;
    loop {
        let mut header = String::new();
        if input.read_line(&mut header).unwrap() == 0 {
            break;
        }
        let length: usize = header
            .trim()
            .strip_prefix("Content-Length: ")
            .unwrap()
            .parse()
            .unwrap();
        assert!(length < 65536);
        header.clear();
        input.read_line(&mut header).unwrap();
        assert_eq!(header, "\r\n");
        let mut bytes = vec![0; length];
        input.read_exact(&mut bytes).unwrap();
        let request: Value = serde_json::from_slice(&bytes).unwrap();
        let method = request["method"].as_str().unwrap();
        let params = &request["params"];
        // 只记录方法名；不把 token 写入证据。
        writeln!(log, "{}", json!({"method":method})).unwrap();
        writeln!(audit, "{}", json!({"method":method})).unwrap();
        let control = match method {
            "connect" => Some("hold-connect"),
            "session.sendAndWait" => Some("hold-text"),
            _ => None,
        };
        if let Some(control) = control {
            while root.parent().unwrap().join(control).is_file() {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
        }
        if method == "$/cancelRequest" {
            assert!(request.get("id").is_none());
            continue;
        }
        if method == "session.sendAndWait" {
            if let Some(response) = text_turn::response(&request) {
                write_frame(
                    &mut output,
                    json!({"jsonrpc":"2.0", "method":"session.event",
                    "params":{"sessionId":params["sessionId"], "event":text_turn::idle()}}),
                );
                write_frame(&mut output, response);
            }
            continue;
        }
        if method == "models.list" {
            if params["gitHubToken"] == "catalog-hang" {
                continue;
            }
            write_frame(&mut output, catalog::response(&request));
            continue;
        }
        let result = match method {
            "connect" => json!({"ok":true,"protocolVersion":3,"version":"synthetic"}),
            "session.create" => {
                assert_eq!(params["gitHubToken"], "synthetic-token");
                assert_eq!(params["availableTools"], json!([]));
                assert_eq!(params["includedBuiltinAgents"], json!([]));
                assert_eq!(params["model"], "synthetic-model");
                json!({"sessionId":params["sessionId"],"workspacePath":root})
            }
            "session.options.update" => {
                std::fs::write(
                    root.join("options.json"),
                    serde_json::to_vec(params).unwrap(),
                )
                .unwrap();
                json!({"success":true})
            }
            "session.detach" => json!({"success":true}),
            "session.skills.reload" => json!({}),
            "runtime.shutdown" => {
                shutdown = true;
                json!({})
            }
            _ => panic!("unexpected RPC: {method}"),
        };
        write_frame(
            &mut output,
            json!({"jsonrpc":"2.0","id":request["id"],"result":result}),
        );
    }
    assert!(shutdown, "stdin closed without runtime.shutdown");
    std::fs::write(
        root.join("graceful-exit"),
        "shutdown acknowledged; stdin EOF",
    )
    .unwrap();
}
