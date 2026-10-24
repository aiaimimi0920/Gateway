//! A local, capability-scoped child webview inside the console's existing native window.
use super::panel_request::Layout;
use std::sync::Mutex;
use tauri::{
    webview::PageLoadEvent, window::Color, AppHandle, Manager, Webview, WebviewBuilder, WebviewUrl,
};

pub const LABEL: &str = "gateway-local-settings";
pub enum Request {
    Show(Layout),
    Hide,
    Close,
}
#[derive(Default)]
struct Queue {
    pending: Option<Request>,
    close_pending: bool,
    running: bool,
    active: Option<Layout>,
    visible: bool,
    revision: u64,
    ready: Option<u64>,
}
impl Queue {
    fn push(&mut self, request: Request) {
        if matches!(request, Request::Close) {
            self.close_pending = true;
            self.pending = None;
        } else {
            self.pending = Some(request);
        }
    }
    fn take(&mut self) -> Option<Request> {
        if self.close_pending {
            self.close_pending = false;
            Some(Request::Close)
        } else {
            self.pending.take()
        }
    }
    fn may_reveal(&self, revision: u64) -> bool {
        self.visible
            && self.revision == revision
            && self.ready == Some(revision)
            && self.pending.is_none()
            && !self.close_pending
    }
}
#[derive(Default)]
pub struct PanelState(Mutex<Queue>);

/// Coalesce resize/navigation bursts into one worker and one latest request, never an unbounded queue.
pub fn enqueue(app: &AppHandle, request: Request) {
    let state = app.state::<PanelState>();
    let Ok(mut queue) = state.0.lock() else {
        return;
    };
    queue.push(request);
    if queue.running {
        return;
    }
    queue.running = true;
    drop(queue);
    let app = app.clone();
    // add_child synchronizes with the UI thread; never call it from on_navigation directly.
    tauri::async_runtime::spawn_blocking(move || loop {
        let state = app.state::<PanelState>();
        let request = {
            let Ok(mut queue) = state.0.lock() else {
                return;
            };
            match queue.take() {
                Some(request) => request,
                None => {
                    queue.running = false;
                    return;
                }
            }
        };
        let result = match request {
            Request::Show(layout) => show(&app, layout),
            Request::Hide => hide(&app),
            Request::Close => close(&app),
        };
        if result.is_err() {
            let _ = close(&app);
            notify(
                &app,
                "window.dispatchEvent(new Event('gateway:settings-error'))",
            );
        }
    });
}

fn notify(app: &AppHandle, script: &str) {
    if let Some(console) = app.get_webview("gateway-console") {
        let _ = console.eval(script);
    }
}

fn close(app: &AppHandle) -> Result<(), String> {
    hide(app)?;
    if let Some(view) = app.get_webview(LABEL) {
        view.close().map_err(|error| error.to_string())?;
    }
    let state = app.state::<PanelState>();
    let mut queue = state.0.lock().map_err(|_| "Settings lock poisoned")?;
    queue.active = None;
    queue.ready = None;
    Ok(())
}

// Keep at most one hidden local view for tab switches; actual document navigation closes it.
fn hide(app: &AppHandle) -> Result<(), String> {
    app.state::<PanelState>()
        .0
        .lock()
        .map_err(|_| "Settings lock poisoned")?
        .visible = false;
    if let Some(view) = app.get_webview(LABEL) {
        view.hide().map_err(|e| e.to_string())?;
    }
    notify(
        app,
        "window.dispatchEvent(new CustomEvent('gateway:settings-busy',{detail:false}))",
    );
    Ok(())
}

fn show(app: &AppHandle, layout: Layout) -> Result<(), String> {
    let window = app
        .get_window("gateway-console")
        .ok_or("Console window unavailable")?;
    let viewport = window
        .inner_size()
        .map_err(|e| e.to_string())?
        .to_logical(window.scale_factor().map_err(|e| e.to_string())?);
    let bounds = layout
        .bounds(viewport)
        .ok_or("Settings slot is outside the window")?;
    let (same, revision) = {
        let state = app.state::<PanelState>();
        let mut queue = state.0.lock().map_err(|_| "Settings lock poisoned")?;
        let same = queue
            .active
            .as_ref()
            .is_some_and(|active| active.same_content(&layout));
        if !same {
            queue.revision += 1;
            queue.ready = None;
        }
        queue.visible = true;
        queue.active = Some(layout.clone());
        (same, queue.revision)
    };
    if let Some(view) = app.get_webview(LABEL) {
        view.set_bounds(bounds).map_err(|e| e.to_string())?;
        if same {
            reveal(app, revision)?;
        } else {
            view.hide().map_err(|e| e.to_string())?;
            update_context(app)?;
        }
        return Ok(());
    }
    let context = context(app, &layout, revision)?;
    let script = format!("Object.defineProperty(window,'__GATEWAY_EMBEDDED_SETTINGS__',{{value:Object.freeze({context})}});if(document.documentElement)document.documentElement.dataset.ntTheme={:?};", layout.theme);
    let trusted = app
        .get_webview("main")
        .ok_or("Local assets unavailable")?
        .url()
        .map_err(|e| e.to_string())?;
    let builder = WebviewBuilder::new(LABEL, WebviewUrl::App("index.html".into()))
        .data_directory(crate::paths::gateway_app_dir()?.join("webview"))
        .initialization_script(script)
        .background_color(if layout.theme == "light" {
            Color(248, 250, 243, 255)
        } else {
            Color(9, 12, 17, 255)
        })
        // No remote document or popup can inherit this view's native capability.
        .on_navigation(move |url| {
            url.origin() == trusted.origin()
                && matches!(url.path(), "/" | "/index.html")
                && url.query().is_none()
                && url.fragment().is_none()
        })
        .on_page_load(|view, payload| {
            if matches!(payload.event(), PageLoadEvent::Finished) {
                // A category/resize request may have arrived before React installed its listener.
                let _ = update_context(view.app_handle());
            }
        })
        .on_new_window(|_, _| tauri::webview::NewWindowResponse::Deny);
    let view = window
        .add_child(builder, bounds.position, bounds.size)
        .map_err(|e| e.to_string())?;
    view.hide().map_err(|e| e.to_string())?;
    // Readiness can race add_child's return. Reconcile after the initial hide, too.
    reveal(app, revision)?;
    Ok(())
}

