use neuro_gateway::upstream::client::UpstreamClient;
#[tokio::main]
async fn main() {
    let client = UpstreamClient::new(30);
    let http = client.client().clone();
    for url in ["https://s.jina.ai/search", "https://r.jina.ai/"] {
        let req = if url.contains("s.jina.ai") {
            http.post(url)
                .header(
                    "Authorization",
                    "Bearer jina_81b9408d7c0840d4a12c78ee7806e9e2fq0ocfabErmkfiKFTdj4N9zUfK0e",
                )
                .header("Accept", "application/json")
                .json(&serde_json::json!({"q": "OpenAI"}))
        } else {
            http.post(url)
                .header(
                    "Authorization",
                    "Bearer jina_81b9408d7c0840d4a12c78ee7806e9e2fq0ocfabErmkfiKFTdj4N9zUfK0e",
                )
                .header("Accept", "application/json")
                .json(&serde_json::json!({"url": "http://example.com"}))
        };
        match req.send().await {
            Ok(resp) => {
                println!(
                    "{} => {} {:?}",
                    url,
                    resp.status(),
                    resp.headers().get("content-type")
                );
            }
            Err(err) => {
                println!("{} => ERR {}", url, err);
            }
        }
    }
}
