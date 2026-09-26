use super::*;

#[test]
fn build_generate_request_maps_seed_and_lyrics() {
    let req = make_request(json!({
        "prompt": "dreamy synthwave soundtrack",
        "seed": 42,
        "lyrics": "electric city lights"
    }));

    let body = build_generate_request(&req, UDIO_DEFAULT_MODEL).unwrap();
    assert_eq!(body["gen_params"]["prompt"], "dreamy synthwave soundtrack");
    assert_eq!(body["gen_params"]["seed"], 42);
    assert_eq!(body["gen_params"]["lyrics"], "electric city lights");
    assert_eq!(body["gen_params"]["lyrics_type"], "user");
    assert!(body.get("model").is_none());
}

#[test]
fn build_generate_request_preserves_conditioning_fields() {
    let req = make_request(json!({
        "prompt": "continue the chorus with brighter strings",
        "audio_conditioning_path": "https://cdn.example.com/song.mp3",
        "audio_conditioning_song_id": "song-123",
        "audio_conditioning_type": "continuation",
        "crop_start_time": 0.9,
        "samplerOptions": {
            "seed": 7
        }
    }));

    let body = build_generate_request(&req, UDIO_DEFAULT_MODEL).unwrap();
    assert_eq!(body["gen_params"]["seed"], 7);
    assert_eq!(
        body["gen_params"]["audio_conditioning_path"],
        "https://cdn.example.com/song.mp3"
    );
    assert_eq!(body["gen_params"]["audio_conditioning_song_id"], "song-123");
    assert_eq!(
        body["gen_params"]["audio_conditioning_type"],
        "continuation"
    );
    assert_eq!(body["gen_params"]["crop_start_time"], 0.9);
    assert_eq!(body["gen_params"]["config"]["mode"], "regular");
}

#[test]
fn build_generate_request_maps_n_to_num_songs() {
    let req = make_request(json!({
        "prompt": "double take chorus",
        "n": 2
    }));

    let body = build_generate_request(&req, UDIO_DEFAULT_MODEL).unwrap();
    assert_eq!(body["num_songs"], 2);
}

#[test]
fn build_generate_request_omits_num_songs_for_single_song_default() {
    let req = make_request(json!({
        "prompt": "single take chorus"
    }));

    let body = build_generate_request(&req, UDIO_DEFAULT_MODEL).unwrap();
    assert!(body.get("num_songs").is_none());

    let explicit_single = make_request(json!({
        "prompt": "single take chorus",
        "n": 1
    }));
    let explicit_single_body =
        build_generate_request(&explicit_single, UDIO_DEFAULT_MODEL).unwrap();
    assert!(explicit_single_body.get("num_songs").is_none());
}

#[test]
fn build_generate_request_preserves_captcha_token_aliases() {
    let direct = make_request(json!({
        "prompt": "challenge aware chorus",
        "captchaToken": "token-direct"
    }));
    let direct_body = build_generate_request(&direct, UDIO_DEFAULT_MODEL).unwrap();
    assert_eq!(direct_body["captchaToken"], "token-direct");

    let alias = make_request(json!({
        "prompt": "challenge aware chorus",
        "captcha_token": "token-alias"
    }));
    let alias_body = build_generate_request(&alias, UDIO_DEFAULT_MODEL).unwrap();
    assert_eq!(alias_body["captchaToken"], "token-alias");
}
