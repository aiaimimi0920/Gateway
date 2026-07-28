#[allow(dead_code)]
mod build_script {
    include!("../build.rs");

    #[cfg(test)]
    mod contract_tests {
        use super::*;
        use std::cell::Cell;
        use std::collections::BTreeSet;
        use std::path::{Path, PathBuf};
        use std::sync::Mutex;
        use std::time::{SystemTime, UNIX_EPOCH};

        static CURRENT_DIR_LOCK: Mutex<()> = Mutex::new(());

        struct Fixture {
            previous_dir: PathBuf,
            root: PathBuf,
        }

        impl Fixture {
            fn new(index: &[u8], files: &[(&str, &[u8])]) -> Self {
                let previous_dir = env::current_dir().expect("read current directory");
                let unique = SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("clock after Unix epoch")
                    .as_nanos();
                let root = env::temp_dir().join(format!(
                    "gateway-prebuilt-web-ui-{}-{unique}",
                    std::process::id()
                ));
                let web_root = root.join("apps/desktop/dist/web");
                fs::create_dir_all(&web_root).expect("create web fixture root");
                fs::write(web_root.join("index.html"), index).expect("write fixture index");
                for (relative_path, contents) in files {
                    let destination = web_root.join(relative_path);
                    fs::create_dir_all(destination.parent().expect("fixture file parent"))
                        .expect("create fixture file parent");
                    fs::write(destination, contents).expect("write fixture file");
                }
                env::set_current_dir(&root).expect("enter fixture root");
                Self { previous_dir, root }
            }

            fn write_marker(&self, files: &[(&str, &[u8])]) {
                let mut manifest = serde_json::Map::new();
                for (relative_path, contents) in files {
                    manifest.insert(
                        (*relative_path).to_string(),
                        serde_json::Value::String(digest(contents)),
                    );
                }
                let index = fs::read(DIST_WEB_INDEX_PATH).expect("read fixture index");
                let marker = serde_json::json!({
                    "schemaVersion": 1,
                    "indexSha256": digest(&index),
                    "files": manifest,
                    "publishedAt": "2026-07-28T00:00:00.000Z",
                });
                fs::write(DIST_WEB_READY_PATH, format!("{marker}\n"))
                    .expect("write fixture ready marker");
            }

            fn create_publish_lock(&self) {
                self.create_publish_lock_with_owner(serde_json::json!({
                    "ownerToken": "active-publisher",
                }));
            }

            fn create_publish_lock_with_owner(&self, owner: serde_json::Value) {
                let owner_path =
                    Path::new("apps/desktop/dist/.gateway-web-publish.lock").join("owner.json");
                fs::create_dir_all(owner_path.parent().expect("publish lock parent"))
                    .expect("create publish lock");
                fs::write(owner_path, format!("{owner}\n")).expect("write publish lock owner");
            }

            fn snapshot_path(&self, name: &str) -> PathBuf {
                self.root.join("out").join(name)
            }
        }

        impl Drop for Fixture {
            fn drop(&mut self) {
                env::set_current_dir(&self.previous_dir).expect("restore current directory");
                fs::remove_dir_all(&self.root).expect("remove web fixture");
            }
        }

        fn digest(contents: &[u8]) -> String {
            hex::encode(Sha256::digest(contents))
        }

        fn lock_current_dir() -> std::sync::MutexGuard<'static, ()> {
            CURRENT_DIR_LOCK
                .lock()
                .unwrap_or_else(|poisoned| poisoned.into_inner())
        }

        fn relative_files(root: &Path) -> BTreeSet<String> {
            fn visit(root: &Path, directory: &Path, files: &mut BTreeSet<String>) {
                for entry in fs::read_dir(directory).expect("read snapshot directory") {
                    let entry = entry.expect("read snapshot entry");
                    let path = entry.path();
                    if entry
                        .file_type()
                        .expect("read snapshot entry type")
                        .is_dir()
                    {
                        visit(root, &path, files);
                    } else {
                        files.insert(
                            path.strip_prefix(root)
                                .expect("snapshot path below root")
                                .to_string_lossy()
                                .replace('\\', "/"),
                        );
                    }
                }
            }

            let mut files = BTreeSet::new();
            visit(root, root, &mut files);
            files
        }

