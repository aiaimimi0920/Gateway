use super::*;

#[test]
fn extract_page_blob_media_assets_recovers_encoded_music_download_url() {
    let body = r#"
            <script>
                window.__DATA__ = [
                    "https:\/\/contribution.usercontent.google.com\/download?c\u003dabc123\u0026filename\u003divory_rain.mp4\u0026opi\u003d103135050"
                ];
            </script>
        "#;

    let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Music)
        .expect("music asset should be recovered from page blob");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "video");
    assert_eq!(assets[0].mime_type, "video/mp4");
    assert_eq!(
            assets[0].url,
            "https://contribution.usercontent.google.com/download?c=abc123&filename=ivory_rain.mp4&opi=103135050"
        );
}

#[test]
fn extract_page_blob_media_assets_recovers_encoded_video_download_url() {
    let body = r#"
            <video
                src="https:\/\/contribution.usercontent.google.com\/download?c\u003dxyz789\u0026filename\u003dvideo.mp4\u0026opi\u003d103135050">
            </video>
        "#;

    let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Video)
        .expect("video asset should be recovered from page blob");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "video");
    assert_eq!(assets[0].mime_type, "video/mp4");
    assert_eq!(
            assets[0].url,
            "https://contribution.usercontent.google.com/download?c=xyz789&filename=video.mp4&opi=103135050"
        );
}

#[test]
fn extract_page_blob_media_assets_recovers_work_fife_music_download_url() {
    let body = r#"
            <audio
                src="https:\/\/work.fife.usercontent.google.com\/rd-gg-dl\/abc123\/ivory_rain.mp4">
            </audio>
        "#;

    let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Music)
        .expect("music asset should be recovered from work.fife page blob");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "video");
    assert_eq!(assets[0].mime_type, "video/mp4");
    assert_eq!(
        assets[0].url,
        "https://work.fife.usercontent.google.com/rd-gg-dl/abc123/ivory_rain.mp4"
    );
}

#[test]
fn extract_page_blob_media_assets_prefers_audio_for_music_when_both_audio_and_video_exist() {
    let body = r#"
            <script>
                window.__DATA__ = [
                    "https:\/\/contribution.usercontent.google.com\/download?c\u003dabc123\u0026filename\u003divory_rain.mp3\u0026opi\u003d103135050",
                    "https:\/\/contribution.usercontent.google.com\/download?c\u003dabc123\u0026filename\u003divory_rain.mp4\u0026opi\u003d103135050"
                ];
            </script>
        "#;

    let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Music)
        .expect("music assets should be recovered from page blob");
    assert_eq!(assets.len(), 2);
    assert_eq!(assets[0].kind, "audio");
    assert_eq!(assets[0].mime_type, "audio/mpeg");
    assert_eq!(
            assets[0].url,
            "https://contribution.usercontent.google.com/download?c=abc123&filename=ivory_rain.mp3&opi=103135050"
        );
}

#[test]
fn extract_page_blob_media_assets_recovers_lh3_video_download_url() {
    let body = r#"
            <script>
                window.__VIDEO__ = [
                    "https:\/\/lh3.googleusercontent.com\/gg-dl\/ABCDEF12345\/video.mp4"
                ];
            </script>
        "#;

    let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Video)
        .expect("video asset should be recovered from lh3 page blob");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "video");
    assert_eq!(assets[0].mime_type, "video/mp4");
    assert_eq!(
        assets[0].url,
        "https://lh3.googleusercontent.com/gg-dl/ABCDEF12345/video.mp4"
    );
}

#[test]
fn extract_page_blob_media_assets_recovers_lh3_rd_ogw_image_url() {
    let body = r#"
            <script>
                window.__IMAGE__ = [
                    "https:\/\/lh3.googleusercontent.com\/rd-ogw\/AF2bZyjVGJ57ttVmI02_SfxKBzom-eSodUpyh2VRJQ9_3HcIhy71DYPyaoB9KrzT8c7l6W4Nw7l7WoKSqnhWlz6RW_VxXyD9WkpkpFmsyva75EH7PKTzGfZIktNW9KbmuqfhEjms3V6AGnWtM4JwmqsPDUzVGGlU22X8hTZSF281orL5hE37kQbf0Jj18yRBse44MCPoJnOuumXyn-ONdceYKQ20Xu52sai3RMnCRPAKYCOlExLjXSJIYTwqmp6ALGdqZNUejShtuXwYsdZspE0G0dkTPyHa2lRBKkVi406x8hjf6_VT40NJ1NaaEpthLCHLTODvp4JfDRXpIFxo_T7JtI6DvfFVSOGoD4NjAizP8u53rgyfNv48b2eXegfMtFi1tRdrEWruepcRE7IqJc383vJ9BOKeLGuB5u0HOdX6_ktB14HY-fVF7Z3vZ8h0wJkgTUXQeEVsYbjTgmt2VIVVkCYy-Wnvbfj9hs7Styyi9mKRtwVmoUtSJsPy9kXrEJAES4Ml1xao4fAY0D1h4JZxqpaaKu65JVGBJpv5xbZ-sqyzrpN09lcnST0aJJPGHYiMMpQYyJySwHZfI2VYPg3P38O1aLhByDMEscyVmxO-VmaPu83_b-FrJem8I63KX6B6cRmmC6-Co56V59kHzJdrPZZR3Xl7QO1qweg576sjaerh73eQjLawbadzn7umSjeZVSp7NFi0ps1daiHafyqH3YCtmEN1gdUDOwATzF088177AG2btc6urscn3sytGl6MPCV4AYec5eDAwYwWzrc-IhSCS1sEP0sQ5PSkqooG0Gft1vsNuyp9kGBT7717M-dLJmyqPHXn8XMujltusg8VhLPX6-FgALkR1EMAVlOZ_HdrytoNHJVmry8MVPb7Etl9OnR5ilqDcyiXYCYMwUatxqSlmjvH9yYLX6c91nY1Jshder2Q86_qb2M5r5uIRAGL85iGSWdGRLQJ14fOykvC0Jwt7J5G4rZTn8_0icvOe8b3itux0nB9up8PnZ1Nm8ne0yrCe8h2SivIDnc55DMcZxjoqAnLt2rDlnStlG2jyPI=s32-c"
                ];
            </script>
        "#;

    let assets = extract_page_blob_media_assets(body, GeminiCanvasMediaOperation::Image)
        .expect("image asset should be recovered from lh3 rd-ogw page blob");
    assert_eq!(assets.len(), 1);
    assert_eq!(assets[0].kind, "image");
    assert_eq!(assets[0].mime_type, "image/png");
    assert!(assets[0]
        .url
        .starts_with("https://lh3.googleusercontent.com/rd-ogw/"));
}
