use super::*;

#[test]
fn extract_signaler_account_id_from_page_blob_reads_s06grb() {
    let blob = r#"<script>window.WIZ_global_data={"S06Grb":"111226070075366785521"};</script>"#;
    assert_eq!(
        extract_signaler_account_id_from_page_blob(blob).as_deref(),
        Some("111226070075366785521")
    );
}

#[test]
fn build_image_edit_signaler_bodies_match_captured_shape() {
    let choose_server = build_image_edit_signaler_choose_server_body("111226070075366785521");
    assert!(choose_server.contains("\"async_bard\""));
    assert!(choose_server.contains("\"111226070075366785521\""));

    let open_channel = build_image_edit_signaler_open_channel_body("111226070075366785521");
    assert!(open_channel.contains("count=4"));
    assert!(open_channel.contains("req0___data__"));
    assert!(open_channel.contains("async_bard"));
    assert!(open_channel.contains("beyond_agency_bard"));
    assert!(open_channel.contains("mini_app_generation_bard"));
    assert!(open_channel.contains("bard-client-sync"));
    assert!(open_channel.contains("111226070075366785521"));

    let refresh_creds = build_image_edit_signaler_refresh_creds_body("FoFSUpYY");
    assert_eq!(refresh_creds, "[\"FoFSUpYY\"]");
}

#[test]
fn parse_signaler_responses_extract_handshake_and_aid() {
    let choose_body =
        r#"["nELF6cLDQmZXFcvDxW0F45qqGXrhMmLfx6qHkU_Mgkg",3,null,"1777607372305639"]"#;
    assert_eq!(
        parse_signaler_choose_server_response(choose_body).unwrap(),
        "nELF6cLDQmZXFcvDxW0F45qqGXrhMmLfx6qHkU_Mgkg"
    );

    let open_body = "51\n[[0,[\"c\",\"C5GZ7BOxKKfdABZvW9wVmg\",\"\",8,14,30000]]]\n";
    assert_eq!(
        parse_signaler_open_channel_sid(open_body).unwrap(),
        "C5GZ7BOxKKfdABZvW9wVmg"
    );

    let long_poll = concat!(
        "188\n",
        "[[1,[[null,null,[\"d5ty4AOG\"]]]],[2,[[[[\"1\",[[\"1777607372727478\"]]]]]]]]",
        "167\n",
        "[[18,[[[[\"4\",[null,null,[\"1777607613519203\"]]],",
        "[\"3\",[null,null,[\"1777607613519203\"]]]]]]]]"
    );
    assert_eq!(
        extract_signaler_long_poll_refresh_token(long_poll).as_deref(),
        Some("d5ty4AOG")
    );
    assert_eq!(extract_signaler_long_poll_max_aid(long_poll), Some(18));

    let shifted_refresh_long_poll = concat!(
            "187\n",
            "[[1,[[[[\"4\",null,[[16,\"generic\"]]]]]]],[2,[[null,null,[\"UTRNw8fZ\"]]]],[3,[[[[\"1\",[[\"1777678505284506\"]]]]]]]"
        );
    assert_eq!(
        extract_signaler_long_poll_refresh_token(shifted_refresh_long_poll).as_deref(),
        Some("UTRNw8fZ")
    );
}

#[test]
fn extract_signaler_app_paths_recovers_conversation_targets() {
    let body = r#"c_90ca192406be80bb /app/263d92f7707033f8 c_90ca192406be80bb"#;
    assert_eq!(
        extract_signaler_app_paths(body),
        vec![
            "/app/90ca192406be80bb".to_string(),
            "/app/263d92f7707033f8".to_string()
        ]
    );
}

#[test]
fn extract_signaler_app_paths_recovers_decimal_conversation_ids() {
    let body = r#"129 [[21,[[[["3",[null,null,["1777712281763442"]]],["1",[null,null,["1777712281763442"]]]]]]]]"#;
    assert_eq!(
        extract_signaler_app_paths(body),
        vec!["/app/1777712281763442".to_string()]
    );
}
