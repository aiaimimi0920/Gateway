import pathlib
import re
import unittest

from gateway_desktop_source import read_native_module, read_styles


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
DESKTOP_ROOT = GATEWAY_ROOT / "apps" / "desktop"


class GatewayDesktopProductContractTests(unittest.TestCase):
    def read(self, relative: str) -> str:
        return (DESKTOP_ROOT / relative).read_text(encoding="utf-8")

    def test_profile_runtime_role_and_management_token_are_editable_and_validated(self):
        types_text = self.read("src/lib/types.ts")
        validation_text = self.read("src/lib/profileValidation.ts")
        templates_text = self.read("src/lib/profileTemplates.ts")
        config_text = self.read("src/features/config/ConfigPanel.tsx")

        self.assertIn('export type GatewayRuntimeRole = "splitter" | "worker" | "standalone"', types_text)
        self.assertIn("gatewayManagementToken", types_text)
        self.assertIn("GATEWAY_RUNTIME_ROLE", validation_text)
        self.assertIn("GATEWAY_MANAGEMENT_TOKEN", validation_text)
        self.assertIn("runtimeRole", templates_text)
        self.assertIn("gatewayManagementToken", config_text)

    def test_path_check_surfaces_dependency_preflight_and_shutdown_state(self):
        types_text = self.read("src/lib/types.ts")
        config_text = self.read("src/features/config/ConfigPanel.tsx")
        status_text = self.read("src/features/status/StatusPanel.tsx")
        state_text = self.read("src/state/useGatewayDesktopState.ts")
        process_text = read_native_module(DESKTOP_ROOT / "src-tauri/src/process.rs")

        self.assertIn("preflightOk", types_text)
        self.assertIn("redis", types_text)
        self.assertIn("database", types_text)
        self.assertIn("preflightMessages", config_text)
        self.assertIn("shutdownState", types_text)
        self.assertIn("shutdownState", status_text)
        self.assertIn('snapshot.shutdownState === "forced"', state_text)
        self.assertIn("GATEWAY_MANAGEMENT_TOKEN", process_text)
        self.assertIn("x-management-token", process_text)

    def test_profile_transfer_does_not_export_management_token(self):
        transfer_text = self.read("src/lib/profileTransfer.ts")
        self.assertIn("gatewayManagementToken: REDACTED_PROFILE_VALUE", transfer_text)

    def test_desktop_shell_has_a_usable_mobile_layout_and_non_overlapping_brand(self):
        app_text = self.read("src/App.tsx")
        shell_text = self.read("src/features/shell/AppShell.tsx")
        styles_text = read_styles(DESKTOP_ROOT / "src/styles.css")

        self.assertIn('from "./features/shell/AppShell"', app_text)
        self.assertIn("<AppShell<LauncherSectionId>", app_text)
        self.assertIn('className="nt-brand__name"', shell_text)
        brand = re.search(r"(?m)^\.nt-brand__name\s*\{([^}]*)\}", styles_text)
        self.assertIsNotNone(brand)
        for declaration in ["min-width: 0", "overflow: hidden", "text-overflow: ellipsis"]:
            self.assertIn(declaration, brand.group(1))
        self.assertIn("@media (max-width: 720px)", styles_text)
        self.assertIn("grid-template-columns: minmax(0, 1fr);", styles_text)
        # The final narrow-screen override uses scrollable tabs, not the old grid.
        mobile = styles_text.rsplit("@media (max-width: 720px)", 1)[1]
        # Match the bare selector, not the preceding console-only grid placement.
        nav = re.search(r"(?m)^[ \t]*\.nt-rail__nav\s*\{([^}]*)\}", mobile)
        self.assertIsNotNone(nav)
        for declaration in ["display: flex", "overflow-x: auto", "overflow-y: hidden"]:
            self.assertIn(declaration, nav.group(1))
        self.assertIn("overflow-x: hidden;", styles_text)
        self.assertIn("flex-direction: column;", styles_text)

    def test_desktop_process_isolation_and_tree_lifecycle_contract(self):
        process_text = read_native_module(DESKTOP_ROOT / "src-tauri/src/process.rs")
        state_text = self.read("src-tauri/src/state.rs")
        profile_text = read_native_module(DESKTOP_ROOT / "src-tauri/src/profile.rs")
        logs_text = self.read("src-tauri/src/logs.rs")

        for required in [
            "env_remove",
            "terminate_process_tree",
            "taskkill",
            '"/T"',
            "status.success()",
            "read_log_tail_lines",
        ]:
            self.assertIn(required, process_text)
        self.assertIn("impl Drop for GatewayDesktopState", state_text)
        self.assertIn("target", profile_text)
        self.assertIn("debug", profile_text)
        self.assertIn("release", profile_text)
        self.assertIn("SeekFrom::End", logs_text)
        self.assertNotIn("desktop-managed redis", process_text.lower())
        self.assertNotIn("docker_executable", process_text)

    def test_desktop_redaction_fails_closed_for_malformed_urls_and_runtime_logs(self):
        diagnostics_text = self.read("src/lib/diagnostics.ts")
        transfer_text = self.read("src/lib/profileTransfer.ts")
        logs_text = self.read("src-tauri/src/logs.rs")

        self.assertIn("return REDACTED;", diagnostics_text)
        self.assertIn("return REDACTED_PROFILE_VALUE;", transfer_text)
        self.assertIn("scrub_runtime_log_line", logs_text)
        self.assertIn("GATEWAY_MANAGEMENT_TOKEN", logs_text)


if __name__ == "__main__":
    unittest.main()
