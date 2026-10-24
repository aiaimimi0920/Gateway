use tauri::{AppHandle, Manager, Url};

// The presentation bridge grants no native commands. Only allowlisted settings
// categories, bounded geometry, theme and locale may cross into a local child view.
pub const INITIALIZATION_SCRIPT: &str = r#"
(() => {
// Settings use a bounded resource-request channel, never a top-level navigation.
let settingsPending = null, settingsRunning = false, settingsAbort = null;
window.addEventListener('pagehide', () => { settingsPending = null; settingsAbort?.abort(); });
async function sendSettings(path) {
  settingsPending = path;
  if (settingsRunning) return;
  settingsRunning = true;
  try {
    while (settingsPending !== null) {
      const next = settingsPending; settingsPending = null;
      const controller = new AbortController(); settingsAbort = controller;
      const timer = setTimeout(() => controller.abort(), 4000);
      try {
        const response = await fetch('__GATEWAY_SETTINGS_BASE__' + next, {
          credentials: 'omit', cache: 'no-store', redirect: 'error', signal: controller.signal
        });
        if (!response.ok) throw new Error('Settings bridge unavailable');
      } catch (_) { window.dispatchEvent(new Event('gateway:settings-error')); }
      finally { clearTimeout(timer); settingsAbort = null; }
    }
  } finally { settingsRunning = false; }
}
Object.defineProperty(window, '__GATEWAY_WINDOW_CHROME__', {
  value: Object.freeze({ send(action) {
    if (action === 'settings-hide') { void sendSettings('/hide'); return; }
    window.location.href = 'gateway-window://control/' + action;
  }, showSettings(layout) {
    void sendSettings('/show?layout=' + encodeURIComponent(JSON.stringify(layout)));
  } })
});
})();
"#;

pub fn initialization_script() -> String {
    INITIALIZATION_SCRIPT.replace(
        "__GATEWAY_SETTINGS_BASE__",
        if cfg!(any(target_os = "windows", target_os = "android")) {
            "https://gateway-settings.localhost"
        } else {
            "gateway-settings://localhost"
        },
    )
}

#[derive(Debug, PartialEq)]
enum Action {
    Ready,
    Minimize,
    ToggleMaximize,
    Close,
    Connection,
    Drag,
    Quit,
    DesktopSettings,
    Console,
    SettingsHide,
}

fn action(url: &Url) -> Option<Action> {
    match url.as_str() {
        "gateway-window://control/ready" => Some(Action::Ready),
        "gateway-window://control/minimize" => Some(Action::Minimize),
        "gateway-window://control/toggle-maximize" => Some(Action::ToggleMaximize),
        "gateway-window://control/close" => Some(Action::Close),
        "gateway-window://control/connection" => Some(Action::Connection),
        "gateway-window://control/drag" => Some(Action::Drag),
        "gateway-window://control/quit" => Some(Action::Quit),
        "gateway-window://control/settings-desktop" => Some(Action::DesktopSettings),
        "gateway-window://control/console" => Some(Action::Console),
        "gateway-window://control/settings-hide" => Some(Action::SettingsHide),
        _ => None,
    }
}

pub fn handle_navigation(app: &AppHandle, label: &str, url: &Url) -> bool {
    if url.scheme() == "gateway-window" && url.host_str() == Some("settings") {
        if label == "gateway-console" {
            if let Some(layout) = super::panel_request::Layout::parse(url) {
                super::panel::enqueue(app, super::panel::Request::Show(layout));
            }
        }
        return true;
    }
    let Some(action) = action(url) else {
        return false;
    };
    if !matches!(label, "main" | "gateway-console") {
        return true;
    }
    let Some(window) = app.get_window(label) else {
        return true;
    };
    match action {
        Action::Ready => {
            // Older servers retain their native frame/menu until their UI opts in.
            let _ = window.set_decorations(false);
            let _ = window.remove_menu();
        }
        Action::Minimize => {
            let _ = window.minimize();
        }
        Action::ToggleMaximize => {
            if window.is_maximized().unwrap_or(false) {
                let _ = window.unmaximize();
            } else {
                let _ = window.maximize();
            }
        }
        Action::Close => {
            let _ = window.close();
        }
        Action::Connection => super::window::show_category(app, "backend"),
        Action::DesktopSettings => super::window::show_category(app, "desktop"),
        Action::Console => crate::desktop_tray::restore(app),
        Action::SettingsHide => {
            if label == "gateway-console" {
                super::panel::enqueue(app, super::panel::Request::Hide);
            }
        }
        Action::Quit => app.exit(0),
        Action::Drag => {
            let _ = window.start_dragging();
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_fixed_window_actions() {
        for name in [
            "ready",
            "minimize",
            "toggle-maximize",
            "close",
            "connection",
            "drag",
            "quit",
            "settings-desktop",
            "console",
            "settings-hide",
        ] {
            assert!(
                action(&Url::parse(&format!("gateway-window://control/{name}")).unwrap()).is_some()
            );
        }
        for input in [
            "https://control/close",
            "gateway-window://other/close",
            "gateway-window://control/close?label=main",
            "gateway-window://control/close#x",
            "gateway-window://user@control/close",
            "gateway-window://control:42/close",
            "gateway-window://control/save_profile",
            "gateway-window://control/stop_gateway_sidecar",
            "gateway-window://control/%63lose",
            "gateway-window://control/close/",
            "gateway-window://control/settings-desktop?autostart=true",
            "gateway-window://control/settings-diagnostics#export",
            "gateway-window://control/settings-diagnostics",
        ] {
            assert_eq!(action(&Url::parse(input).unwrap()), None, "{input}");
        }
    }
}
