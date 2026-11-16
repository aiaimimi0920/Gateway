//! Autostart must reopen the same data root, including isolated test installations.
use std::path::Path;

pub fn command(executable: &Path, data: &Path) -> Result<String, String> {
    if !executable.is_absolute() || !data.is_absolute() {
        return Err("Autostart paths must be absolute".into());
    }
    let exe = executable.to_str().ok_or("Invalid executable path")?;
    let root = data.to_str().ok_or("Invalid data path")?;
    if [exe, root]
        .iter()
        .any(|value| value.contains(['"', '\0', '\r', '\n']))
    {
        return Err("Invalid autostart path".into());
    }
    Ok(format!(
        "\"{}\" --gateway-autostart --gateway-data-dir \"{}\"",
        exe.replace('\\', "/"),
        root.replace('\\', "/")
    ))
}

pub fn data_root(args: &[String]) -> Result<Option<&str>, String> {
    let Some(position) = args.iter().position(|arg| arg == "--gateway-data-dir") else {
        return Ok(None);
    };
    let root = args
        .get(position + 1)
        .ok_or("Missing autostart data directory")?;
    if !Path::new(root).is_absolute() {
        return Err("Autostart data directory must be absolute".into());
    }
    Ok(Some(root))
}

pub fn apply() -> Result<(), String> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(root) = data_root(&args)? {
        // Runs before Tauri or worker threads start and before selected_root is resolved.
        std::env::set_var("GATEWAY_DATA_DIR", root);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn startup_retains_data_root_without_changing_the_environment() {
        let root = std::env::temp_dir().join("Gateway isolated data");
        let exe = root.join("gateway-ui.exe");
        let command = command(&exe, &root).unwrap();
        assert!(command.contains("--gateway-autostart --gateway-data-dir \""));
        let args = vec![
            "--gateway-data-dir".into(),
            root.to_string_lossy().into_owned(),
        ];
        assert_eq!(data_root(&args).unwrap(), Some(args[1].as_str()));
        assert!(data_root(&["--gateway-data-dir".into(), "relative".into()]).is_err());
        assert!(data_root(&["--gateway-data-dir".into()]).is_err());
        assert_eq!(data_root(&["--gateway-autostart".into()]).unwrap(), None);
    }
}