fn context(app: &AppHandle, layout: &Layout, revision: u64) -> Result<serde_json::Value, String> {
    let current = app
        .state::<super::ConnectionState>()
        .origin
        .lock()
        .map_err(|_| "Connection lock poisoned")?
        .clone()
        .unwrap_or_default();
    Ok(
        serde_json::json!({"category":layout.category,"theme":layout.theme,"locale":layout.locale,"current":current,"revision":revision}),
    )
}

fn update_context(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<PanelState>();
    let (layout, revision) = {
        let queue = state.0.lock().map_err(|_| "Settings lock poisoned")?;
        (queue.active.clone(), queue.revision)
    };
    if let (Some(layout), Some(view)) = (layout, app.get_webview(LABEL)) {
        let context = context(app, &layout, revision)?;
        view.eval(format!("window.dispatchEvent(new CustomEvent('gateway:settings-context',{{detail:{context}}}))"))
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

// Recheck desired state on the UI thread: stale React readiness must not reopen a hidden panel.
fn reveal(app: &AppHandle, revision: u64) -> Result<(), String> {
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let state = handle.state::<PanelState>();
        let Ok(queue) = state.0.lock() else {
            return;
        };
        if queue.may_reveal(revision) {
            if let Some(view) = handle.get_webview(LABEL) {
                if view.show().is_ok() {
                    notify(
                        &handle,
                        "window.dispatchEvent(new Event('gateway:settings-ready'))",
                    );
                }
            }
        }
    })
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gateway_settings_ready(
    app: AppHandle,
    webview: Webview,
    revision: u64,
) -> Result<(), String> {
    if webview.label() != LABEL {
        return Err("Only local settings may report readiness".into());
    }
    {
        let state = app.state::<PanelState>();
        let mut queue = state.0.lock().map_err(|_| "Settings lock poisoned")?;
        if queue.active.is_none() {
            return Ok(());
        }
        if queue.revision != revision {
            drop(queue);
            // The child listener is now installed: replay an update lost during initial loading.
            return update_context(&app);
        }
        queue.ready = Some(revision);
    }
    reveal(&app, revision)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn document_close_is_a_barrier_not_overwritten_by_layout_bursts() {
        let mut queue = Queue::default();
        queue.push(Request::Hide);
        queue.push(Request::Close);
        queue.push(Request::Hide);
        assert!(matches!(queue.take(), Some(Request::Close)));
        assert!(matches!(queue.take(), Some(Request::Hide)));
        assert!(queue.take().is_none());
    }
    #[test]
    fn readiness_never_reopens_hidden_stale_or_pending_content() {
        let mut queue = Queue {
            visible: true,
            revision: 3,
            ready: Some(3),
            ..Queue::default()
        };
        assert!(queue.may_reveal(3));
        assert!(!queue.may_reveal(2));
        queue.visible = false;
        assert!(!queue.may_reveal(3));
        queue.visible = true;
        queue.ready = None;
        assert!(!queue.may_reveal(3));
        queue.ready = Some(3);
        queue.push(Request::Close);
        assert!(!queue.may_reveal(3));
        queue.take();
        queue.push(Request::Hide);
        assert!(!queue.may_reveal(3));
    }
}

#[tauri::command]
pub fn set_gateway_settings_busy(
    app: AppHandle,
    webview: Webview,
    busy: bool,
    revision: u64,
) -> Result<(), String> {
    if webview.label() != LABEL {
        return Err("Only the local settings view may report its state".into());
    }
    let state = app.state::<PanelState>();
    let queue = state.0.lock().map_err(|_| "Settings lock poisoned")?;
    if !queue.visible || queue.revision != revision || queue.close_pending {
        return Ok(());
    }
    drop(queue);
    notify(
        &app,
        if busy {
            "window.dispatchEvent(new CustomEvent('gateway:settings-busy',{detail:true}))"
        } else {
            "window.dispatchEvent(new CustomEvent('gateway:settings-busy',{detail:false}))"
        },
    );
    Ok(())
}
