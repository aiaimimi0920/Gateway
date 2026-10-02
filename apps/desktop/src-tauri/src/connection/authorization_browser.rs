//! Only the official ChatGPT authorization endpoint may leave the console WebView.
use std::{
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    thread::JoinHandle,
    time::{Duration, Instant},
};
use tauri::{AppHandle, Manager, Url};

#[derive(Default)]
pub(crate) struct AuthorizationBrowser {
    launch: Mutex<Launch>,
    stopping: Arc<AtomicBool>,
}

#[derive(Default)]
struct Launch {
    worker: Option<JoinHandle<()>>,
    last_started: Option<Instant>,
}

fn allowed(url: &Url) -> bool {
    url.as_str().len() <= 8192
        && url.scheme() == "https"
        && url.host_str() == Some("auth.openai.com")
        && url.port_or_known_default() == Some(443)
        && url.path() == "/oauth/authorize"
        && url.username().is_empty()
        && url.password().is_none()
        && url.fragment().is_none()
}

impl AuthorizationBrowser {
    fn spawn(&self, work: impl FnOnce(Arc<AtomicBool>) + Send + 'static) -> bool {
        let Ok(mut launch) = self.launch.try_lock() else {
            return false;
        };
        if self.stopping.load(Ordering::Acquire)
            || launch
                .worker
                .as_ref()
                .is_some_and(|worker| !worker.is_finished())
            || launch
                .last_started
                .is_some_and(|time| time.elapsed() < Duration::from_secs(1))
        {
            return false;
        }
        if let Some(worker) = launch.worker.take() {
            let _ = worker.join();
        }
        let stopping = self.stopping.clone();
        match std::thread::Builder::new()
            .name("oauth-browser".into())
            .spawn(move || work(stopping))
        {
            Ok(worker) => {
                launch.worker = Some(worker);
                launch.last_started = Some(Instant::now());
                true
            }
            Err(_) => false,
        }
    }

    pub(super) fn open(&self, app: AppHandle, url: Url) {
        if !allowed(&url) {
            return;
        }
        let origin = app
            .state::<super::ConnectionState>()
            .origin
            .lock()
            .ok()
            .and_then(|v| v.clone());
        let fallback = app.clone();
        if !self.spawn(move |stopping| {
            let success = run_bounded(browser_command(&url), Duration::from_secs(10), &stopping);
            if !success && !stopping.load(Ordering::Acquire) {
                let target = app.clone();
                // Never wait for the UI thread: exit joins this worker on that thread.
                let _ = app.run_on_main_thread(move || {
                    if let Some(window) = target.get_webview_window("gateway-console") {
                        if window.url().ok().map(|v| v.origin().ascii_serialization()) == origin {
                            notify_failure(&target);
                        }
                    }
                });
            }
        }) {
            notify_failure(&fallback);
        }
    }

    pub(crate) fn stop(&self) {
        // App exit cancels and joins the sole worker before the process disappears.
        self.stopping.store(true, Ordering::Release);
        if let Ok(mut launch) = self.launch.lock() {
            if let Some(worker) = launch.worker.take() {
                let _ = worker.join();
            }
        }
    }
}

fn notify_failure(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("gateway-console") {
        let _ = window.eval("window.dispatchEvent(new Event('gateway:oauth-browser-failed'))");
    }
}

fn browser_command(url: &Url) -> Command {
    #[cfg(target_os = "windows")]
    let mut command = {
        use std::os::windows::process::CommandExt;
        let mut command = Command::new("rundll32.exe");
        command
            .arg("url.dll,FileProtocolHandler")
            .creation_flags(0x08000000);
        command
    };
    #[cfg(target_os = "macos")]
    let mut command = Command::new("open");
    #[cfg(not(any(target_os = "windows", target_os = "macos")))]
    let mut command = Command::new("xdg-open");
    // An argv value, not a shell command. Never log authorization URLs or child output.
    command.arg(url.as_str());
    command
}

fn run_bounded(mut command: Command, timeout: Duration, stopping: &AtomicBool) -> bool {
    if stopping.load(Ordering::Acquire) {
        return false;
    }
    command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let Ok(mut child) = command.spawn() else {
        return false;
    };
    let deadline = Instant::now() + timeout;
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return status.success(),
            Ok(None) if Instant::now() < deadline && !stopping.load(Ordering::Acquire) => {
                std::thread::sleep(Duration::from_millis(25))
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return false;
            }
        }
    }
}

#[cfg(test)]
#[path = "authorization_browser_tests.rs"]
mod tests;
