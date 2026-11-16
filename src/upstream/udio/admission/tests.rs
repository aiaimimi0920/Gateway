//! Raw metadata cases deliberately differ from the permissive legacy decoder.
use super::*;
use serde_json::json;

fn song(id: &str) -> Value {
    json!({"id":id,"status":"complete","song_path":"https://asset.invalid/not-audio"})
}
fn result(ids: Value, songs: Value, completed: bool) -> UdioBrowserWorkerSuccess {
    serde_json::from_value(json!({"trackIds":ids,"songs":songs,"completed":completed})).unwrap()
}
fn rejected(value: UdioBrowserWorkerSuccess, wait: bool) {
    let error = admit_songs(&value, UdioOutputKind::Music, wait).unwrap_err();
    assert_eq!(error.http_status, Some(502));
    assert_eq!(error.provider_name.as_deref(), Some("udio_compatible"));
    assert!(!error.retryable);
    assert!(matches!(
        error.fallback_hint,
        crate::error::FallbackHint::Abort { .. }
    ));
    assert!(!error.message.contains("private"));
}
#[test]
fn udio_admission_track_identity_is_bounded_and_not_repaired() {
    for ids in [
        json!([]),
        json!(["one", "one"]),
        json!([""]),
        json!([" one"]),
        json!(["one\n"]),
        json!(["中".repeat(43)]),
        json!((0..17).map(|n| n.to_string()).collect::<Vec<_>>()),
    ] {
        rejected(result(ids, json!([]), false), false);
    }
    let id = "x".repeat(128);
    assert_eq!(
        admit_songs(
            &result(json!([id]), json!([]), false),
            UdioOutputKind::Music,
            false
        )
        .unwrap()[0]
            .id
            .len(),
        128
    );
}
#[test]
fn udio_admission_never_filters_malformed_duplicate_or_foreign_songs() {
    for songs in [
        json!([null]),
        json!([[]]),
        json!([{}]),
        json!([{"id":1}]),
        json!([{"id":" one"}]),
        json!([song("one"), song("one")]),
        json!([song("private")]),
    ] {
        rejected(result(json!(["one"]), songs, false), false);
    }
}
#[test]
fn udio_admission_both_status_fields_are_strict_and_failure_wins() {
    for key in ["status", "state"] {
        for status in [
            Value::Null,
            json!(1),
            json!(""),
            json!(" complete"),
            json!("x".repeat(65)),
            json!("ERROR"),
            json!("failed"),
            json!("canceled"),
            json!("cancelled"),
        ] {
            let mut value = song("one");
            value["finished"] = json!(true);
            value[key] = status;
            rejected(result(json!(["one"]), json!([value]), true), false);
        }
    }
}
#[test]
fn udio_admission_requires_target_and_raw_terminal_evidence() {
    for raw in [
        json!({"id":"one","song_path":"https://asset.invalid/not-audio","finished":"true"}),
        json!({"id":"one","song_path":"https://asset.invalid/not-audio","readyToStream":true}),
        json!({"id":"one","status":"complete","video_url":"https://asset.invalid/not-video"}),
        json!({"id":"one","status":"complete","song_path":1,"audio_url":"https://asset.invalid/not-audio"}),
        json!({"id":"one","status":"complete","song_path":" ","audio_url":"https://asset.invalid/not-audio"}),
    ] {
        rejected(result(json!(["one"]), json!([raw]), true), true);
    }
}
#[test]
fn udio_admission_completed_flag_matches_the_whole_single_snapshot() {
    rejected(
        result(json!(["one", "two"]), json!([song("one")]), true),
        false,
    );
    rejected(result(json!(["one"]), json!([song("one")]), false), false);
    rejected(result(json!(["one"]), json!([]), false), true);
}
#[test]
fn udio_admission_no_wait_preserves_all_ids_order_and_pending_facts() {
    let value = result(
        json!(["one", "two"]),
        json!([{"id":"two","finished":"true",
        "song_path":1,"audio_url":"https://asset.invalid/not-audio"}]),
        false,
    );
    let songs = admit_songs(&value, UdioOutputKind::Music, false).unwrap();
    assert_eq!(
        songs.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
        ["one", "two"]
    );
    assert!(songs
        .iter()
        .all(|s| !s.finished && s.status == "pending" && s.audio_url.is_none()));
}
#[test]
fn udio_admission_all_targets_accept_only_their_own_assets_and_restore_order() {
    for (target, field) in [
        (UdioOutputKind::Music, "song_path"),
        (UdioOutputKind::Image, "coverArtUrl"),
        (UdioOutputKind::Video, "videoPath"),
    ] {
        let mut songs = json!([{"id":"two","finished":true},{"id":"one","state":"COMPLETED"}]);
        for song in songs.as_array_mut().unwrap() {
            song[field] = json!("https://asset.invalid/not-media");
        }
        let value = result(json!(["one", "two"]), songs, true);
        for wait in [true, false] {
            let admitted = admit_songs(&value, target, wait).unwrap();
            assert_eq!(
                admitted.iter().map(|s| s.id.as_str()).collect::<Vec<_>>(),
                ["one", "two"]
            );
        }
    }
}
#[test]
fn udio_admission_nullish_alias_priority_allows_null_but_not_shadowed_fallback() {
    let mut raw = song("one");
    raw["song_path"] = Value::Null;
    raw["audio_url"] = json!("https://asset.invalid/not-audio");
    assert!(admit_songs(
        &result(json!(["one"]), json!([raw]), true),
        UdioOutputKind::Music,
        true
    )
    .is_ok());
}
