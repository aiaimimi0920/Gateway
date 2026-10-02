//! 默认环境代理与 bypass 在独立子进程中验证，不修改测试进程或 Windows 代理设置。
#[path = "http_client_connection/fixture.rs"]
mod fixture;

use fixture::{receive, reply, DEADLINE};
use tokio::net::TcpListener;
use tokio::process::Command;

fn uses_proxy(mode: &str) -> bool {
    matches!(
        mode,
        "environment" | "nonmatching-bypass" | "all" | "cgi-all" | "specific" | "auth"
    )
}

fn bypass_value(mode: &str) -> &str {
    match mode {
        "bypass" => "127.0.0.1",
        "nonmatching-bypass" => "not-matching.invalid",
        _ => "",
    }
}

async fn isolated_proxy_case(mode: &str) {
    let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let proxy = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/proxy-contract", origin.local_addr().unwrap());
    let proxy_url = format!("http://{}", proxy.local_addr().unwrap());
    let mut command = Command::new(std::env::current_exe().unwrap());
    command
        .args(["--exact", "proxy_child", "--ignored", "--nocapture"])
        .env("GATEWAY_HTTP_PROXY_FIXTURE_MODE", mode)
        .env("GATEWAY_HTTP_PROXY_FIXTURE_URL", &url)
        .env_remove("ALL_PROXY")
        .env_remove("all_proxy")
        .env_remove("HTTPS_PROXY")
        .env_remove("https_proxy")
        .env_remove("REQUEST_METHOD")
        .env("HTTP_PROXY", &proxy_url)
        .env("http_proxy", &proxy_url)
        .env("NO_PROXY", bypass_value(mode))
        .env("no_proxy", bypass_value(mode))
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .kill_on_drop(true);
    if matches!(mode, "all" | "cgi-all") {
        command
            .env_remove("HTTP_PROXY")
            .env_remove("http_proxy")
            .env("ALL_PROXY", &proxy_url)
            .env("all_proxy", &proxy_url);
    }
    if mode == "cgi-all" {
        command
            .env("REQUEST_METHOD", "GET")
            .env("HTTP_PROXY", "http://127.0.0.1:1")
            .env("http_proxy", "http://127.0.0.1:1");
    }
    if mode == "specific" {
        command
            .env("ALL_PROXY", "http://127.0.0.1:1")
            .env("all_proxy", "http://127.0.0.1:1");
    }
    if mode == "auth" {
        let authenticated = proxy_url.replacen("http://", "http://fixture-user:fixture-pass@", 1);
        command
            .env("HTTP_PROXY", &authenticated)
            .env("http_proxy", authenticated);
    }
    #[cfg(windows)]
    command.creation_flags(0x08000000); // CREATE_NO_WINDOW：测试子进程不弹出窗口。
    let child = command.spawn().unwrap();
    let server = async {
        let selected = if uses_proxy(mode) { &proxy } else { &origin };
        let (mut socket, _) = selected.accept().await.unwrap();
        let request = receive(&mut socket).await;
        assert_eq!(request.method, "GET");
        assert!(request.header("host").is_some());
        assert!(request.body.is_empty());
        assert_eq!(
            request.header("proxy-authorization"),
            if mode == "auth" {
                Some("Basic Zml4dHVyZS11c2VyOmZpeHR1cmUtcGFzcw==")
            } else {
                None
            }
        );
        assert_eq!(
            request.target,
            if uses_proxy(mode) {
                url.as_str()
            } else {
                "/proxy-contract"
            }
        );
        reply(&mut socket, "200 OK", "", mode, true).await;
    };
    let run = async {
        let output = child.wait_with_output().await.unwrap();
        let stdout = String::from_utf8(output.stdout).unwrap();
        assert!(
            output.status.success(),
            "isolated proxy child failed: {stdout}; {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(
            stdout.contains("HTTP_PROXY_CHILD_OK"),
            "child must execute the selected case"
        );
        assert!(stdout.contains("1 passed"));
    };
    // 子进程 owner 包含 kill_on_drop；超时会丢弃 join、socket 和子进程，不留下 server task。
    tokio::time::timeout(DEADLINE + DEADLINE, async { tokio::join!(server, run) })
        .await
        .unwrap();
}

#[tokio::test]
async fn default_client_discovers_environment_proxy() {
    isolated_proxy_case("environment").await;
}

#[tokio::test]
async fn no_proxy_environment_bypasses_default_proxy() {
    isolated_proxy_case("bypass").await;
}

#[tokio::test]
async fn explicit_no_proxy_disables_environment_proxy() {
    isolated_proxy_case("disabled").await;
}

#[tokio::test]
async fn nonmatching_bypass_does_not_load_windows_bypass() {
    isolated_proxy_case("nonmatching-bypass").await;
}

#[tokio::test]
async fn all_proxy_is_used_without_http_proxy() {
    isolated_proxy_case("all").await;
}

#[tokio::test]
async fn cgi_ignores_http_but_retains_all_proxy() {
    isolated_proxy_case("cgi-all").await;
}

#[tokio::test]
async fn http_proxy_overrides_all_proxy() {
    isolated_proxy_case("specific").await;
}

#[tokio::test]
async fn environment_proxy_preserves_synthetic_basic_auth() {
    isolated_proxy_case("auth").await;
}

#[tokio::test]
#[ignore = "由八个父测试在隔离环境中启动；不是未执行的业务合同"]
async fn proxy_child() {
    let mode = std::env::var("GATEWAY_HTTP_PROXY_FIXTURE_MODE").unwrap();
    assert!(matches!(
        mode.as_str(),
        "environment"
            | "bypass"
            | "disabled"
            | "nonmatching-bypass"
            | "all"
            | "cgi-all"
            | "specific"
            | "auth"
    ));
    assert_eq!(std::env::var("NO_PROXY").unwrap(), bypass_value(&mode));
    let url = std::env::var("GATEWAY_HTTP_PROXY_FIXTURE_URL").unwrap();
    let mut builder = neuro_gateway::http_client::builder()
        .http1_only()
        .timeout(DEADLINE);
    if mode == "disabled" {
        builder = builder.no_proxy();
    }
    let response = builder.build().unwrap().get(url).send().await.unwrap();
    assert_eq!(response.text().await.unwrap(), mode);
    println!("HTTP_PROXY_CHILD_OK");
}
