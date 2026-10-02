//! 真实 loopback 保护连接复用、重定向请求体和跨 origin 的敏感头边界。
#[path = "http_client_connection/fixture.rs"]
mod fixture;

use rquest::Client;

use fixture::{receive, reply, DEADLINE};
use tokio::net::TcpListener;

fn client() -> Client {
    neuro_gateway::http_client::builder()
        .no_proxy()
        .http1_only()
        .redirect(rquest::redirect::Policy::limited(5))
        .timeout(DEADLINE)
        .build()
        .unwrap()
}

#[tokio::test]
async fn cloned_client_reuses_a_connection_after_a_completed_body() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/reuse", listener.local_addr().unwrap());
    let server = async {
        // 第二个请求必须出现在同一个已接受的 socket，而非第二次 accept。
        let (mut socket, _) = listener.accept().await.unwrap();
        for _ in 0..2 {
            let request = receive(&mut socket).await;
            assert_eq!(request.method, "GET");
            assert_eq!(request.target, "/reuse");
            reply(&mut socket, "200 OK", "", "reused", false).await;
        }
    };
    let request = async {
        let first = client();
        let second = first.clone();
        for client in [first, second] {
            assert_eq!(
                client.get(&url).send().await.unwrap().text().await.unwrap(),
                "reused"
            );
        }
    };
    tokio::time::timeout(DEADLINE, async { tokio::join!(server, request) })
        .await
        .unwrap();
}

#[tokio::test]
async fn temporary_redirect_preserves_post_body_and_content_type() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/first", listener.local_addr().unwrap());
    let payload = r#"{"message":"fixture"}"#;
    let server = async {
        for path in ["/first", "/second"] {
            let (mut socket, _) = listener.accept().await.unwrap();
            let request = receive(&mut socket).await;
            assert_eq!(request.method, "POST");
            assert_eq!(request.target, path);
            assert_eq!(request.body, payload.as_bytes());
            assert_eq!(request.header("content-type"), Some("application/json"));
            if path == "/first" {
                reply(
                    &mut socket,
                    "307 Temporary Redirect",
                    "Location: /second\r\n",
                    "",
                    true,
                )
                .await;
            } else {
                reply(&mut socket, "200 OK", "", "accepted", true).await;
            }
        }
    };
    let request = async {
        let response = client()
            .post(url)
            .header("content-type", "application/json")
            .body(payload)
            .send()
            .await
            .unwrap();
        assert_eq!(response.status().as_u16(), 200);
        assert_eq!(response.text().await.unwrap(), "accepted");
    };
    tokio::time::timeout(DEADLINE, async { tokio::join!(server, request) })
        .await
        .unwrap();
}

#[tokio::test]
async fn cross_origin_redirect_strips_authorization_and_cookie_headers() {
    let origin = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let destination = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}/first", origin.local_addr().unwrap());
    let final_url = format!("http://{}/final", destination.local_addr().unwrap());
    let first = async {
        let (mut socket, _) = origin.accept().await.unwrap();
        let request = receive(&mut socket).await;
        assert_eq!(
            request.header("authorization"),
            Some("Bearer fixture-secret")
        );
        assert_eq!(request.header("cookie"), Some("fixture=private"));
        reply(
            &mut socket,
            "302 Found",
            &format!("Location: {final_url}\r\n"),
            "",
            true,
        )
        .await;
    };
    let final_server = async {
        let (mut socket, _) = destination.accept().await.unwrap();
        let request = receive(&mut socket).await;
        assert_eq!(request.target, "/final");
        assert!(request.header("authorization").is_none());
        assert!(request.header("cookie").is_none());
        reply(&mut socket, "200 OK", "", "safe", true).await;
    };
    let request = async {
        let response = client()
            .get(url)
            .header("authorization", "Bearer fixture-secret")
            .header("cookie", "fixture=private")
            .send()
            .await
            .unwrap();
        assert_eq!(response.text().await.unwrap(), "safe");
    };
    tokio::time::timeout(DEADLINE, async {
        tokio::join!(first, final_server, request)
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn default_and_request_redirect_none_do_not_follow() {
    for override_policy in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/first", listener.local_addr().unwrap());
        let server = async {
            let (mut socket, _) = listener.accept().await.unwrap();
            assert_eq!(receive(&mut socket).await.target, "/first");
            reply(
                &mut socket,
                "302 Found",
                "Location: /not-followed\r\n",
                "original",
                true,
            )
            .await;
        };
        let request = async {
            // 两客户端 builder 默认不跟随；请求级 none 还必须覆盖显式 follow client。
            let request = if override_policy {
                client().get(url).redirect(rquest::redirect::Policy::none())
            } else {
                neuro_gateway::http_client::builder()
                    .no_proxy()
                    .http1_only()
                    .timeout(DEADLINE)
                    .build()
                    .unwrap()
                    .get(url)
            };
            let response = request.send().await.unwrap();
            assert_eq!(response.status().as_u16(), 302);
            assert_eq!(response.headers()["location"], "/not-followed");
            assert_eq!(response.text().await.unwrap(), "original");
        };
        tokio::time::timeout(DEADLINE, async { tokio::join!(server, request) })
            .await
            .unwrap();
    }
}
