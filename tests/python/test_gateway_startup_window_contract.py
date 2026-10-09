"""Desktop startup must not expose an intermediate settings/empty WebView window."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "apps" / "desktop" / "src-tauri"


class GatewayStartupWindowContractTests(unittest.TestCase):
    def test_settings_start_hidden_with_native_dark_background(self):
        config = json.loads((ROOT / "tauri.conf.json").read_text(encoding="utf-8"))
        window = config["app"]["windows"][0]
        self.assertEqual(window["label"], "main")
        self.assertFalse(window["create"])
        self.assertFalse(window["visible"])
        self.assertEqual(window["backgroundColor"], "#06080d")

    def test_console_waits_for_finished_allowed_navigation_before_show(self):
        source = (ROOT / "src/connection/window.rs").read_text(encoding="utf-8")
        self.assertIn(".visible(false)", source)
        self.assertIn(".background_color(Color(6, 8, 13, 255))", source)
        callback = source.split(".on_page_load(", 1)[1].split(".menu(menu)", 1)[0]
        self.assertIn("PageLoadEvent::Finished", callback)
        self.assertIn("payload.url().origin().ascii_serialization()", callback)
        self.assertLess(callback.index("if !allowed"), callback.index("window.show()"))
        self.assertLess(callback.index("window.show()"), callback.index("settings.hide()"))
        self.assertIn("!window.is_visible()", callback)

    def test_reconnect_hides_console_before_navigation_without_early_show(self):
        source = (ROOT / "src/connection/window.rs").read_text(encoding="utf-8")
        reuse = source.split('if let Some(window) = app.get_webview_window("gateway-console")', 1)[1].split("} else {", 1)[0]
        self.assertLess(reuse.index("window.hide()"), reuse.index("window.navigate(url)"))
        self.assertNotIn("window.show()", reuse)

    def test_settings_load_and_connection_failures_reveal_recovery_window(self):
        source = (ROOT / "src/connection.rs").read_text(encoding="utf-8")
        load = source.split("pub fn load_gateway_connection", 1)[1].split("#[tauri::command]", 1)[0]
        connect = source.split("pub async fn connect_gateway", 1)[1].split("fn connect(", 1)[0]
        for handler in (load, connect):
            self.assertIn("if result.is_err()", handler)
            self.assertIn("window::show_settings", handler)
        self.assertIn("require_settings_window(&window)?", connect)

    def test_closing_recovery_exits_when_console_has_not_become_visible(self):
        source = (ROOT / "src/connection.rs").read_text(encoding="utf-8")
        close = source.split("pub fn handle_window_event", 1)[1]
        self.assertIn("console.is_visible().unwrap_or(false)", close)
        self.assertIn("window.app_handle().exit(0)", close)


if __name__ == "__main__":
    unittest.main()
