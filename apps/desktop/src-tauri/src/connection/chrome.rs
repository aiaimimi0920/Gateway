use tauri::{AppHandle, Manager, Url};

// A fixed navigation bridge keeps server pages outside the native IPC capability
// boundary. It accepts no target window, filesystem path, settings or process data.
pub const INITIALIZATION_SCRIPT: &str = r#"
Object.defineProperty(window, '__GATEWAY_WINDOW_CHROME__', {
  value: Object.freeze({ send(action) {
    window.location.href = 'gateway-window://control/' + action;
  } })
});
"#;

#[derive(Debug, PartialEq)]
enum Action {
    Ready,
    Minimize,
    ToggleMaximize,
    Close,
    Connection,
    Drag,
}

fn action(url: &Url) -> Option<Action> {
    match url.as_str() {
        "gateway-window://control/ready" => Some(Action::Ready),
        "gateway-window://control/minimize" => Some(Action::Minimize),
        "gateway-window://control/toggle-maximize" => Some(Action::ToggleMaximize),
        "gateway-window://control/close" => Some(Action::Close),
        "gateway-window://control/connection" => Some(Action::Connection),
        "gateway-window://control/drag" => Some(Action::Drag),
        _ => None,
    }
}

pub fn handle_navigation(app: &AppHandle, label: &str, url: &Url) -> bool {
    let Some(action) = action(url) else {
        return false;
    };
    if !matches!(label, "main" | "gateway-console") {
        return true;
    }
    let Some(window) = app.get_webview_window(label) else {
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
        Action::Connection => super::window::show_settings(app),
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
        ] {
            assert_eq!(action(&Url::parse(input).unwrap()), None, "{input}");
        }
    }
}
