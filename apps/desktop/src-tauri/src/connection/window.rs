use super::ConnectionState;
use std::io::Read;
use std::time::{Duration, Instant};
use tauri::menu::{Menu, MenuItem};
use tauri::{
    webview::PageLoadEvent, window::Color, AppHandle, Manager, Url, WebviewUrl,
    WebviewWindowBuilder,
};

pub fn wait_for_console(url: &Url, local: bool) -> Result<(), String> {
    let client = reqwest::blocking::Client::builder()
        .connect_timeout(Duration::from_secs(2))
        .timeout(Duration::from_secs(3))
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .map_err(|error| error.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(if local { 45 } else { 3 });
    loop {
        if let Ok(response) = client.get(url.clone()).send() {
            if response.status().is_success() {
                let mut body = String::new();
                if response.take(256 * 1024).read_to_string(&mut body).is_ok()
                    && body.contains("Neuro Gateway")
                    && body.contains("root")
                {
                    return Ok(());
                }
            }
        }
        if Instant::now() >= deadline {
            return Err(
                "Gateway /ui/ is unavailable. Check the server address and connection, then retry."
                    .into(),
            );
        }
        std::thread::sleep(Duration::from_millis(250));
    }
}

pub fn show_settings(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

pub fn open(app: &AppHandle, url: Url) -> Result<(), String> {
    *app.state::<ConnectionState>()
        .origin
        .lock()
        .map_err(|_| "Connection lock poisoned")? = Some(url.origin().ascii_serialization());
    if let Some(window) = app.get_webview_window("gateway-console") {
        window
            .set_decorations(true)
            .map_err(|error| error.to_string())?;
        window
            .set_menu(native_menu(app)?)
            .map_err(|error| error.to_string())?;
        window.hide().map_err(|error| error.to_string())?;
        window.navigate(url).map_err(|error| error.to_string())?;
    } else {
        let menu = native_menu(app)?;
        let navigation_app = app.clone();
        let browser_app = app.clone();
        // Server pages receive no native IPC permissions. Window chrome uses only
        // the fixed, intercepted navigation actions in chrome.rs.
        WebviewWindowBuilder::new(app, "gateway-console", WebviewUrl::External(url))
            .data_directory(crate::paths::gateway_app_dir()?.join("webview"))
            .title("Gateway")
            .visible(false)
            .background_color(Color(6, 8, 13, 255))
            .inner_size(1280.0, 850.0)
            .min_inner_size(900.0, 600.0)
            .initialization_script(super::chrome::INITIALIZATION_SCRIPT)
            .on_page_load(|window, payload| {
                // Do not expose WebView2's empty document or the intermediate connection form.
                if !matches!(payload.event(), PageLoadEvent::Finished) {
                    return;
                }
                let app = window.app_handle();
                let allowed = app
                    .state::<ConnectionState>()
                    .origin
                    .lock()
                    .map(|origin| {
                        origin.as_ref() == Some(&payload.url().origin().ascii_serialization())
                    })
                    .unwrap_or(false);
                if !allowed {
                    show_settings(app);
                    return;
                }
                if !window.is_visible().unwrap_or(false) {
                    if window.show().is_ok() {
                        let _ = window.set_focus();
                        if let Some(settings) = app.get_webview_window("main") {
                            let _ = settings.hide();
                        }
                    } else {
                        show_settings(app);
                    }
                }
            })
            .menu(menu)
            .on_new_window(move |url, _| {
                browser_app
                    .state::<super::authorization_browser::AuthorizationBrowser>()
                    .open(browser_app.clone(), url);
                // OAuth uses the system browser, never an IPC-enabled child WebView.
                tauri::webview::NewWindowResponse::Deny
            })
            .on_navigation(move |target| {
                if super::chrome::handle_navigation(&navigation_app, "gateway-console", target) {
                    return false;
                }
                navigation_app
                    .state::<ConnectionState>()
                    .origin
                    .lock()
                    .map(|allowed| allowed.as_ref() == Some(&target.origin().ascii_serialization()))
                    .unwrap_or(false)
            })
            .on_menu_event(|window, event| match event.id().as_ref() {
                "gateway-connection" => show_settings(window.app_handle()),
                "gateway-quit" => window.app_handle().exit(0),
                _ => {}
            })
            .build()
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn native_menu(app: &AppHandle) -> Result<Menu<tauri::Wry>, String> {
    let settings = MenuItem::with_id(
        app,
        "gateway-connection",
        "连接设置 / Connection",
        true,
        Some("CmdOrCtrl+Shift+G"),
    )
    .map_err(|error| error.to_string())?;
    let quit = MenuItem::with_id(
        app,
        "gateway-quit",
        "退出 / Quit",
        true,
        Some("CmdOrCtrl+Q"),
    )
    .map_err(|error| error.to_string())?;
    Menu::with_items(app, &[&settings, &quit]).map_err(|error| error.to_string())
}
