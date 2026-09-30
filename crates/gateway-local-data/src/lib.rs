//! Shared EXE data ownership. No backend or desktop framework dependency.
pub mod schema;

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub fn env_path(name: &str) -> Option<PathBuf> {
    env::var_os(name)
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
}

pub fn resolve(
    executable: &Path,
    home: Option<&Path>,
    explicit: Option<&Path>,
) -> io::Result<PathBuf> {
    if let Some(path) = explicit {
        if !path.is_absolute() {
            return Err(io::Error::other(
                "Gateway data directory override must be absolute",
            ));
        }
        return Ok(path.to_path_buf());
    }
    let parent = executable
        .parent()
        .ok_or_else(|| io::Error::other("Missing executable directory"))?;
    let portable = parent.join(".ng");
    match fs::metadata(&portable) {
        Ok(metadata) if metadata.is_dir() => return Ok(portable),
        Ok(_) => {
            return Err(io::Error::other(
                "Executable-adjacent .ng must be a directory",
            ))
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => {}
        Err(error) => return Err(error),
    }
    let home = home
        .filter(|path| path.is_absolute())
        .ok_or_else(|| io::Error::other("Cannot resolve the current user's home directory"))?;
    Ok(home.join(".ng"))
}

pub fn selected_root() -> io::Result<PathBuf> {
    let home = if cfg!(windows) {
        env_path("USERPROFILE").or_else(|| env_path("HOME"))
    } else {
        env_path("HOME")
    };
    // The old override remains supported for existing automation/isolation.
    let explicit = env_path("GATEWAY_DATA_DIR").or_else(|| env_path("GATEWAY_UI_DATA_DIR"));
    resolve(&env::current_exe()?, home.as_deref(), explicit.as_deref())
}

pub fn prepare(root: &Path) -> io::Result<fs::File> {
    fs::create_dir_all(root)?;
    schema::upgrade(root)
}

pub fn local_environment(root: &Path) -> Vec<(&'static str, String)> {
    vec![
        ("GATEWAY_STORAGE_MODE", "local".into()),
        ("GATEWAY_DATA_DIR", root.to_string_lossy().into_owned()),
        (
            "GATEWAY_STATE_DIR",
            root.join("local/state").to_string_lossy().into_owned(),
        ),
        (
            "GATEWAY_ROUTES_FILE",
            root.join("local/routes.yaml")
                .to_string_lossy()
                .into_owned(),
        ),
        ("AI_GATEWAY_OBJECT_STORAGE_DRIVER", "local".into()),
        (
            "AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR",
            root.join("objects").to_string_lossy().into_owned(),
        ),
        (
            "GATEWAY_PROVIDER_CREDENTIAL_FOLDER_SYNC_ROOT_DIR",
            root.join("credentials").to_string_lossy().into_owned(),
        ),
    ]
}
