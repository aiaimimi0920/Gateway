use super::*;
use crate::credential_pool_storage::webdav::path::{href_key, namespace_urls};

fn namespace() -> Url {
    namespace_urls("https://dav.invalid/root", "credentials/provider", false)
        .unwrap()
        .0
}

fn response(href: &str, resource_type: &str, status: u16) -> String {
    format!("<d:response><d:href>{href}</d:href><d:propstat><d:prop><d:resourcetype>{resource_type}</d:resourcetype></d:prop><d:status>HTTP/1.1 {status} Result</d:status></d:propstat></d:response>")
}

fn document(body: &str) -> String {
    format!("<?xml version='1.0'?><d:multistatus xmlns:d='DAV:'>{body}</d:multistatus>")
}

#[test]
fn lists_only_successful_immediate_namespace_json_files() {
    let body = [
        response("/root/credentials/provider/a.json", "", 200),
        response(
            "https://dav.invalid/root/credentials/provider/b%20name.json",
            "",
            200,
        ),
        response("/root/credentials/provider/a.json", "", 200),
        response("/root/credentials/provider/", "<d:collection/>", 200),
        response(
            "/root/credentials/provider/folder.json",
            "<d:collection/>",
            200,
        ),
        response("/root/credentials/other/denied.json", "", 403),
        response("/root/credentials/provider/nested/a.json", "", 200),
        response("/root/credentials/other/a.json", "", 200),
        response(
            "https://other.invalid/root/credentials/provider/x.json",
            "",
            200,
        ),
    ]
    .concat();
    assert_eq!(
        list_keys(document(&body).as_bytes(), &namespace()).unwrap(),
        vec!["a.json", "b name.json"]
    );
}

#[test]
fn resource_failures_do_not_become_empty_or_partial_successful_lists() {
    for status in [403, 500] {
        let denied = response("/root/credentials/provider/denied.json", "", status);
        let allowed = response("/root/credentials/provider/a.json", "", 200);
        for body in [denied.clone(), format!("{allowed}{denied}")] {
            let error = list_keys(document(&body).as_bytes(), &namespace()).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!("WebDAV listing resource returned HTTP {status}")
            );
        }
        let direct = format!("<d:response><d:href>/root/credentials/provider/a.json</d:href><d:status>HTTP/1.1 {status} Failed</d:status></d:response>");
        assert!(list_keys(document(&direct).as_bytes(), &namespace()).is_err());
    }
}

#[test]
fn requested_collection_failures_are_errors_with_or_without_trailing_slash() {
    for href in [
        "/root/credentials/provider/",
        "/root/credentials/provider",
        "https://dav.invalid/root/credentials/provider/",
    ] {
        let denied = response(href, "<d:collection/>", 403);
        assert!(list_keys(document(&denied).as_bytes(), &namespace()).is_err());
        let direct = format!("<d:response><d:href>{href}</d:href><d:status>HTTP/1.1 403 Failed</d:status></d:response>");
        assert!(list_keys(document(&direct).as_bytes(), &namespace()).is_err());
    }
}

#[test]
fn failed_foreign_or_nested_resources_do_not_poison_scope() {
    let body = [
        response(
            "https://other.invalid/root/credentials/provider/a.json",
            "",
            403,
        ),
        response("/root/credentials/other/", "<d:collection/>", 403),
        response("/root/credentials/provider///", "<d:collection/>", 403),
        response("/root/credentials/provider/nested/a.json", "", 500),
        response("/root/credentials/provider/a.json", "", 200),
    ]
    .concat();
    assert_eq!(
        list_keys(document(&body).as_bytes(), &namespace()).unwrap(),
        vec!["a.json"]
    );
}

#[test]
fn one_failed_propstat_cannot_be_masked_by_another_successful_status() {
    let body = "<d:response><d:href>/root/credentials/provider/a.json</d:href><d:status>HTTP/1.1 200 OK</d:status><d:propstat><d:status>HTTP/1.1 200 OK</d:status></d:propstat><d:propstat><d:status>HTTP/1.1 403 Forbidden</d:status></d:propstat></d:response>";
    assert!(list_keys(document(body).as_bytes(), &namespace()).is_err());
}

#[test]
fn supports_default_namespaces_and_literal_entities() {
    let xml = "<multistatus xmlns='DAV:'><response><href>/root/credentials/provider/a&amp;b.json</href><status>HTTP/1.1 200 OK</status></response></multistatus>";
    assert_eq!(
        list_keys(xml.as_bytes(), &namespace()).unwrap(),
        vec!["a&b.json"]
    );
    let xml = document("<d:response><d:href><![CDATA[/root/credentials/provider/a.json]]></d:href><d:status>HTTP/1.1 200 OK</d:status></d:response>");
    assert_eq!(
        list_keys(xml.as_bytes(), &namespace()).unwrap(),
        vec!["a.json"]
    );
}

