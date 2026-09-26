use std::path::PathBuf;

pub(super) fn gemini_canvas_local_runtime_root() -> PathBuf {
    let direct = PathBuf::from(".runtime");
    if direct.exists() {
        return direct;
    }

    let parent = PathBuf::from("..").join(".runtime");
    if parent.exists() {
        return parent;
    }

    direct
}

pub(crate) fn gemini_canvas_image_edit_debug_output_path(name: &str) -> PathBuf {
    gemini_canvas_local_runtime_root().join(name)
}
