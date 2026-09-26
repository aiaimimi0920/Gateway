use super::{
    build_clips_library_url, build_conversation_url, build_image_generation_url,
    build_message_stream_url, build_session_url, build_video_detail_path, build_video_library_path,
    build_video_status_path, build_video_status_url,
};

#[test]
fn ordinary_identifiers_and_surrounding_whitespace_keep_the_existing_wire_shape() {
    assert_eq!(
        build_message_stream_url("https://producer.ai/", " job-123_abc.v1 "),
        "https://producer.ai/__api/messages/job-123_abc.v1/stream"
    );
    assert_eq!(
        build_session_url("https://producer.ai/", " conv-123_abc.v1 "),
        "https://producer.ai/session/conv-123_abc.v1"
    );
}

#[test]
fn message_stream_identifier_cannot_escape_into_path_query_or_fragment() {
    assert_eq!(
        build_message_stream_url("https://producer.ai", " ../job?x=1#part%2F "),
        "https://producer.ai/__api/messages/..%2Fjob%3Fx%3D1%23part%252F/stream"
    );
}

#[test]
fn session_identifier_encodes_unicode_spaces_backslashes_and_controls() {
    assert_eq!(
        build_session_url("https://producer.ai", "会 话\\id\r\nx"),
        "https://producer.ai/session/%E4%BC%9A%20%E8%AF%9D%5Cid%0D%0Ax"
    );
}

#[test]
fn reserved_delimiters_are_encoded_while_unreserved_characters_stay_stable() {
    assert_eq!(
        build_session_url("https://producer.ai", "job!$&'()*+,:;=@id-._~"),
        concat!(
            "https://producer.ai/session/",
            "job%21%24%26%27%28%29%2A%2B%2C%3A%3B%3D%40id-._~"
        )
    );
}

#[test]
fn exact_dot_segments_are_not_emitted_as_navigation_segments() {
    assert_eq!(
        build_video_status_path("."),
        "/__api/music-video/%2E/status"
    );
    assert_eq!(
        build_video_detail_path(".."),
        "/__api/music-video/get/%2E%2E"
    );
}

#[test]
fn video_status_url_uses_the_same_single_segment_encoding() {
    assert_eq!(
        build_video_status_url("https://producer.ai/", "job/next?x#y%2F"),
        "https://producer.ai/__api/music-video/job%2Fnext%3Fx%23y%252F/status"
    );
}

#[test]
fn build_conversation_url_trims_trailing_slash() {
    assert_eq!(
        build_conversation_url("https://producer.ai/"),
        "https://producer.ai/__api/conversation"
    );
    assert_eq!(
        build_conversation_url("https://producer.ai"),
        "https://producer.ai/__api/conversation"
    );
}

#[test]
fn build_image_generation_url_trims_trailing_slash() {
    assert_eq!(
        build_image_generation_url("https://producer.ai/"),
        "https://producer.ai/__api/generate/image"
    );
    assert_eq!(
        build_image_generation_url("https://producer.ai"),
        "https://producer.ai/__api/generate/image"
    );
}

#[test]
fn build_clips_library_url_trims_trailing_slash() {
    assert_eq!(
        build_clips_library_url("https://www.flowmusic.app/"),
        "https://www.flowmusic.app/__api/clips/auth-user"
    );
}

#[test]
fn build_message_stream_url_uses_job_id_path() {
    assert_eq!(
        build_message_stream_url("https://producer.ai/", "job-123"),
        "https://producer.ai/__api/messages/job-123/stream"
    );
}

#[test]
fn build_session_url_uses_conversation_id_path() {
    assert_eq!(
        build_session_url("https://producer.ai/", "conv-123"),
        "https://producer.ai/session/conv-123"
    );
    assert_eq!(
        build_session_url("https://producer.ai", "conv-123"),
        "https://producer.ai/session/conv-123"
    );
}

#[test]
fn build_video_status_url_uses_job_id_path() {
    assert_eq!(
        build_video_status_url("https://producer.ai/", "job-123"),
        "https://producer.ai/__api/music-video/job-123/status"
    );
}

#[test]
fn build_video_status_path_uses_job_id_path() {
    assert_eq!(
        build_video_status_path("job-123"),
        "/__api/music-video/job-123/status"
    );
}

#[test]
fn build_video_detail_path_uses_job_id_path() {
    assert_eq!(
        build_video_detail_path("job-123"),
        "/__api/music-video/get/job-123"
    );
}

#[test]
fn build_video_library_path_matches_canonical_route() {
    assert_eq!(build_video_library_path(), "/library/videos");
}
