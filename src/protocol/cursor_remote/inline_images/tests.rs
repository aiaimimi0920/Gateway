use super::*;
use crate::protocol::cursor_remote::ingress::{self, Execution};

pub(crate) fn url(format: image::ImageFormat, red: u8) -> String {
    let image =
        image::DynamicImage::ImageRgb8(image::RgbImage::from_pixel(2, 2, image::Rgb([red, 0, 0])));
    let mut bytes = std::io::Cursor::new(Vec::new());
    image.write_to(&mut bytes, format).unwrap();
    let mime = if format == image::ImageFormat::Png {
        "image/png"
    } else {
        "image/jpeg"
    };
    format!("data:{mime};base64,{}", STANDARD.encode(bytes.into_inner()))
}
pub(crate) fn message(url: &str, chat: bool) -> Value {
    let image = if chat {
        json!({"type":"image_url","image_url":{"url":url}})
    } else {
        json!({"type":"input_image","image_url":url})
    };
    json!({"role":"user","content":[
        {"type":if chat {"text"} else {"input_text"},"text":"hello do not call tools."}, image,
    ]})
}
fn parse(body: Value, chat: bool) -> Result<Execution, GatewayError> {
    let req = if chat {
        crate::protocol::openai::normalize_chat_completions(body)?
    } else {
        crate::protocol::responses::normalize_responses(body)?
    };
    ingress::parse(&req)
}
pub(crate) fn body(chat: bool, history: Value, choice: Option<&str>, stream: bool) -> Value {
    let mut body = json!({"model":"cursor-personal","stream":stream,"store":false});
    body[if chat { "messages" } else { "input" }] = history;
    if let Some(choice) = choice {
        let tool = json!({"name":"echo","parameters":{"type":"object"}});
        body["tools"] = if chat {
            json!([{"type":"function","function":tool}])
        } else {
            json!([{"type":"function","name":"echo","parameters":{"type":"object"}}])
        };
        body["tool_choice"] = json!(choice);
    }
    body
}

#[tokio::test]
async fn cursor_remote_inline_png_jpeg_text_none_and_tools_keep_bytes_not_prompt() {
    for chat in [false, true] {
        for format in [image::ImageFormat::Png, image::ImageFormat::Jpeg] {
            let url = url(format, 10);
            for choice in [None, Some("none"), Some("auto")] {
                let execution = parse(
                    body(chat, json!([message(&url, chat)]), choice, false),
                    chat,
                )
                .unwrap();
                assert!(!execution.prompt().contains("base64"));
                assert!(execution.prompt().contains("hello do not call tools."));
                assert_eq!(
                    matches!(&execution, Execution::Tools(_)),
                    choice == Some("auto")
                );
                let mut input = json!({});
                execution.prepare_input(&mut input).await.unwrap();
                assert_eq!(input["images"][0]["data"], url.split_once(',').unwrap().1);
                assert!(input["images"][0].get("sha256").is_none());
            }
        }
        let mut only_image = message(&url(image::ImageFormat::Png, 0), chat);
        only_image["content"].as_array_mut().unwrap().remove(0);
        if chat {
            only_image["content"][0]["image_url"]["detail"] = json!("auto");
        } else {
            only_image["content"][0]["detail"] = json!("auto");
        }
        assert!(parse(body(chat, json!([only_image]), None, false), chat).is_ok());
    }
}

