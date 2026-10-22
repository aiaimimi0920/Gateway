//! Verify immutable upstream bytes, then apply small exact-count wiring patches in OUT_DIR.
//! No network access, global Cargo-cache mutation, or unchecked source-version fallback.
use sha2::{Digest, Sha256};
use std::{fs, path::Path};

fn patch(root: &Path, file: &str, old: &str, new: &str, count: usize) {
    let path = root.join(file);
    let source = fs::read_to_string(&path).expect("read pinned source");
    assert_eq!(source.matches(old).count(), count, "patch drift: {file}");
    fs::write(path, source.replace(old, new)).expect("write generated source");
}

fn main() {
    println!("cargo:rerun-if-changed=upstream/hpack-fixtures.tar.gz");
    for path in [
        "upstream/http2-0.5.20.crate",
        "compat/go_hpack.rs",
        "compat/tests.rs",
        "build.rs",
    ] {
        println!("cargo:rerun-if-changed={path}");
    }
    let archive = fs::read("upstream/http2-0.5.20.crate").expect("immutable upstream archive");
    assert_eq!(
        format!("{:x}", Sha256::digest(&archive)),
        "92d3114be2f413b2e491e686b93a28cda30c355cffc8d091a57f8be4b1342896"
    );
    let out = std::path::PathBuf::from(std::env::var_os("OUT_DIR").expect("Cargo OUT_DIR"));
    tar::Archive::new(flate2::read::GzDecoder::new(archive.as_slice()))
        .unpack(&out)
        .expect("unpack verified archive");
    let root = out.join("http2-0.5.20/src");
    let fixtures = fs::read("upstream/hpack-fixtures.tar.gz").expect("pinned upstream fixtures");
    assert_eq!(
        format!("{:x}", Sha256::digest(&fixtures)),
        "307c762f4594ca79d61ecec39b9c2d8cc792e246a7af0e09a180e163d7b4a252"
    );
    tar::Archive::new(flate2::read::GzDecoder::new(fixtures.as_slice()))
        .unpack(out.join("fixtures"))
        .expect("unpack verified fixtures");
    patch(
        &root,
        "hpack/test/fixture.rs",
        "env!(\"CARGO_MANIFEST_DIR\")",
        "env!(\"OUT_DIR\")",
        1,
    );
    let source = fs::read_to_string(root.join("lib.rs")).unwrap();
    let (_, body) = source
        .split_once("mod tracing;")
        .expect("upstream entrypoint");
    fs::write(root.join("lib_body.rs"), format!("mod tracing;{body}")).unwrap();

    patch(
        &root,
        "hpack/mod.rs",
        "mod encoder;",
        "mod encoder;\nmod go_hpack;",
        1,
    );
    patch(
        &root,
        "hpack/encoder.rs",
        "    size_update: Option<SizeUpdate>,",
        "    size_update: Option<SizeUpdate>,\n    go_peer_size: Option<usize>,",
        1,
    );
    patch(
        &root,
        "hpack/encoder.rs",
        "            size_update: None,",
        "            size_update: None,\n            go_peer_size: None,",
        1,
    );
    patch(&root, "hpack/encoder.rs", "    pub fn update_max_size(&mut self, val: usize) {",
        "    pub fn update_max_size(&mut self, val: usize) {\n        self.go_peer_size = Some(val);", 1);
    // Keep new encoder logic inside the existing module to reuse private table/int primitives.
    patch(
        &root,
        "hpack/encoder.rs",
        "impl Default for Encoder {",
        "include!(\"go_encoder.rs\");\n\nimpl Default for Encoder {",
        1,
    );
    patch(&root, "hpack/table.rs", "    pub fn index(&mut self, header: Header) -> Index {",
        "    pub fn index(&mut self, header: Header) -> Index { self.index_policy(header, false) }\n    pub fn index_go(&mut self, header: Header) -> Index { self.index_policy(header, true) }\n    fn index_policy(&mut self, header: Header, go: bool) -> Index {", 1);
    patch(
        &root,
        "hpack/table.rs",
        "if header.skip_value_index() {",
        "if !go && header.skip_value_index() {",
        1,
    );
    patch(
        &root,
        "hpack/table.rs",
        "if header.len() * 4 > self.max_size * 3 {",
        "if if go { header.len() > self.max_size } else { header.len() * 4 > self.max_size * 3 } {",
        1,
    );
    patch(
        &root,
        "frame/headers.rs",
        "pub struct Pseudo {",
        "pub struct Pseudo {\n    pub(crate) go_compat: bool,",
        1,
    );
    // Keep HeaderValue's log redaction; only Go's HPACK indexing policy ignores that flag.
    // Thread policy explicitly rather than introducing shared mutable encoder mode state.
    for (old, new, count) in [
        (
            "self.index_dynamic(header, statik)",
            "self.index_dynamic(header, statik, go)",
            1,
        ),
        (
            "fn index_dynamic(&mut self, header: Header, statik: Option<(usize, bool)>)",
            "fn index_dynamic(&mut self, header: Header, statik: Option<(usize, bool)>, go: bool)",
            1,
        ),
        (
            "self.index_vacant(header, hash, dist, probe, statik)",
            "self.index_vacant(header, hash, dist, probe, statik, go)",
            2,
        ),
        (
            "self.index_occupied(header, hash, pos.index, statik.map(|(n, _)| n))",
            "self.index_occupied(header, hash, pos.index, statik.map(|(n, _)| n), go)",
            1,
        ),
        (
            "        statik: Option<usize>,\n    ) -> Index",
            "        statik: Option<usize>,\n        go: bool,\n    ) -> Index",
            1,
        ),
        (
            "        statik: Option<(usize, bool)>,\n    ) -> Index",
            "        statik: Option<(usize, bool)>,\n        go: bool,\n    ) -> Index",
            1,
        ),
        (
            "!header.is_sensitive()",
            "(!header.is_sensitive() || go)",
            1,
        ),
        (
            "if header.is_sensitive() {",
            "if header.is_sensitive() && !go {",
            2,
        ),
    ] {
        patch(&root, "hpack/table.rs", old, new, count);
    }
    patch(
        &root,
        "frame/headers.rs",
        "            order: Default::default(),",
        "            order: Default::default(),\n            go_compat: false,",
        2,
    );
    patch(&root, "frame/headers.rs", "        let headers = Iter {\n            pseudo: Some(self.pseudo),",
        "        let go = self.pseudo.go_compat;\n        let headers = Iter {\n            pseudo: Some(self.pseudo),", 1);
    patch(&root, "frame/headers.rs", "        encoder.encode(headers, &mut hpack);",
        "        if go { encoder.encode_go(headers, &mut hpack); } else { encoder.encode(headers, &mut hpack); }", 1);
    patch(&root, "client.rs", "        use http::request::Parts;",
        "        use http::request::Parts;\n        let go_compat = request.extensions().get::<crate::GoCompatibility>().is_some();", 1);
    patch(&root, "client.rs", "        let mut pseudo = Pseudo::request(method, uri, protocol);",
        "        let mut pseudo = Pseudo::request(method, uri, protocol);\n        pseudo.go_compat = go_compat;", 1);
    // Preserve only our inert marker; upstream must still drop user extensions before locking.
    patch(&root, "proto/streams/streams.rs", "        request.extensions_mut().clear();",
        "        let go = request.extensions_mut().remove::<crate::GoCompatibility>();\n        request.extensions_mut().clear();\n        if let Some(go) = go { request.extensions_mut().insert(go); }", 2);
    fs::copy("compat/go_hpack.rs", root.join("hpack/go_encoder.rs")).unwrap();
    // Tests are a sibling module, not embedded strings or unmeasured copied first-party code.
    fs::copy("compat/tests.rs", root.join("hpack/go_hpack.rs")).unwrap();
}
