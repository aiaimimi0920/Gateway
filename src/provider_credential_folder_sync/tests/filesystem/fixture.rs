//! Isolated filesystem fixtures; links are unlinked before recursive cleanup.
use std::fs;
use std::path::PathBuf;

pub(super) struct Directory {
    pub root: PathBuf,
    pub outside: PathBuf,
    base: PathBuf,
    links: Vec<PathBuf>,
}

impl Directory {
    pub fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "gateway-folder-containment-{}",
            uuid::Uuid::new_v4()
        ));
        fs::create_dir(&base).unwrap();
        let directory = Self {
            root: base.join("root"),
            outside: base.join("outside"),
            base,
            links: Vec::new(),
        };
        fs::create_dir(&directory.root).unwrap();
        fs::create_dir(&directory.outside).unwrap();
        directory
    }

    #[cfg(any(unix, windows))]
    pub fn link(&mut self, link: PathBuf, target: PathBuf) {
        #[cfg(unix)]
        std::os::unix::fs::symlink(&target, &link).unwrap();
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            let output = std::process::Command::new("cmd")
                .args(["/d", "/c", "mklink", "/J"])
                .arg(&link)
                .arg(&target)
                .creation_flags(0x08000000)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "junction creation failed: {output:?}"
            );
        }
        self.links.push(link);
    }
}

impl Drop for Directory {
    fn drop(&mut self) {
        for link in &self.links {
            #[cfg(windows)]
            let _ = fs::remove_dir(link);
            #[cfg(not(windows))]
            let _ = fs::remove_file(link);
        }
        let temp = std::env::temp_dir();
        if self.base.parent() == Some(temp.as_path())
            && self
                .base
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("gateway-folder-containment-")
        {
            let _ = fs::remove_dir_all(&self.base);
        }
    }
}