        #[test]
        fn rejects_a_ready_marker_while_the_publisher_lock_is_active() {
            let _cwd_guard = lock_current_dir();
            let index = br#"<script src="/ui/static/js/app.js"></script>"#;
            let asset = b"console.log('ready');";
            let fixture = Fixture::new(index, &[("static/js/app.js", asset)]);
            fixture.write_marker(&[("index.html", index), ("static/js/app.js", asset)]);
            fixture.create_publish_lock();

            let error = validate_prebuilt_web_ui().expect_err("active publish must be rejected");

            assert!(error.contains("publish lock"), "error={error}");
        }

        #[test]
        fn rejects_a_marker_that_omits_an_index_referenced_asset() {
            let _cwd_guard = lock_current_dir();
            let index = br#"<script src="/ui/static/js/app.js"></script>"#;
            let fixture = Fixture::new(index, &[]);
            fixture.write_marker(&[("index.html", index)]);

            let error = validate_prebuilt_web_ui()
                .expect_err("marker without referenced asset must be rejected");

            assert!(error.contains("static/js/app.js"), "error={error}");
        }

        #[test]
        fn rejects_a_published_asset_whose_digest_does_not_match_the_marker() {
            let _cwd_guard = lock_current_dir();
            let index = br#"<script src="/ui/static/js/app.js"></script>"#;
            let published_asset = b"console.log('corrupt');";
            let expected_asset = b"console.log('ready');";
            let fixture = Fixture::new(index, &[("static/js/app.js", published_asset)]);
            fixture.write_marker(&[("index.html", index), ("static/js/app.js", expected_asset)]);

            let error =
                validate_prebuilt_web_ui().expect_err("asset digest mismatch must be rejected");

            assert!(error.contains("static/js/app.js"), "error={error}");
        }

        #[test]
        fn accepts_a_stable_complete_published_manifest() {
            let _cwd_guard = lock_current_dir();
            let index = br#"<link href="/ui/static/css/app.css" rel="stylesheet"><script src="/ui/static/js/app.js"></script>"#;
            let css = b"body { color: white; }";
            let js = b"console.log('ready');";
            let fixture = Fixture::new(
                index,
                &[("static/css/app.css", css), ("static/js/app.js", js)],
            );
            fixture.write_marker(&[
                ("index.html", index),
                ("static/css/app.css", css),
                ("static/js/app.js", js),
            ]);

            validate_prebuilt_web_ui().expect("complete publish must validate");
        }

        #[test]
        fn stale_lock_recovery_requires_a_parseable_owner_pid() {
            let _cwd_guard = lock_current_dir();
            let fixture = Fixture::new(b"index", &[]);
            fixture.create_publish_lock();
            let process_probe_called = Cell::new(false);

            let recovered = recover_stale_publish_lock_with(|_| {
                process_probe_called.set(true);
                Ok(false)
            })
            .expect("inspect owner without pid");

            assert!(!recovered);
            assert!(!process_probe_called.get());
            assert!(Path::new(DIST_WEB_PUBLISH_LOCK_PATH).is_dir());
        }

        #[test]
        fn stale_lock_recovery_preserves_a_lock_while_its_owner_is_alive() {
            let _cwd_guard = lock_current_dir();
            let fixture = Fixture::new(b"index", &[]);
            fixture.create_publish_lock_with_owner(serde_json::json!({
                "ownerToken": "live-publisher",
                "pid": 4242,
            }));

            let recovered = recover_stale_publish_lock_with(|pid| {
                assert_eq!(pid, 4242);
                Ok(true)
            })
            .expect("inspect live owner");

            assert!(!recovered);
            assert!(Path::new(DIST_WEB_PUBLISH_LOCK_PATH).is_dir());
        }

        #[test]
        fn platform_process_probe_recognizes_the_current_process() {
            assert!(
                process_is_alive(std::process::id()).expect("inspect current process"),
                "the integration-test process must still be alive"
            );
        }

