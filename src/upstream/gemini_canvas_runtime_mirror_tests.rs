use super::*;

#[test]
fn runtime_mirror_rejects_unsafe_object_keys_before_filesystem_access() {
    for key in [
        "../outside.json",
        "nested/../../outside.json",
        "C:/outside.json",
        "C:outside.json",
        "//server/share/outside.json",
        "/outside.json",
        "nested/file.json:stream",
        "nested/NUL.json",
        "nested/CON.json",
        "nested/../outside.json",
        "nested//file.json",
        "nested/./file.json",
        "nested/file.json.",
        "nested/\0file.json",
    ] {
        assert!(
            gemini_canvas_runtime_mirror_json_path(key).is_none(),
            "{key:?}"
        );
    }
}
