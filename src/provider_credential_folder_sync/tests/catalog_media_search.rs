//! Media and search catalog surface contracts.
use super::super::canonicalization::canonicalize_folder_surface_slug;
use super::super::classification::{
    derive_provider_family_slug, derive_provider_surface_slug, derive_service_provider_slug,
};
use super::super::layout::default_folder_sync_relative_path;
use super::{build_test_credential, build_test_provider_account};

#[test]
fn derive_provider_surface_slug_maps_media_platform_lines() {
    let mut suno = build_test_provider_account(
        "Suno Music",
        "suno_compatible",
        "suno_music",
        "suno",
        "https://suno.com",
        Some("web_reverse_api"),
        Some("browser_challenge"),
    );
    suno.service_provider_key = "suno_platform".to_string();
    suno.service_provider_label = "Suno Platform".to_string();
    assert_eq!(derive_provider_family_slug(&suno), "suno");
    assert_eq!(derive_service_provider_slug(&suno), "suno-platform");
    assert_eq!(derive_provider_surface_slug(&suno), "suno");
    assert_eq!(canonicalize_folder_surface_slug("suno-music"), "suno");
    assert_eq!(
        default_folder_sync_relative_path(
            &suno,
            &build_test_credential("cred-suno", "folder_sync", None, "active", None),
        ),
        "suno-platform/suno/session-auth/cred-suno.json"
    );

    let mut udio = build_test_provider_account(
        "Udio Images",
        "udio_compatible",
        "udio_images",
        "udio",
        "https://www.udio.com",
        Some("web_reverse_api"),
        Some("browser_challenge"),
    );
    udio.service_provider_key = "udio_platform".to_string();
    udio.service_provider_label = "Udio Platform".to_string();
    assert_eq!(derive_provider_family_slug(&udio), "udio");
    assert_eq!(derive_service_provider_slug(&udio), "udio-platform");
    assert_eq!(derive_provider_surface_slug(&udio), "udio");
    assert_eq!(canonicalize_folder_surface_slug("udio-videos"), "udio");
    assert_eq!(
        default_folder_sync_relative_path(
            &udio,
            &build_test_credential("cred-udio", "folder_sync", None, "active", None),
        ),
        "udio-platform/udio/session-auth/cred-udio.json"
    );

    let mut lumalabs = build_test_provider_account(
        "LumaLabs Videos",
        "lumalabs_compatible",
        "lumalabs_videos",
        "lumalabs",
        "https://app.lumalabs.ai",
        Some("web_reverse_api"),
        Some("browser_challenge"),
    );
    lumalabs.service_provider_key = "lumalabs_platform".to_string();
    lumalabs.service_provider_label = "LumaLabs Platform".to_string();
    assert_eq!(derive_provider_family_slug(&lumalabs), "lumalabs");
    assert_eq!(derive_service_provider_slug(&lumalabs), "lumalabs-platform");
    assert_eq!(derive_provider_surface_slug(&lumalabs), "lumalabs");
    assert_eq!(
        canonicalize_folder_surface_slug("luma-labs-videos"),
        "lumalabs"
    );
    assert_eq!(
        default_folder_sync_relative_path(
            &lumalabs,
            &build_test_credential("cred-luma", "folder_sync", None, "active", None),
        ),
        "lumalabs-platform/lumalabs/session-auth/cred-luma.json"
    );
}

#[test]
fn derive_provider_surface_slug_maps_search_family_lines() {
    let mut perplexity = build_test_provider_account(
        "Perplexity Search",
        "search_api_compatible",
        "perplexity_search",
        "perplexity_search",
        "https://api.perplexity.ai",
        Some("official_vendor_api"),
        None,
    );
    perplexity.service_provider_key = "perplexity_platform".to_string();
    perplexity.service_provider_label = "Perplexity Search".to_string();
    assert_eq!(
        derive_provider_surface_slug(&perplexity),
        "perplexity-search"
    );

    let mut tavily = build_test_provider_account(
        "Tavily Search",
        "search_api_compatible",
        "tavily_search",
        "tavily",
        "https://api.tavily.com",
        Some("official_vendor_api"),
        None,
    );
    tavily.service_provider_key = "tavily_platform".to_string();
    tavily.service_provider_label = "Tavily Search".to_string();
    assert_eq!(derive_provider_surface_slug(&tavily), "tavily-search");

    let mut exa = build_test_provider_account(
        "Exa Search",
        "search_api_compatible",
        "exa_search",
        "exa",
        "https://api.exa.ai",
        Some("official_vendor_api"),
        None,
    );
    exa.service_provider_key = "exa_platform".to_string();
    exa.service_provider_label = "Exa Search".to_string();
    assert_eq!(derive_provider_surface_slug(&exa), "exa-search");

    let mut jina_search = build_test_provider_account(
        "Jina Search",
        "search_api_compatible",
        "jina_search",
        "jina_search",
        "https://s.jina.ai",
        Some("official_vendor_api"),
        None,
    );
    jina_search.service_provider_key = "jina_platform".to_string();
    jina_search.service_provider_label = "Jina Search".to_string();
    assert_eq!(derive_provider_surface_slug(&jina_search), "jina-search");

    let mut jina_reader = build_test_provider_account(
        "Jina Reader",
        "search_api_compatible",
        "jina_reader",
        "jina_reader",
        "https://r.jina.ai",
        Some("official_vendor_api"),
        None,
    );
    jina_reader.service_provider_key = "jina_platform".to_string();
    jina_reader.service_provider_label = "Jina Reader".to_string();
    assert_eq!(derive_provider_surface_slug(&jina_reader), "jina-reader");

    let mut linkup = build_test_provider_account(
        "Linkup Search",
        "search_api_compatible",
        "linkup_search",
        "linkup",
        "https://api.linkup.so",
        Some("official_vendor_api"),
        None,
    );
    linkup.service_provider_key = "linkup_platform".to_string();
    linkup.service_provider_label = "Linkup Search".to_string();
    assert_eq!(derive_provider_surface_slug(&linkup), "linkup-search");

    let mut you = build_test_provider_account(
        "You.com Search",
        "search_api_compatible",
        "you_search",
        "you_search",
        "https://api.ydc-index.io",
        Some("official_vendor_api"),
        None,
    );
    you.service_provider_key = "you_platform".to_string();
    you.service_provider_label = "You.com Search".to_string();
    assert_eq!(derive_provider_surface_slug(&you), "you-search");

    let mut websearchapi = build_test_provider_account(
        "WebSearchAPI Search",
        "search_api_compatible",
        "websearchapi_search",
        "websearchapi",
        "https://api.websearchapi.ai",
        Some("official_vendor_api"),
        None,
    );
    websearchapi.service_provider_key = "websearchapi_platform".to_string();
    websearchapi.service_provider_label = "WebSearchAPI Search".to_string();
    assert_eq!(
        derive_provider_surface_slug(&websearchapi),
        "websearchapi-search"
    );
}