        #[test]
        fn stale_lock_recovery_preserves_a_lock_when_liveness_cannot_be_proven() {
            let _cwd_guard = lock_current_dir();
            let fixture = Fixture::new(b"index", &[]);
            fixture.create_publish_lock_with_owner(serde_json::json!({
                "ownerToken": "unknown-publisher",
                "pid": 4242,
            }));

            let recovered = recover_stale_publish_lock_with(|pid| {
                assert_eq!(pid, 4242);
                Err("process inspection unavailable".to_string())
            })
            .expect("an inconclusive probe keeps the lock active");

            assert!(!recovered);
            assert!(Path::new(DIST_WEB_PUBLISH_LOCK_PATH).is_dir());
        }

        #[test]
        fn stale_lock_recovery_reclaims_only_a_lock_whose_owner_has_exited() {
            let _cwd_guard = lock_current_dir();
            let fixture = Fixture::new(b"index", &[]);
            fixture.create_publish_lock_with_owner(serde_json::json!({
                "ownerToken": "dead-publisher",
                "pid": 4242,
            }));

            let recovered = recover_stale_publish_lock_with(|pid| {
                assert_eq!(pid, 4242);
                Ok(false)
            })
            .expect("recover dead owner lock");

            assert!(recovered);
            assert!(!Path::new(DIST_WEB_PUBLISH_LOCK_PATH).exists());
        }

        #[test]
        fn active_lock_waiting_has_a_bounded_retry_budget() {
            assert!(PREBUILT_WEB_UI_WAIT_ATTEMPTS > 0);
            let retry_budget = PREBUILT_WEB_UI_WAIT_INTERVAL
                .checked_mul(PREBUILT_WEB_UI_WAIT_ATTEMPTS as u32)
                .expect("bounded retry duration");

            assert!(retry_budget <= std::time::Duration::from_secs(5));
        }

        #[test]
        fn snapshot_excludes_live_files_missing_from_the_ready_marker() {
            let _cwd_guard = lock_current_dir();
            let index = br#"<script src="/ui/static/js/app.js"></script>"#;
            let js = b"console.log('ready');";
            let stale = b"console.log('stale');";
            let fixture = Fixture::new(
                index,
                &[("static/js/app.js", js), ("static/js/old.js", stale)],
            );
            fixture.write_marker(&[("index.html", index), ("static/js/app.js", js)]);
            let snapshot = fixture.snapshot_path("manifest-only");
            fs::create_dir_all(snapshot.join("static/js"))
                .expect("create previous snapshot directory");
            fs::write(snapshot.join("static/js/previous.js"), b"previous snapshot")
                .expect("write previous snapshot residue");

            snapshot_prebuilt_web_ui(&snapshot).expect("create manifest-only snapshot");

            assert_eq!(
                relative_files(&snapshot),
                BTreeSet::from(["index.html".to_string(), "static/js/app.js".to_string(),])
            );
            assert!(!snapshot.join("static/js/old.js").exists());
            assert!(!snapshot.join("static/js/previous.js").exists());
        }

        #[test]
        fn snapshot_is_detached_from_later_live_publish_mutation() {
            let _cwd_guard = lock_current_dir();
            let index = br#"<script src="/ui/static/js/app.js"></script>"#;
            let js = b"console.log('ready');";
            let fixture = Fixture::new(index, &[("static/js/app.js", js)]);
            fixture.write_marker(&[("index.html", index), ("static/js/app.js", js)]);
            let snapshot = fixture.snapshot_path("detached");

            snapshot_prebuilt_web_ui(&snapshot).expect("create detached snapshot");
            fs::write(DIST_WEB_INDEX_PATH, b"new live index").expect("mutate live index");
            fs::write(
                Path::new(DIST_WEB_ROOT_PATH).join("static/js/app.js"),
                b"console.log('new live publish');",
            )
            .expect("mutate live asset");

            assert_eq!(fs::read(snapshot.join("index.html")).unwrap(), index);
            assert_eq!(fs::read(snapshot.join("static/js/app.js")).unwrap(), js);
        }

