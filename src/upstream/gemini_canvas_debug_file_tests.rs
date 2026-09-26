use super::*;

#[cfg(windows)]
#[test]
fn gemini_canvas_debug_file_failed_replace_preserves_old_and_removes_staging() {
    use std::os::windows::fs::OpenOptionsExt;
    use windows_sys::Win32::Storage::FileSystem::{FILE_SHARE_READ, FILE_SHARE_WRITE};
    let fixture = Fixture::new();
    let name = "gemini-canvas-held.json";
    let path = write_debug_file(&fixture.directory(), name, b"preserve")
        .unwrap()
        .unwrap();
    let held = OpenOptions::new()
        .read(true)
        .share_mode(FILE_SHARE_READ | FILE_SHARE_WRITE)
        .open(&path)
        .unwrap();
    assert!(write_debug_file(&fixture.directory(), name, b"replacement").is_err());
    drop(held);
    assert_eq!(std::fs::read(&path).unwrap(), b"preserve");
    assert_eq!(std::fs::read_dir(fixture.directory()).unwrap().count(), 2);
    assert!(
        write_debug_file(&fixture.directory(), name, b"after release")
            .unwrap()
            .is_some()
    );
}

#[test]
fn gemini_canvas_debug_file_excess_existing_entries_fail_closed() {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.directory()).unwrap();
    for n in 0..=MAX_FILES {
        std::fs::write(
            fixture.directory().join(format!("gemini-canvas-{n}.json")),
            b"preserve",
        )
        .unwrap();
    }
    assert!(
        write_debug_file(&fixture.directory(), "gemini-canvas-0.json", b"new")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        std::fs::read(fixture.directory().join("gemini-canvas-0.json")).unwrap(),
        b"preserve"
    );
}

#[test]
fn gemini_canvas_debug_file_lock_skips_and_pending_files_are_cleaned() {
    let fixture = Fixture::new();
    let name = "gemini-canvas-fixture.json";
    let path = write_debug_file(&fixture.directory(), name, b"old")
        .unwrap()
        .unwrap();
    let lock = OpenOptions::new()
        .read(true)
        .write(true)
        .open(fixture.directory().join(LOCK_NAME))
        .unwrap();
    lock.try_lock().unwrap();
    assert!(write_debug_file(&fixture.directory(), name, b"blocked")
        .unwrap()
        .is_none());
    drop(lock);
    assert_eq!(std::fs::read(&path).unwrap(), b"old");
    write_debug_file(&fixture.directory(), name, b"new")
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"new");
    assert_eq!(std::fs::read_dir(fixture.directory()).unwrap().count(), 2);
}

#[test]
fn gemini_canvas_debug_file_concurrent_writers_publish_complete_files() {
    let fixture = Fixture::new();
    std::fs::create_dir(fixture.directory()).unwrap();
    let barrier = std::sync::Arc::new(std::sync::Barrier::new(8));
    let threads: Vec<_> = (0..8)
        .map(|n| {
            let directory = fixture.directory();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                for _ in 0..16 {
                    write_debug_file(
                        &directory,
                        "gemini-canvas-shared.json",
                        &vec![b'a' + n; 1024],
                    )
                    .unwrap();
                }
            })
        })
        .collect();
    for thread in threads {
        thread.join().unwrap();
    }
    let bytes = std::fs::read(fixture.directory().join("gemini-canvas-shared.json")).unwrap();
    assert_eq!(bytes.len(), 1024);
    assert!(bytes.iter().all(|b| *b == bytes[0]));
    assert_eq!(std::fs::read_dir(fixture.directory()).unwrap().count(), 2);
}

#[test]
fn gemini_canvas_debug_file_nonregular_entries_are_preserved() {
    let fixture = Fixture::new();
    let target = fixture.directory().join("gemini-canvas-test.json");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("marker"), b"preserve").unwrap();
    assert!(write_debug_file(&fixture.directory(), "gemini-canvas-test.json", b"new").is_err());
    assert_eq!(std::fs::read(target.join("marker")).unwrap(), b"preserve");
}

struct Fixture(PathBuf);

impl Fixture {
    fn new() -> Self {
        let parent = std::env::temp_dir().canonicalize().unwrap();
        let path = parent.join(format!("gateway-debug-file-test-{}", uuid::Uuid::new_v4()));
        assert_eq!(path.parent(), Some(parent.as_path()));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }

    fn directory(&self) -> PathBuf {
        self.0.join("snapshots")
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[test]
fn gemini_canvas_debug_file_rejects_escape_and_reserved_names() {
    let fixture = Fixture::new();
    let outside = fixture.0.join("outside.txt");
    std::fs::write(&outside, b"preserve").unwrap();
    for name in [
        "../outside.txt",
        "..\\outside.txt",
        "C:escape.txt",
        "writer.lock",
        "NUL",
        "gemini-canvas-test.json:stream",
    ] {
        assert!(
            write_debug_file(&fixture.directory(), name, b"bad").is_err(),
            "{name}"
        );
    }
    assert_eq!(std::fs::read(outside).unwrap(), b"preserve");
}

#[test]
fn gemini_canvas_debug_file_size_skip_preserves_previous_snapshot() {
    let fixture = Fixture::new();
    let name = "gemini-canvas-test.json";
    let path = write_debug_file(&fixture.directory(), name, b"preserve")
        .unwrap()
        .unwrap();
    assert!(
        write_debug_file(&fixture.directory(), name, &vec![0; MAX_FILE_BYTES + 1])
            .unwrap()
            .is_none()
    );
    assert_eq!(std::fs::read(path).unwrap(), b"preserve");
}

#[test]
fn gemini_canvas_debug_file_count_is_bounded_without_pruning() {
    let fixture = Fixture::new();
    for n in 0..MAX_FILES {
        assert!(write_debug_file(
            &fixture.directory(),
            &format!("gemini-canvas-{n}.json"),
            b"old"
        )
        .unwrap()
        .is_some());
    }
    assert!(
        write_debug_file(&fixture.directory(), "gemini-canvas-extra.json", b"extra")
            .unwrap()
            .is_none()
    );
    let path = write_debug_file(&fixture.directory(), "gemini-canvas-0.json", b"new")
        .unwrap()
        .unwrap();
    assert_eq!(std::fs::read(path).unwrap(), b"new");
    assert_eq!(
        std::fs::read(fixture.directory().join("gemini-canvas-1.json")).unwrap(),
        b"old"
    );
}

#[test]
fn gemini_canvas_debug_file_total_bytes_are_bounded() {
    let fixture = Fixture::new();
    let bytes = vec![b'x'; MAX_FILE_BYTES];
    for n in 0..MAX_DIRECTORY_BYTES as usize / MAX_FILE_BYTES {
        write_debug_file(
            &fixture.directory(),
            &format!("gemini-canvas-{n}.jpg"),
            &bytes,
        )
        .unwrap()
        .unwrap();
    }
    assert!(
        write_debug_file(&fixture.directory(), "gemini-canvas-extra.jpg", b"x")
            .unwrap()
            .is_none()
    );
    assert!(
        write_debug_file(&fixture.directory(), "gemini-canvas-0.jpg", b"short")
            .unwrap()
            .is_some()
    );
    assert!(
        write_debug_file(&fixture.directory(), "gemini-canvas-extra.jpg", b"x")
            .unwrap()
            .is_some()
    );
}
