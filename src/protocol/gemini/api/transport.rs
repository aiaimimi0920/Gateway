pub fn default_path(model: &str, stream: bool) -> String {
    let suffix = if stream {
        "streamGenerateContent"
    } else {
        "generateContent"
    };
    format!("/models/{model}:{suffix}")
}

pub fn default_query(stream: bool) -> Vec<(String, String)> {
    if stream {
        vec![("alt".to_string(), "sse".to_string())]
    } else {
        Vec::new()
    }
}