#[test]
fn cursor_remote_inline_rejects_unmapped_roles_fields_urls_and_budgets() {
    let valid = message(&url(image::ImageFormat::Png, 0), false);
    for image in [
        json!({"type":"input_image","image_url":"https://example.test/image.png"}),
        json!({"type":"input_image","file_id":"file_1"}),
        json!({"type":"input_image","image_url":"data:image/png;base64,AA=="}),
        json!({"type":"input_image","image_url":url(image::ImageFormat::Png, 0),"detail":"high"}),
        json!({"type":"input_image","image_url":url(image::ImageFormat::Png, 0),"unknown":true}),
        json!({"type":"input_image","image_url":url(image::ImageFormat::Jpeg, 0).replace("jpeg", "png")}),
    ] {
        let mut value = valid.clone();
        value["content"][1] = image;
        assert!(parse(body(false, json!([value]), None, false), false).is_err());
    }
    for role in ["assistant", "system", "developer", "tool"] {
        let mut value = valid.clone();
        value["role"] = json!(role);
        assert!(parse(body(false, json!([value]), None, false), false).is_err());
    }
    assert!(parse(
        body(
            false,
            json!([valid, {"role":"user","content":"later"}]),
            None,
            false
        ),
        false
    )
    .is_err());
    let mut bytes = vec![0; MAX_IMAGE];
    bytes[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    let data = format!("data:image/png;base64,{}", STANDARD.encode(&bytes));
    assert!(data_url(&data).is_ok());
    assert!(data_url(&(data.clone() + "\n")).is_err());
    bytes.push(0);
    assert!(data_url(&format!("data:image/png;base64,{}", STANDARD.encode(bytes))).is_err());
    let image = json!({"type":"input_image","image_url":data});
    assert!(parse(
        body(
            false,
            json!([{"role":"user","content":vec![image; 3]}]),
            None,
            false
        ),
        false
    )
    .is_err());
    let mut total = vec![0; 768 * 1024];
    total[..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
    let image = json!({"type":"input_image","image_url":format!(
        "data:image/png;base64,{}", STANDARD.encode(total),
    )});
    assert!(parse(
        body(
            false,
            json!([{"role":"user","content":vec![image; 3]}]),
            None,
            false
        ),
        false
    )
    .is_err());
    let small = json!({"type":"input_image","image_url":url(image::ImageFormat::Png, 0)});
    assert!(parse(
        body(
            false,
            json!([{"role":"user","content":vec![small; 5]}]),
            None,
            false
        ),
        false
    )
    .is_err());
    let mut large_text = message(&url(image::ImageFormat::Png, 0), false);
    large_text["content"][0]["text"] = json!("a".repeat(512 * 1024));
    assert!(parse(body(false, json!([large_text]), None, false), false).is_err());
}

#[tokio::test]
async fn cursor_remote_inline_full_decode_rejects_truncation_animation_and_dimensions() {
    let (_, png) = data_url(&url(image::ImageFormat::Png, 0)).unwrap();
    let mut animated = png.clone();
    animated.splice(8..8, [0, 0, 0, 0, b'a', b'c', b'T', b'L', 0, 0, 0, 0]);
    let wide = image::DynamicImage::ImageRgb8(image::RgbImage::new(4097, 1));
    let mut encoded = std::io::Cursor::new(Vec::new());
    wide.write_to(&mut encoded, image::ImageFormat::Png)
        .unwrap();
    for bytes in [png[..16].to_vec(), animated, encoded.into_inner()] {
        let url = format!("data:image/png;base64,{}", STANDARD.encode(bytes));
        let execution = parse(
            body(false, json!([message(&url, false)]), None, false),
            false,
        )
        .unwrap();
        assert!(execution.prepare_input(&mut json!({})).await.is_err());
    }
}

#[test]
fn cursor_remote_inline_typed_signature_binds_text_on_each_side_of_image() {
    let url = url(image::ImageFormat::Png, 0);
    let mut signatures = Vec::new();
    for (left, right) in [("A", "B"), ("AB", "")] {
        let history = json!([{"role":"user","content":[
            {"type":"text","text":left}, {"type":"image_url","image_url":{"url":url}},
            {"type":"text","text":right},
        ]}]);
        let Execution::Tools(request) =
            parse(body(true, history, Some("auto"), false), true).unwrap()
        else {
            panic!("tools");
        };
        assert_eq!(request.history[0]["content"], "AB");
        signatures.push(request.signature);
    }
    assert_ne!(signatures[0], signatures[1]);
}
