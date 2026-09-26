use std::path::{Path, PathBuf};

pub(super) fn node_entry(canonical: &Path) -> anyhow::Result<PathBuf> {
    #[cfg(not(windows))]
    return Ok(canonical.to_path_buf());

    #[cfg(windows)]
    {
        use std::ffi::OsString;
        use std::path::{Component, Prefix};

        let mut components = canonical.components();
        let Some(Component::Prefix(prefix)) = components.next() else {
            return Ok(canonical.to_path_buf());
        };
        let mut compatible = match prefix.kind() {
            Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:\\", char::from(drive))),
            Prefix::VerbatimUNC(server, share) => {
                let mut path = OsString::from(r"\\");
                path.push(server);
                path.push(r"\");
                path.push(share);
                PathBuf::from(path)
            }
            _ => return Ok(canonical.to_path_buf()),
        };
        for component in components {
            if !matches!(component, Component::RootDir) {
                compatible.push(component.as_os_str());
            }
        }
        // Node rejects verbatim main paths. Keep the allowlisted target unchanged
        // when converting its spelling for the interpreter.
        if compatible.canonicalize().ok().as_deref() != Some(canonical) {
            anyhow::bail!(
                "credential automation Node script path cannot preserve its canonical target"
            );
        }
        Ok(compatible)
    }
}
