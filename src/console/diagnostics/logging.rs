//! Runtime tracing owns its file on the backend machine, including standalone/server launches.
use super::{files, load_policy, retention};
use crate::console::ConsoleConfig;
use std::{
    fs::{File, OpenOptions},
    io::{self, Write},
    sync::{Arc, Mutex},
};

struct Writer {
    file: Arc<Mutex<File>>,
    echo: bool,
}
impl Write for Writer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.file
            .lock()
            .map_err(|_| io::Error::other("Gateway log lock poisoned"))?
            .write_all(bytes)?;
        if self.echo {
            io::stdout().write_all(bytes)?;
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        self.file
            .lock()
            .map_err(|_| io::Error::other("Gateway log lock poisoned"))?
            .flush()
    }
}

pub fn initialize(config: &mut ConsoleConfig) -> Result<(), String> {
    let captured = config.log_source.is_some()
        && std::env::var("GATEWAY_DESKTOP_MANAGED").as_deref() == Ok("1");
    let directory = files::directory(config)?;
    files::ensure_directory(&directory)?;
    let path = match &config.log_source {
        Some(path) => files::absolute(path).map_err(|_| "Cannot resolve Gateway log file")?,
        None => directory.join(format!(
            "gateway-runtime-{}-{}.log",
            std::process::id(),
            uuid::Uuid::new_v4()
        )),
    };
    files::reject_links(&path).map_err(|_| "Unsafe Gateway log file")?;
    retention::clean(&directory, &path, &load_policy(config)?)?;
    let mut options = OpenOptions::new();
    options.create(true).append(true);
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        options.share_mode(3).custom_flags(0x00200000);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
            .mode(0o600);
    }
    let file = options
        .open(&path)
        .map_err(|_| "Cannot open Gateway log file")?;
    files::validate_handle(&file).map_err(|_| "Unsafe Gateway log handle")?;
    let metadata = file
        .metadata()
        .map_err(|_| "Cannot inspect Gateway log handle")?;
    if !metadata.is_file() || files::reparse(&metadata) {
        return Err("Unsafe Gateway log handle".into());
    }
    // The managed process inherits a locked stdout handle from its launcher.
    #[cfg(unix)]
    if !captured {
        file.try_lock()
            .map_err(|_| "Gateway log is already in use")?;
    }
    let file = Arc::new(Mutex::new(file));
    tracing_subscriber::fmt()
        .with_ansi(false)
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .with_writer(move || Writer {
            file: Arc::clone(&file),
            echo: !captured,
        })
        .try_init()
        .map_err(|_| "Cannot initialize Gateway logging")?;
    config.log_source = Some(path);
    Ok(())
}
