//! Invalid local output contracts must fail before a potentially charged submission.
use super::*;
use serde_json::json;

#[test]
fn luma_web_red_image_output_contract_is_admitted_before_generation() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
    for patch in [
        json!({"response_format":null}),
        json!({"response_format":"private-invalid"}),
        json!({"n":0}),
        json!({"n":-1}),
        json!({"n":1.5}),
        json!({"n":"1"}),
        json!({"n":null}),
        json!({"output_format":"webp"}),
        json!({"format":"gif"}),
        json!({"output_format":null}),
        json!({"output_format":""}),
        json!({"images":"private-input"}),
        json!({"images":null}),
    ] {
        let mut raw = json!({"prompt":"fixture"});
        raw.as_object_mut()
            .unwrap()
            .extend(patch.as_object().unwrap().clone());
        let request = image_generation_request(raw);
        let error =
            prepare_lumalabs_media_plan(&payload, &request, "uni-1", Duration::from_secs(1))
                .expect_err("unsupported local output/input must not consume remote credits");
        assert_eq!(error.http_status, Some(400));
        assert!(!error.message.contains("private-"));
    }
}

#[test]
fn luma_web_image_output_contract_keeps_supported_formats_and_empty_input_compatibility() {
    let payload = make_payload("lumalabs_compatible", "https://app.lumalabs.ai");
    for format in ["png", "jpeg"] {
        for response in ["url", "b64_json"] {
            let request = image_generation_request(json!({"prompt":"fixture","n":1,
                "images":[],"output_format":format,"response_format":response}));
            let plan =
                prepare_lumalabs_media_plan(&payload, &request, "uni-1", Duration::from_secs(1))
                    .unwrap();
            assert_eq!(plan.action_body["fields"]["output_format"], format);
        }
    }
}
