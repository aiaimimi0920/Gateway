//! Windows EXE defaults; explicitly configured server deployments remain untouched.
use std::fs::{File, OpenOptions};
use std::io::Write;

pub fn configure() -> anyhow::Result<Option<File>> {
    let managed = std::env::var("GATEWAY_DESKTOP_MANAGED").as_deref() == Ok("1");
    let selected =
        neuro_gateway::config::GatewayStorageMode::from_env().map_err(anyhow::Error::msg)?;
    let explicit = std::env::var_os("GATEWAY_STORAGE_MODE").is_some();
    if explicit && selected == neuro_gateway::config::GatewayStorageMode::Server {
        return Ok(None);
    }
    let explicit_root = gateway_local_data::env_path("GATEWAY_DATA_DIR").is_some();
    let has_server_config = [
        "GATEWAY_REDIS_URL",
        "GATEWAY_DATABASE_URL",
        "DATABASE_URL",
        "GATEWAY_ROUTES_FILE",
        "GATEWAY_STATE_DIR",
    ]
    .iter()
    .any(|name| std::env::var_os(name).is_some());
    let server_role = std::env::var("GATEWAY_RUNTIME_ROLE").is_ok_and(|role| role != "standalone");
    if !managed
        && selected != neuro_gateway::config::GatewayStorageMode::Local
        && (!cfg!(windows) || server_role || (has_server_config && !explicit_root))
    {
        return Ok(None);
    }
    let root = gateway_local_data::selected_root()?;
    let lease = gateway_local_data::prepare(&root)?;
    std::fs::create_dir_all(root.join("local"))?;
    let routes = root.join("local/routes.yaml");
    match OpenOptions::new().write(true).create_new(true).open(routes) {
        Ok(mut file) => file.write_all(b"providers: []\nmodel_routes: []\n")?,
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(error) => return Err(error.into()),
    }
    // This runs before constructing Tokio or worker threads.
    for (key, value) in gateway_local_data::local_environment(&root) {
        std::env::set_var(key, value);
    }
    for (key, value) in [
        ("GATEWAY_RUNTIME_ROLE", "standalone"),
        ("GATEWAY_BIND_HOST", "127.0.0.1"),
    ] {
        if std::env::var_os(key).is_none() {
            std::env::set_var(key, value);
        }
    }
    Ok(Some(lease))
}
