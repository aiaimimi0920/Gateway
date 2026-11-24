//! Tiny full-decode RGBA fixtures; no real images, credentials or external I/O.
use super::{
    pipeline_fixture::IMAGE,
    test_support::{job, Reply, ASSET, ID},
};
use serde_json::{json, Value};
pub(super) fn input() -> Value {
    json!({"model":IMAGE,"type":"layering","prompt":"","response_format":"b64_json",
        "source":{"generation_id":ID},"layering":{"resolution":"2k"}})
}
pub(super) fn task(state: &str, count: usize) -> Value {
    let mut value = job(state);
    value["type"] = json!("layering");
    value["usage"] = json!({"credits_charged":0.75});
    if state == "completed" {
        value["output"]=json!((0..count).map(|index| {
            let last=index+1==count;let size=if last{2}else{1};
            json!({"type":"image","url":ASSET,"layer":{"index":index,"label":if last{"background"}else{"subject"},
                "description":"完整合成元素","alpha_hint":if last{"none"}else{"hard"},
                "bounds":{"x":0,"y":0,"width":size,"height":size,"canvas_width":2,"canvas_height":2},
                "contains_text":false,"text":null}})
        }).collect::<Vec<_>>());
    }
    value
}
pub(super) fn image(last: bool) -> Reply {
    let size = if last { 2 } else { 1 };
    let pixels = image::RgbaImage::from_pixel(
        size,
        size,
        image::Rgba([10, 20, 30, if last { 255 } else { 128 }]),
    );
    let mut out = std::io::Cursor::new(Vec::new());
    image::DynamicImage::ImageRgba8(pixels)
        .write_to(&mut out, image::ImageFormat::Png)
        .unwrap();
    Reply {
        status: 200,
        content_type: "image/png",
        bytes: out.into_inner(),
        hold: false,
    }
}
pub(super) fn replies(count: usize) -> Vec<Reply> {
    let mut replies = vec![
        Reply::json(201, task("queued", count)),
        Reply::json(200, task("completed", count)),
    ];
    replies.extend((0..count).map(|index| image(index + 1 == count)));
    replies
}
pub(super) fn text() -> Value {
    json!({"content":"SUMMER\nSALE","decorated":false,"font":{"category":"display sans","weight":"bold",
        "width":"condensed","italic":false,"families":["Oswald","Anton"],
        "match":{"family_id":"font_family:synthetic","face_id":"font_face:synthetic","family_name":"Example","weight":700,"style":"normal"}}})
}
