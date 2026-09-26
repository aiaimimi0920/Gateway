import json
import re
import unittest
from pathlib import Path

from gateway_desktop_source import read_styles


REPO_ROOT = Path(__file__).resolve().parents[2]
GATEWAY_ROOT = REPO_ROOT
DESKTOP_ROOT = GATEWAY_ROOT / "apps" / "desktop"


class GatewayDesktopUiContractTests(unittest.TestCase):
    def test_desktop_backend_runtime_modules_exist(self):
        required_paths = [
            "src-tauri/src/profile.rs",
            "src-tauri/src/process.rs",
            "src-tauri/src/logs.rs",
            "src-tauri/src/paths.rs",
            "src-tauri/src/state.rs",
        ]
        for required_path in required_paths:
            self.assertTrue(
                (DESKTOP_ROOT / required_path).exists(),
                f"{required_path} must exist for Gateway desktop runtime management",
            )

    def test_desktop_frontend_feature_modules_exist(self):
        required_paths = [
            "src/lib/types.ts",
            "src/lib/tauri.ts",
            "src/lib/http.ts",
            "src/lib/profileValidation.ts",
            "src/state/useGatewayDesktopState.ts",
            "src/features/launcher",
            "src/features/config",
            "src/features/status",
            "src/features/models",
            "src/features/api-test",
            "src/features/logs",
        ]
        for required_path in required_paths:
            self.assertTrue(
                (DESKTOP_ROOT / required_path).exists(),
                f"{required_path} must exist for Gateway desktop phase-1 UI",
            )

    def test_desktop_project_declares_tauri_react_rsbuild_stack(self):
        package_json = DESKTOP_ROOT / "package.json"
        self.assertTrue(package_json.exists(), "Gateway desktop package.json must exist")
        package = json.loads(package_json.read_text(encoding="utf-8"))

        self.assertEqual(package.get("name"), "gateway-ui")
        scripts = package.get("scripts", {})
        for script_name in ["build", "typecheck", "tauri"]:
            self.assertIn(script_name, scripts)
        deps = package.get("dependencies", {})
        dev_deps = package.get("devDependencies", {})
        self.assertIn("@tauri-apps/api", deps)
        self.assertEqual(deps.get("react"), "19.2.7")
        self.assertEqual(deps.get("react-dom"), "19.2.7")
        self.assertEqual(deps.get("react-router"), "8.3.0")
        self.assertNotIn("react-router-dom", deps)
        self.assertEqual(package.get("engines", {}).get("node"), ">=22.22.0")
        self.assertIn("@rsbuild/core", dev_deps)
        self.assertIn("@tauri-apps/cli", dev_deps)
        self.assertIn("typescript", dev_deps)

    def test_desktop_tauri_binary_is_named_gateway_ui(self):
        cargo_toml = DESKTOP_ROOT / "src-tauri" / "Cargo.toml"
        tauri_conf = DESKTOP_ROOT / "src-tauri" / "tauri.conf.json"
        self.assertTrue(cargo_toml.exists(), "Tauri Cargo.toml must exist")
        self.assertTrue(tauri_conf.exists(), "tauri.conf.json must exist")

        cargo_text = cargo_toml.read_text(encoding="utf-8")
        self.assertRegex(cargo_text, r'name\s*=\s*"gateway-ui"')
        self.assertIn('tauri = { version = "=2.11.2"', cargo_text)
        self.assertIn('reqwest = { version = "0.12"', cargo_text)

        config = json.loads(tauri_conf.read_text(encoding="utf-8"))
        self.assertEqual(config["productName"], "Gateway UI")
        self.assertEqual(config["identifier"], "com.vmjcv.neuro.gateway")
        self.assertEqual(config["build"]["frontendDist"], "../dist/tauri")

    def test_tauri_bundle_declares_cross_platform_icons(self):
        tauri_root = DESKTOP_ROOT / "src-tauri"
        config = json.loads(
            (tauri_root / "tauri.conf.json").read_text(encoding="utf-8")
        )
        icons = config.get("bundle", {}).get("icon", [])

        self.assertIn("icons/icon.png", icons)
        self.assertIn("icons/icon.ico", icons)
        for icon_name in ("icon.png", "icon.ico"):
            icon_path = tauri_root / "icons" / icon_name
            self.assertTrue(icon_path.is_file(), f"missing Tauri icon: {icon_name}")

        png_signature = (tauri_root / "icons" / "icon.png").read_bytes()[:8]
        self.assertEqual(
            b"\x89PNG\r\n\x1a\n",
            png_signature,
            "Gateway Linux packaging requires a valid PNG icon",
        )

    def test_desktop_uses_neuroterminal_theme_tokens(self):
        styles = DESKTOP_ROOT / "src" / "styles.css"
        self.assertTrue(styles.exists(), "Gateway desktop styles.css must exist")
        css = read_styles(styles)
        for required in [
            "--nt-color-signal-yellow: #d9ff38",
            "--nt-color-signal-green: #22c55e",
            "--nt-color-info-blue: #06b6d4",
            "--nt-color-danger-red: #f43f5e",
            "--nt-signal: var(--nt-color-signal-yellow)",
            "--nt-cyan: var(--nt-color-info-blue)",
            "--nt-graphite: var(--nt-color-background)",
            "--nt-canvas: var(--nt-color-background)",
        ]:
            self.assertIn(required, css)
        for selector in [".nt-shell", ".nt-rail", ".nt-board", ".nt-btn", ".nt-input", ".nt-brand__name"]:
            self.assertIn(selector, css)
        shell = (DESKTOP_ROOT / "src/features/shell/AppShell.tsx").read_text(encoding="utf-8")
        self.assertIn('className="nt-brand__name"', shell)
        self.assertNotIn("linear-gradient(135deg, #8b5cf6, #d946ef", css)

    def test_browser_console_uses_global_top_right_dismissible_toasts(self):
        toast_file = DESKTOP_ROOT / "src" / "components" / "AppToast.tsx"
        toast_test_file = DESKTOP_ROOT / "src" / "components" / "AppToast.test.tsx"
        main_file = DESKTOP_ROOT / "src" / "main.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        toast_text = toast_file.read_text(encoding="utf-8")
        toast_test_text = toast_test_file.read_text(encoding="utf-8")
        main_text = main_file.read_text(encoding="utf-8")
        css = read_styles(styles_file)

        self.assertIn("createPortal", toast_text)
        self.assertIn("document.body", toast_text)
        self.assertIn("TOAST_LIFETIME_MS", toast_text)
        self.assertIn("onClick={dismiss}", toast_text)
        self.assertIn('role={role}', toast_text)
        self.assertIn("<AppToastViewport />", main_text)
        self.assertIn(".nt-toast-viewport", css)
        self.assertIn("position: fixed", css)
        self.assertIn("top: 64px", css)
        self.assertIn("right: 18px", css)
        self.assertIn("@media (prefers-reduced-motion: reduce)", css)
        self.assertIn("portals the notification viewport to the document body", toast_test_text)
        self.assertIn("preserves the longer error lifetime", toast_test_text)

    def test_account_ledger_filters_orphans_and_wires_trusted_pool_automation(self):
        browser_console_text = "\n".join(
            (DESKTOP_ROOT / "src/features/console" / owner).read_text(encoding="utf-8")
            for owner in [
                "BrowserConsoleApp.tsx",
                "useConsoleController.ts",
                "routeAccountCatalog.ts",
                "useConsoleRouteData.ts",
                "useCredentialPoolActions.ts",
            ]
        )
        view_model_text = (
            DESKTOP_ROOT
            / "src"
            / "features"
            / "console"
            / "accountManagementViewModel.ts"
        ).read_text(encoding="utf-8")

        orphan_filter = ".filter((account) => providersById.has(account.providerId))"
        self.assertIn(orphan_filter, browser_console_text)
        self.assertIn(orphan_filter, view_model_text)
        self.assertIn("getCredentialPoolAutomation", browser_console_text)
        self.assertIn("requestCredentialRefill", browser_console_text)
        self.assertIn("pruneCredentialPool", browser_console_text)
        self.assertIn("purgeCredentialArchive", browser_console_text)
        self.assertIn("automation?.driverConfigured", browser_console_text)
        self.assertIn("updateIdentityCategoryAutomationToggle", browser_console_text)
        self.assertIn("setError(message)", browser_console_text)

    def test_credential_refill_framework_wires_all_three_triggers_without_stream_secrets(self):
        refill_file = GATEWAY_ROOT / "src" / "credential_refill.rs"
        refill_route_file = (
            GATEWAY_ROOT / "src" / "http" / "routes" / "internal_credential_refill.rs"
        )
        router_file = GATEWAY_ROOT / "src" / "http" / "router.rs"
        redis_keys_file = GATEWAY_ROOT / "src" / "redis" / "keys.rs"
        account_ledger_file = (
            DESKTOP_ROOT
            / "src"
            / "features"
            / "console"
            / "AccountsLedgerWorkspace.tsx"
        )
        lifecycle_file = account_ledger_file.with_name("ProviderLifecycleBack.tsx")
        endpoints_file = account_ledger_file.with_name("ProviderStorageEndpoints.tsx")

        self.assertTrue(refill_file.is_file())
        self.assertTrue(refill_route_file.is_file())
        refill_text = "\n".join([
            refill_file.read_text(encoding="utf-8"),
            (refill_file.with_suffix("") / "creation.rs").read_text(encoding="utf-8"),
        ])
        route_text = refill_route_file.read_text(encoding="utf-8")
        router_text = "\n".join([
            router_file.read_text(encoding="utf-8"),
            (router_file.with_suffix("") / "credential_lifecycle.rs").read_text(encoding="utf-8"),
        ])
        redis_keys_text = redis_keys_file.read_text(encoding="utf-8")
        ledger_text = account_ledger_file.read_text(encoding="utf-8")
        lifecycle_text = lifecycle_file.read_text(encoding="utf-8")
        endpoints_text = endpoints_file.read_text(encoding="utf-8")

        for trigger in ["Notification", "Inquiry", "UserRequested"]:
            self.assertIn(trigger, refill_text)
        for endpoint_fragment in [
            '"/v1/internal/gateway/credential-pool-refill"',
            '"/v1/internal/gateway/credential-pool-refill/tasks"',
            '"/v1/internal/gateway/credential-pool-refill/tasks/claim"',
            '"/v1/internal/gateway/credential-pool-refill/providers/:providerId"',
            '"/v1/internal/gateway/credential-pool-refill/providers/:providerId/tasks/claim"',
            '"/v1/internal/gateway/credential-pool-refill/tasks/:taskId/renew"',
            '"/v1/internal/gateway/credential-pool-refill/tasks/:taskId/complete"',
            '"/v1/internal/gateway/credential-pool-refill/tasks/:taskId/fail"',
        ]:
            self.assertIn(endpoint_fragment, router_text)
        self.assertIn("assert_management_access", route_text)
        self.assertIn("gw:credential-pool:refill:requests", redis_keys_text)
        self.assertIn("mod creation;", refill_text)
        self.assertIn("credential_lifecycle::mount(router)", router_text)
        self.assertIn("<ProviderLifecycleBack", ledger_text)
        self.assertIn("onRequestProviderRefill={onRequestProviderRefill}", ledger_text)
        self.assertIn("<ProviderStorageEndpoints", lifecycle_text)
        self.assertIn("refill={refill}", lifecycle_text)
        self.assertIn("refill?.notificationApi", endpoints_text)
        self.assertIn("refill?.inquiryApi", endpoints_text)
        self.assertIn("!refill?.userRequestEnabled", lifecycle_text)
        self.assertIn("onRequestProviderRefill(section.providerId)", lifecycle_text)

        stream_event = re.search(
            r"redis\.call\('XADD'.*?return \{1, ARGV\[2\]\}",
            refill_text,
            flags=re.DOTALL,
        )
        self.assertIsNotNone(stream_event)
        event_text = stream_event.group(0).lower()
        for forbidden in ["api_key", "cookie", "claimtoken", "authorization"]:
            self.assertNotIn(forbidden, event_text)


if __name__ == "__main__":
    unittest.main()
