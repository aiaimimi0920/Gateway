use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use std::fmt::{self, Display, Formatter};

const PATH_SEGMENT_ENCODE_SET: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'!')
    .add(b'"')
    .add(b'#')
    .add(b'$')
    .add(b'%')
    .add(b'&')
    .add(b'\'')
    .add(b'(')
    .add(b')')
    .add(b'*')
    .add(b'+')
    .add(b',')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');
const DOT_PATH_SEGMENT_ENCODE_SET: &AsciiSet = &PATH_SEGMENT_ENCODE_SET.add(b'.');

struct ProducerPathSegment<'a>(&'a str);

impl Display for ProducerPathSegment<'_> {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        let value = self.0.trim();
        let encode_set = if matches!(value, "." | "..") {
            DOT_PATH_SEGMENT_ENCODE_SET
        } else {
            PATH_SEGMENT_ENCODE_SET
        };
        Display::fmt(&utf8_percent_encode(value, encode_set), formatter)
    }
}

fn producer_path_segment(value: &str) -> ProducerPathSegment<'_> {
    ProducerPathSegment(value)
}

pub fn build_conversation_url(base_url: &str) -> String {
    format!("{}/__api/conversation", base_url.trim_end_matches('/'))
}

pub fn build_message_stream_url(base_url: &str, job_id: &str) -> String {
    format!(
        "{}/__api/messages/{}/stream",
        base_url.trim_end_matches('/'),
        producer_path_segment(job_id)
    )
}

pub fn build_session_url(base_url: &str, conversation_id: &str) -> String {
    format!(
        "{}/session/{}",
        base_url.trim_end_matches('/'),
        producer_path_segment(conversation_id)
    )
}

pub fn build_video_status_url(base_url: &str, job_id: &str) -> String {
    format!(
        "{}/__api/music-video/{}/status",
        base_url.trim_end_matches('/'),
        producer_path_segment(job_id)
    )
}

pub fn build_video_status_path(job_id: &str) -> String {
    format!(
        "/__api/music-video/{}/status",
        producer_path_segment(job_id)
    )
}

pub fn build_video_detail_path(job_id: &str) -> String {
    format!("/__api/music-video/get/{}", producer_path_segment(job_id))
}

pub fn build_image_generation_url(base_url: &str) -> String {
    format!("{}/__api/generate/image", base_url.trim_end_matches('/'))
}

pub fn build_clips_library_url(base_url: &str) -> String {
    format!("{}/__api/clips/auth-user", base_url.trim_end_matches('/'))
}

pub fn build_video_library_path() -> &'static str {
    "/library/videos"
}
