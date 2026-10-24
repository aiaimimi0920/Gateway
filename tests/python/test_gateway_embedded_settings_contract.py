"""Protect the same-window presentation change without granting server content native IPC."""
import json
import unittest
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2] / "apps" / "desktop"
NATIVE = ROOT / "src-tauri"


class GatewayEmbeddedSettingsContractTests(unittest.TestCase):
    def test_capability_uses_exact_webviews_without_remote_access(self):
        capability = json.loads((NATIVE / "capabilities/default.json").read_text(encoding="utf-8"))
        self.assertEqual(capability["webviews"], ["main", "gateway-local-settings"])
        self.assertNotIn("windows", capability)
        self.assertNotIn("remote", capability)

    def test_server_remains_denied_and_child_has_a_narrow_command_list(self):
        source = (NATIVE / "src/lib.rs").read_text(encoding="utf-8")
        child = source.split("== connection::panel::LABEL", 1)[1].split("return handler(invoke);", 1)[0]
        for command in ("load_gateway_connection", "save_gateway_desktop_preferences", "set_gateway_settings_busy"):
            self.assertIn(command, child)
        for command in ("save_profile", "delete_profile", "read_gateway_log_tail", "stop_gateway_sidecar", "open_gateway_log_directory", "export_gateway_settings_diagnostics"):
            self.assertNotIn(command, child)
        self.assertIn('invoke.message.webview_ref().label() != "main"', source)
        self.assertIn('.reject("Native commands are restricted to connection settings")', source)

    def test_only_backend_and_desktop_use_native_settings(self):
        parser = (NATIVE / "src/connection/panel_request.rs").read_text(encoding="utf-8")
        production = parser.split("#[cfg(test)]", 1)[0]
        self.assertNotIn('"diagnostics"', production)
        component = (ROOT / "src/features/settings/GatewayDiagnosticsSettings.tsx").read_text(encoding="utf-8")
        self.assertIn("gatewayDiagnosticsApi(host)", component)
        self.assertNotIn("invoke(", component)

    def test_settings_use_a_local_child_not_a_new_window_or_remote_frame(self):
        source = (NATIVE / "src/connection/panel.rs").read_text(encoding="utf-8")
        self.assertIn('get_window("gateway-console")', source)
        self.assertIn('.add_child(builder, bounds.position, bounds.size)', source)
        self.assertIn('WebviewUrl::App("index.html".into())', source)
        self.assertIn("url.origin() == trusted.origin()", source)
        self.assertIn("NewWindowResponse::Deny", source)
        self.assertNotIn("WebviewWindowBuilder", source)
        self.assertIn("queue.push(request)", source)
        self.assertIn("if queue.running", source)
        self.assertIn("view.close()", source)

    def test_embedded_connection_failure_stays_in_place(self):
        source = (NATIVE / "src/connection.rs").read_text(encoding="utf-8")
        self.assertIn('if result.is_err() && window.label() == "main"', source)
        self.assertIn('let recovery_window = window.label() == "main"', source)
        self.assertIn("if result.is_err() && recovery_window", source)

    def test_multi_webview_console_keeps_chrome_tray_and_oauth_owned_by_native_window(self):
        for name in ("src/desktop_tray.rs", "src/connection/chrome.rs", "src/connection/authorization_browser.rs"):
            source = (NATIVE / name).read_text(encoding="utf-8")
            self.assertNotIn('get_webview_window("gateway-console")', source)


if __name__ == "__main__":
    unittest.main()