        #[test]
        fn snapshot_copy_rechecks_the_destination_digest() {
            let _cwd_guard = lock_current_dir();
            let index = br#"<script src="/ui/static/js/app.js"></script>"#;
            let js = b"console.log('ready');";
            let fixture = Fixture::new(index, &[("static/js/app.js", js)]);
            fixture.write_marker(&[("index.html", index), ("static/js/app.js", js)]);
            let published = validate_prebuilt_web_ui_locked().expect("validate fixture publish");
            let staging = fixture.snapshot_path("corrupt-copy");

            let error = copy_validated_files_to_directory(
                &published.files,
                &staging,
                |source, destination| {
                    fs::copy(source, destination)
                        .map(|_| ())
                        .map_err(|error| error.to_string())?;
                    if destination.ends_with("app.js") {
                        fs::write(destination, b"corrupted after copy")
                            .map_err(|error| error.to_string())?;
                    }
                    Ok(())
                },
            )
            .expect_err("post-copy corruption must be rejected");

            assert!(error.contains("static/js/app.js"), "error={error}");
            assert!(error.contains("snapshot"), "error={error}");
        }

        #[test]
        fn prebuilt_mode_snapshots_without_invoking_the_source_builder() {
            let _cwd_guard = lock_current_dir();
            let index = b"prebuilt index";
            let fixture = Fixture::new(index, &[]);
            fixture.write_marker(&[("index.html", index)]);
            let snapshot = fixture.snapshot_path("prebuilt-mode");
            let builder_called = Cell::new(false);

            prepare_web_ui_snapshot(true, &snapshot, || {
                builder_called.set(true);
                Err("source builder must not run".to_string())
            })
            .expect("snapshot prebuilt publish");

            assert!(!builder_called.get());
            assert_eq!(fs::read(snapshot.join("index.html")).unwrap(), index);
        }

        #[test]
        fn source_build_mode_snapshots_the_publish_created_by_the_builder() {
            let _cwd_guard = lock_current_dir();
            let index = b"source-built index";
            let fixture = Fixture::new(b"old index", &[]);
            let snapshot = fixture.snapshot_path("source-mode");
            let builder_called = Cell::new(false);

            prepare_web_ui_snapshot(false, &snapshot, || {
                builder_called.set(true);
                fs::write(DIST_WEB_INDEX_PATH, index).expect("publish source-built index");
                fixture.write_marker(&[("index.html", index)]);
                Ok(())
            })
            .expect("snapshot source-built publish");

            assert!(builder_called.get());
            assert_eq!(fs::read(snapshot.join("index.html")).unwrap(), index);
        }

        #[test]
        fn generated_rust_embed_source_targets_only_the_out_dir_snapshot() {
            let _cwd_guard = lock_current_dir();
            let fixture = Fixture::new(b"index", &[]);
            let out_dir = fixture.snapshot_path("generated-source");
            let snapshot = fixture.snapshot_path("embed-snapshot");
            fs::create_dir_all(&snapshot).expect("create embed snapshot");

            let generated = write_embedded_ui_source(&out_dir, &snapshot)
                .expect("write generated RustEmbed source");
            let source = fs::read_to_string(generated).expect("read generated RustEmbed source");
            let ui_source = include_str!("../src/http/ui.rs");
            let escaped_snapshot = snapshot.to_string_lossy().replace('\\', "\\\\");

            assert!(source.contains("derive(rust_embed::RustEmbed)"));
            assert!(source.contains(escaped_snapshot.as_str()));
            assert!(source.contains(r#"env!("GATEWAY_WEB_UI_SNAPSHOT")"#));
            assert!(!source.contains(DIST_WEB_ROOT_PATH));
            assert!(ui_source
                .contains(r#"include!(concat!(env!("OUT_DIR"), "/gateway_embedded_ui.rs"))"#));
            assert!(!ui_source.contains("apps/desktop/dist/web/"));
        }

        #[test]
        fn snapshot_path_is_exported_to_rustc_for_build_diagnostics() {
            let snapshot = Path::new("C:/out/gateway-web-ui-snapshot");

            let directive =
                snapshot_rustc_env_directive(snapshot).expect("build rustc env directive");

            assert_eq!(
                directive,
                "cargo:rustc-env=GATEWAY_WEB_UI_SNAPSHOT=C:/out/gateway-web-ui-snapshot"
            );
        }
    }
}