#[test]
fn ignores_namespace_spoofing() {
    let xml = document("<d:response><d:href xmlns:d='other'>/root/credentials/provider/a.json</d:href><d:status>HTTP/1.1 200 OK</d:status></d:response>");
    assert!(list_keys(xml.as_bytes(), &namespace()).unwrap().is_empty());
    assert!(list_keys(b"<multistatus/>", &namespace()).is_err());
}

#[test]
fn rejects_dtd_entities_malformed_and_deep_xml() {
    for xml in [
        "<!DOCTYPE d:multistatus SYSTEM 'https://secrets.invalid'><d:multistatus xmlns:d='DAV:'/>",
        "<!DOCTYPE d:multistatus [<!ENTITY x 'hello'>]><d:multistatus xmlns:d='DAV:'/>",
        "<d:multistatus xmlns:d='DAV:'><d:response></d:multistatus>",
        "<d:multistatus xmlns:d='DAV:'>",
        "<d:multistatus xmlns:d='DAV:' xmlns:d='DAV:'/>",
        "<d:multistatus xmlns:d='DAV:'>&unknown;</d:multistatus>",
        "<d:multistatus xmlns:d='DAV:'><?injected test?></d:multistatus>",
    ] {
        assert!(
            list_keys(xml.as_bytes(), &namespace()).is_err(),
            "accepted unsafe XML"
        );
    }
    let deep = document(&format!("{}{}", "<d:x>".repeat(32), "</d:x>".repeat(32)));
    assert!(list_keys(deep.as_bytes(), &namespace()).is_err());
    let duplicate = document("<d:response><d:href>/a</d:href><d:href>/b</d:href></d:response>");
    assert!(list_keys(duplicate.as_bytes(), &namespace()).is_err());
}

#[test]
fn bounds_xml_objects_entry_count_and_values() {
    assert!(list_keys(&vec![b' '; MAX_OBJECT_BYTES + 1], &namespace()).is_err());
    let oversized = document(&response(&"a".repeat(8193), "", 200));
    assert!(list_keys(oversized.as_bytes(), &namespace()).is_err());
    let body = response("/root/credentials/provider/a.json", "", 200).repeat(MAX_LIST_ENTRIES + 2);
    assert!(list_keys(document(&body).as_bytes(), &namespace()).is_err());
}

#[test]
fn rejects_href_traversal_double_encoding_and_origin_escape() {
    for href in [
        "../provider/a.json",
        "a.json",
        "//evil.invalid/a.json",
        "/root/credentials/provider/../provider/a.json",
        "/root/credentials/provider/%2e%2e/provider/a.json",
        "/root/credentials/provider/a%2fb.json",
        "/root/credentials/provider/a%5cb.json",
        "/root/credentials/provider/%252e%252e.json",
        "/root/credentials/provider/bad%zz.json",
        "/root/credentials/provider/a.json?token=secret",
        "/root/credentials/provider/a.json#fragment",
        "/root/credentials/provider/%00.json",
        "https://user:password@dav.invalid/root/credentials/provider/a.json",
        "https://@dav.invalid/root/credentials/provider/a.json",
        "http://dav.invalid/root/credentials/provider/a.json",
        "https://dav.invalid:444/root/credentials/provider/a.json",
        "/root/credentials/provider-other/a.json",
    ] {
        assert!(
            href_key(href, &namespace()).is_none(),
            "unsafe href accepted"
        );
    }
}

#[test]
fn endpoint_and_namespace_validation_precedes_url_normalization() {
    for endpoint in [
        "http://dav.invalid/",
        "ftp://dav.invalid/",
        "https://user:secret@dav.invalid/",
        "https://@dav.invalid/",
        " https://dav.invalid/",
        "https://dav.invalid/?secret=value",
        "https://dav.invalid/#fragment",
        "https://dav.invalid/root/../private",
        "https://dav.invalid/root/%2e%2e/private",
        "https://dav.invalid/root%2fprivate",
        "https://dav.invalid/root\\private",
    ] {
        assert!(namespace_urls(endpoint, "dir/provider", false).is_err());
    }
    for path in [
        "",
        "/dir",
        "dir/",
        "dir//provider",
        "dir/../provider",
        "dir/%2e%2e",
        "dir\\provider",
    ] {
        assert!(namespace_urls("https://dav.invalid/base", path, false).is_err());
    }
    let (url, collections) =
        namespace_urls("http://127.0.0.1:1234/base", "dir/provider", true).unwrap();
    assert_eq!(url.as_str(), "http://127.0.0.1:1234/base/dir/provider/");
    assert_eq!(collections.len(), 2);
    assert_eq!(collections[0].path(), "/base/dir/");
}
