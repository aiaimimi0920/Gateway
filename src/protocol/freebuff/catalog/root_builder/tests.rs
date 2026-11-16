use super::*;
use crate::protocol::freebuff::catalog::{
    tests::body, BusinessOrigin, CatalogSession, ClientSurface, ModelCatalog,
};
use serde_json::json;
#[path = "merge_tests.rs"]
mod merge;
#[path = "pairing_tests.rs"]
mod pairing;

fn context() -> RootHostContext<'static> {
    RootHostContext {
        current_date: "November 7, 2026",
        knowledge_files: "host-knowledge {CODEBUFF_SYSTEM_INFO_PROMPT}",
        git_changes: "host-git",
        system_info: "host-system",
    }
}
fn tools() -> Vec<Value> {
    [
        "read_files",
        "str_replace",
        "write_file",
        "run_terminal_command",
        "code_search",
        "glob",
        "list_directory",
        "write_todos",
        "web_search",
        "read_url",
        "ask_user",
        "suggest_followups",
        "gravity_index",
        "render_ui",
        "skill",
    ]
    .into_iter()
    .map(|name| {
        json!({"type":"function","function":{
            "name":name,"description":"Synthetic host tool; not locally executed",
            "parameters":{"type":"object","properties":{}}}})
    })
    .collect()
}
pub(in crate::protocol::freebuff::catalog) fn input() -> RootStep<'static> {
    RootStep {
        context: context(),
        messages: vec![json!({"role":"user","content":"host-user"})],
        tools: tools(),
        stream: false,
    }
}
fn built(digest: &str, step: RootStep<'_>) -> Result<Value, BusinessError> {
    built_limit(digest, step, 32768)
}
fn built_limit(digest: &str, step: RootStep<'_>, limit: usize) -> Result<Value, BusinessError> {
    let mut owner =
        CatalogSession::new(BusinessOrigin::Codebuff, "synthetic", ClientSurface::Cli).unwrap();
    let mut catalog = body();
    catalog["rows"][0]["legacyDigests"] = json!([digest]);
    owner.catalog = Some(ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap());
    let (private, public) = crate::protocol::freebuff::catalog::tests::pair();
    let device = super::super::DeviceRegistration::new(
        &owner.binding,
        std::sync::Arc::new(super::super::DeviceKey::from_pkcs8(&private, &public).unwrap()),
        "synthetic".into(),
    );
    owner
        .business_requests(&device, "open")
        .unwrap()
        .build_root_step(step, limit)
        .map(|(body, _)| body)
}
#[test]
fn freebuff_root_builder_full_system_is_first_and_host_text_is_literal() {
    let body = built("unknown", input()).unwrap();
    let system = body["messages"][0]["content"].as_str().unwrap();
    assert_eq!(body["messages"][0]["role"], "system");
    assert!(system.starts_with("You are Buffy, the coding agent behind Codebuff."));
    for expected in [
        "November 7, 2026",
        "host-knowledge {CODEBUFF_SYSTEM_INFO_PROMPT}",
        "host-git",
        "host-system",
        "# Working with the user",
        "# Freebuff Meta-information",
        "Do not skip tests",
        "You are running on the open model.",
    ] {
        assert!(system.contains(expected), "missing {expected}");
    }
    assert_eq!(body["provider"], json!({"data_collection":"deny"}));
    assert_eq!(body["model"], "fbm1.synthetic-open");
}
#[test]
fn freebuff_root_builder_turn_note_follows_every_user_not_tool_history() {
    let mut step = input();
    step.messages = vec![
        json!({"role":"user","content":"first"}),
        json!({"role":"assistant","content":"working"}),
        json!({"role":"tool","tool_call_id":"call","content":"result"}),
        json!({"role":"user","content":"second"}),
    ];
    let value = built("957291675c9f9132", step).unwrap();
    let messages = value["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 7);
    for index in [2, 6] {
        assert_eq!(messages[index]["role"], "user");
        assert!(messages[index]["content"]
            .as_str()
            .unwrap()
            .starts_with("<system>Do not reply"));
    }
    assert_eq!(messages[4]["role"], "tool");
}
#[test]
fn freebuff_root_builder_batch_reminder_is_wrapped_and_only_in_this_snapshot() {
    let value = built("1e303ac563a6f9cc", input()).unwrap();
    let messages = value["messages"].as_array().unwrap();
    assert_eq!(messages.len(), 3);
    assert!(messages[0]["content"]
        .as_str()
        .unwrap()
        .contains("## Work in batches"));
    assert!(messages.last().unwrap()["content"]
        .as_str()
        .unwrap()
        .starts_with("<system_reminder>Work in batches:"));
    assert_eq!(built("1e303ac563a6f9cc", input()).unwrap(), value);
}
#[test]
fn freebuff_root_builder_rejects_a_prompt_toolset_without_host_capabilities() {
    let mut step = input();
    step.tools.pop();
    assert_eq!(
        built("unknown", step).err(),
        Some(BusinessError::InvalidInput)
    );
}
#[test]
fn freebuff_root_builder_complete_prompt_matches_fixed_source_literal_vectors() {
    use sha2::{Digest, Sha256};
    let policy = super::super::RootCompactionPolicy {
        cache_expiry_ms: 3600000,
        cache_expiry_min_tokens: 40000,
        max_context_length: Some(110000),
        lean: Some(true),
    };
    // Independently reconstructed from fixed TS literals, not a hash of this Rust implementation.
    for (batching, lean, bytes, expected) in [
        (
            false,
            false,
            9143,
            "f1b4beadd0d495b5a8e935505096e358847b6761645b8bc6ef171148e6ab4eb9",
        ),
        (
            true,
            false,
            10763,
            "faa805a1900df43ef6e19afa8cf1627182097d11c459e0a2650d22b45c8dee63",
        ),
        (
            false,
            true,
            11125,
            "d06e5dee332ede4d5e0e385324a0e3c244fa8f47ee27cbb3fc0ef48917494c65",
        ),
    ] {
        let model = super::super::RootModelInput {
            model: "fbm1.synthetic-open",
            model_label: "open",
            compaction: lean.then_some(&policy),
            compiled_model_id: None,
            turn_ending_note: false,
            batching,
        };
        let text = prompt::system(&model, &context());
        assert_eq!(text.len(), bytes);
        assert_eq!(format!("{:x}", Sha256::digest(text.as_bytes())), expected);
    }
}
#[test]
fn freebuff_root_builder_lean_requires_context_for_guidance_and_does_not_invent_a_budget() {
    let mut catalog = body();
    catalog["rows"][0]["legacyDigests"] = json!(["1e303ac563a6f9cc"]);
    catalog["rows"][0]["compaction"] =
        json!({"cacheExpiryMs":3600000,"cacheExpiryMinTokens":40000,"lean":true});
    let parsed = ModelCatalog::parse(&serde_json::to_vec(&catalog).unwrap()).unwrap();
    let input = parsed.select("open").unwrap().root_input();
    assert!(!input.batching);
    let text = prompt::system(&input, &context());
    assert!(!text.contains("# Context budget"));
    assert!(!text.contains("## Work in batches"));
    for (max, expected) in [(1, "~0k"), (1874, "~1k"), (1875, "~2k"), (110000, "~88k")] {
        assert!(guidance::lean(max).contains(expected));
    }
}
#[test]
fn freebuff_root_builder_preserves_native_reasoning_images_tools_and_stream_choice() {
    let mut step = input();
    step.stream = true;
    let user = json!({"role":"user","content":[{"type":"image_url","image_url":{"url":"https://example.invalid/image"}}]});
    let assistant = json!({"role":"assistant","content":null,"reasoning":"keep",
        "tool_calls":[{"id":"call","type":"function","function":{"name":"read_files","arguments":"{}"}}]});
    let tool = json!({"role":"tool","tool_call_id":"call","content":"result"});
    step.messages = vec![user.clone(), assistant.clone(), tool.clone()];
    let schemas = step.tools.clone();
    let value = built("unknown", step).unwrap();
    assert_eq!(value["messages"][1], user);
    assert_eq!(value["messages"][2], assistant);
    assert_eq!(value["messages"][3], tool);
    assert_eq!(value["tools"], json!(schemas));
    assert_eq!(value["stream"], true);
}
#[test]
fn freebuff_root_builder_invalid_context_history_and_tool_identity_are_rejected() {
    for phase in 0..9 {
        let mut step = input();
        match phase {
            0 => step.context.current_date = "\n",
            1 => step.messages.clear(),
            2 => step.messages = vec![json!({"role":"system","content":"foreign"})],
            3 => step.messages = vec![json!({"role":"user"}); 4097],
            4 => step.tools[1] = step.tools[0].clone(),
            5 => step.tools[0]["function"]["name"] = json!("foreign"),
            6 => step.tools[0]["type"] = json!("foreign"),
            7 => step.tools[0]["function"]["parameters"] = json!(null),
            _ => step.tools.push(step.tools[0].clone()),
        }
        assert_eq!(
            built("unknown", step).err(),
            Some(BusinessError::InvalidInput),
            "{phase}"
        );
    }
    let large = "x".repeat(1024 * 1024 + 1);
    let mut step = input();
    step.context.knowledge_files = &large;
    assert_eq!(
        built("unknown", step).err(),
        Some(BusinessError::InvalidInput)
    );
}
#[test]
fn freebuff_root_builder_static_tokens_in_host_data_are_never_recursively_expanded() {
    let mut step = input();
    step.context.knowledge_files = "{{SYSTEM_INFO}} {{GIT}}";
    let value = built("unknown", step).unwrap();
    assert!(value["messages"][0]["content"]
        .as_str()
        .unwrap()
        .contains("{{SYSTEM_INFO}} {{GIT}}"));
}
