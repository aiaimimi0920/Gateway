use rquest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, ORIGIN, REFERER, USER_AGENT};
use rquest_util::Emulation;
use serde_json::json;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let token = std::env::var("PRODUCER_SESSION_BEARER_1")
        .map_err(|_| anyhow::anyhow!("PRODUCER_SESSION_BEARER_1 is required"))?;
    let clip_id = std::env::var("PRODUCER_CLIP_ID")
        .unwrap_or_else(|_| "698c44ed-d021-4231-89e0-9a005cca3c58".to_string());

    let client = neuro_gateway::http_client::builder()
        .emulation(Emulation::Chrome131)
        .build()?;

    let mut headers = HeaderMap::new();
    headers.insert(
        AUTHORIZATION,
        HeaderValue::from_str(&format!("Bearer {token}"))?,
    );
    headers.insert(
        ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(ORIGIN, HeaderValue::from_static("https://www.producer.ai"));
    headers.insert(
        REFERER,
        HeaderValue::from_static("https://www.producer.ai/"),
    );
    headers.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/146.0.0.0 Safari/537.36",
        ),
    );

    let get_resp = client
        .get("https://www.producer.ai/__api/music-videos/get")
        .headers(headers.clone())
        .query(&[("offset", "0"), ("limit", "1")])
        .send()
        .await?;
    let get_status = get_resp.status();
    let get_text = get_resp.text().await?;
    println!("GET_STATUS={get_status}");
    println!(
        "GET_BODY={}",
        if get_text.len() > 1200 {
            &get_text[..1200]
        } else {
            &get_text
        }
    );

    let post_resp = client
        .post("https://www.producer.ai/__api/producer/tool-call")
        .headers(headers)
        .json(&json!({
            "part": {
                "tool_name": "video__create_music_video",
                "args": {
                    "clip_id": clip_id,
                    "user_message": "Create a cinematic synthwave performance video with neon skyline lighting, retro stage energy, and smooth camera motion. No text overlays.",
                    "aspect_ratio": "16:9",
                    "resolution": "720p",
                    "render_lyrics": false,
                    "duration_s": 12
                }
            },
            "client_context": {
                "current_song_id": clip_id,
                "song_queue": [clip_id],
                "project_id": null,
                "selected_model": "producer:music-video",
                "lyrics_id_map": {},
                "ghostwriter_version": "standard"
            }
        }))
        .send()
        .await?;
    let post_status = post_resp.status();
    let post_text = post_resp.text().await?;
    println!("POST_STATUS={post_status}");
    println!(
        "POST_BODY={}",
        if post_text.len() > 1200 {
            &post_text[..1200]
        } else {
            &post_text
        }
    );

    Ok(())
}
