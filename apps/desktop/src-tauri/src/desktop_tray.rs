use tauri::{
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager,
};

pub fn restore(app: &AppHandle) {
    let window = app
        .get_window("gateway-console")
        .or_else(|| app.get_window("main"));
    if let Some(window) = window {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}
pub fn setup(app: &AppHandle) -> tauri::Result<()> {
    let show = MenuItem::with_id(
        app,
        "gateway-show",
        "打开 Gateway / Open",
        true,
        None::<&str>,
    )?;
    let settings = MenuItem::with_id(
        app,
        "gateway-settings",
        "设置 / Settings",
        true,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "gateway-exit", "退出 / Quit", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &settings, &quit])?;
    let mut tray = TrayIconBuilder::with_id("gateway-tray")
        .tooltip("Neuro Gateway")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "gateway-show" => restore(app),
            "gateway-settings" => crate::connection::window::show_settings(app),
            "gateway-exit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if matches!(
                event,
                TrayIconEvent::Click {
                    button: MouseButton::Left,
                    button_state: MouseButtonState::Up,
                    ..
                }
            ) {
                restore(tray.app_handle());
            }
        });
    if let Some(icon) = app.default_window_icon() {
        tray = tray.icon(icon.clone());
    }
    tray.build(app)?;
    Ok(())
}
