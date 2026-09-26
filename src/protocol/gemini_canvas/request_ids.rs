pub fn new_text_stream_generate_request_uuid() -> String {
    new_stream_generate_request_uuid()
}

pub fn new_batchexecute_header_id() -> String {
    uuid::Uuid::new_v4().to_string().to_ascii_uppercase()
}

pub fn new_stream_generate_request_uuid() -> String {
    new_batchexecute_header_id()
}

pub(super) fn random_hex_32() -> String {
    uuid::Uuid::new_v4().simple().to_string()
}

pub(super) fn current_reqid() -> u64 {
    let millis = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;
    10_000 + (millis % 89_999)
}
