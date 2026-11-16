//! Generated tones prove PCM decoding; malformed fixtures must not resynchronize to success.
use super::*;
const STEREO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/audio/synthetic-tone.mp3"
));
const MONO: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/tests/fixtures/audio/synthetic-mono.mp3"
));
fn check(bytes: &[u8]) -> Result<(), Failure> {
    decode(
        bytes,
        &AtomicBool::new(false),
        tokio::time::Instant::now() + std::time::Duration::from_secs(10),
    )
}
#[test]
fn mp3_audio_decodes_generated_mpeg1_stereo_and_mpeg2_mono() {
    check(STEREO).unwrap();
    check(MONO).unwrap();
}
#[test]
fn mp3_audio_accepts_bounded_id3v2_and_id3v1_without_parsing_metadata() {
    let mut bytes = b"ID3\x04\0\x10\0\0\0\x03abc3DI\x04\0\x10\0\0\0\x03".to_vec();
    bytes.extend_from_slice(STEREO);
    bytes.extend_from_slice(b"TAG");
    bytes.extend_from_slice(&[0; 125]);
    check(&bytes).unwrap();
}
#[test]
fn mp3_audio_rejects_id3_only_junk_and_unsupported_container() {
    for bytes in [
        b"ID3\x04\0\0\0\0\0\0".as_slice(),
        b"RIFFxxxxWAVE",
        b"<html>audio</html>",
        b"",
    ] {
        assert_eq!(check(bytes), Err(Failure::Type));
    }
}
#[test]
fn mp3_audio_rejects_every_truncation_inside_last_frame() {
    let mut offset = 0;
    while offset + framing::frame_size(&STEREO[offset..]).unwrap() < STEREO.len() {
        offset += framing::frame_size(&STEREO[offset..]).unwrap();
    }
    for cut in offset + 1..STEREO.len() {
        assert_eq!(check(&STEREO[..cut]), Err(Failure::Type), "cut {cut}");
    }
}
#[test]
fn mp3_audio_rejects_trailing_html_and_corrupt_sync_without_skipping_bytes() {
    let mut bytes = STEREO.to_vec();
    bytes.extend_from_slice(b"<html>private</html>");
    assert_eq!(check(&bytes), Err(Failure::Type));
    let mut bytes = STEREO.to_vec();
    let next = framing::frame_size(&bytes).unwrap();
    bytes[next] = 0;
    assert_eq!(check(&bytes), Err(Failure::Type));
}
#[test]
fn mp3_audio_rejects_valid_frame_header_with_invalid_layer3_payload() {
    let size = framing::frame_size(STEREO).unwrap();
    let mut bytes = vec![0xff; size];
    bytes[..4].copy_from_slice(&STEREO[..4]);
    assert_eq!(
        check(&bytes),
        Err(Failure::Type),
        "header/frame length alone is not an audio proof"
    );
}
#[test]
fn mp3_audio_rejects_signal_change_and_free_format() {
    let mut bytes = STEREO.to_vec();
    bytes.extend_from_slice(MONO);
    assert_eq!(check(&bytes), Err(Failure::Type));
    bytes[..4].copy_from_slice(&STEREO[..4]);
    bytes[2] &= 0x0f;
    assert_eq!(check(&bytes), Err(Failure::Type));
}
#[test]
fn mp3_audio_rejects_oversized_invalid_and_truncated_id3() {
    assert_eq!(check(b"ID3\x04\0\0\x01\0\0\x01"), Err(Failure::Limit));
    for bytes in [
        b"ID3\x04\0\0\x80\0\0\0".as_slice(),
        b"ID3\x04\0\x10\0\0\0\0",
        b"ID3\x03\0\0\0\0\0\x02x",
    ] {
        assert_eq!(check(bytes), Err(Failure::Type));
    }
}
#[test]
fn mp3_audio_checks_original_deadline_and_cooperative_cancel_between_frames() {
    assert_eq!(
        decode(
            STEREO,
            &AtomicBool::new(true),
            tokio::time::Instant::now() + std::time::Duration::from_secs(1)
        ),
        Err(Failure::Timeout)
    );
    assert_eq!(
        decode(STEREO, &AtomicBool::new(false), tokio::time::Instant::now()),
        Err(Failure::Timeout)
    );
    let flag = Arc::new(AtomicBool::new(false));
    drop(Stop(flag.clone()));
    assert!(flag.load(Ordering::Relaxed));
}

#[test]
fn mp3_audio_rejects_decoder_concealment_of_missing_reservoir_history() {
    let size = framing::frame_size(STEREO).unwrap();
    let mut frame = STEREO[..size].to_vec();
    frame[4] = 0xff;
    frame[5] |= 0x80; // main_data_begin=511, but no earlier data exists.
    let mut decoder = symphonia::default::get_codecs()
        .make(
            CodecParameters::new().for_codec(CODEC_TYPE_MP3),
            &Default::default(),
        )
        .unwrap();
    assert!(
        decoder
            .decode(&Packet::new_from_slice(0, 0, 0, &frame))
            .is_ok(),
        "fixed dependency actively conceals this corruption, so decode Ok alone is insufficient"
    );
    assert_eq!(check(&frame), Err(Failure::Type));
    assert_eq!(
        check(&STEREO[size..]),
        Err(Failure::Type),
        "a complete-frame prefix cut still needs reservoir history"
    );
}

#[test]
fn mp3_audio_rejects_missing_history_in_later_frame_not_only_first_frame() {
    let size = framing::frame_size(STEREO).unwrap();
    let mut bytes = STEREO.to_vec();
    bytes[size + 4] = 0xff;
    bytes[size + 5] |= 0x80;
    assert_eq!(check(&bytes), Err(Failure::Type));
    check(STEREO).unwrap();
}
