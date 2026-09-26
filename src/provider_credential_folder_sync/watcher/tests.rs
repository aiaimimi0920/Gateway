use std::path::{Path, PathBuf};

use notify::{Event, EventKind};

use super::{folder_sync_event_should_trigger, folder_sync_path_should_trigger};

#[test]
fn folder_sync_event_ignores_access_only_changes() {
    let event = Event {
        kind: EventKind::Access(notify::event::AccessKind::Any),
        paths: vec![PathBuf::from("/tmp/example.json")],
        attrs: Default::default(),
    };
    assert!(!folder_sync_event_should_trigger(&event));
}

#[test]
fn folder_sync_event_triggers_for_json_changes() {
    let event = Event {
        kind: EventKind::Modify(notify::event::ModifyKind::Data(
            notify::event::DataChange::Any,
        )),
        paths: vec![PathBuf::from("/tmp/example.json")],
        attrs: Default::default(),
    };
    assert!(folder_sync_event_should_trigger(&event));
}

#[test]
fn folder_sync_path_triggers_for_provider_directory_changes() {
    assert!(folder_sync_path_should_trigger(Path::new("/tmp/codex")));
    assert!(!folder_sync_path_should_trigger(Path::new(
        "/tmp/readme.txt"
    )));
}
