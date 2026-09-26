import unittest
from pathlib import Path

from gateway_desktop_source import read_styles


GATEWAY_ROOT = Path(__file__).resolve().parents[2]
DESKTOP_ROOT = GATEWAY_ROOT / "apps" / "desktop"


class GatewayDesktopWorkbenchContractTests(unittest.TestCase):
    def test_desktop_onboarding_checklist_is_derived_from_runtime_state(self):
        onboarding_file = DESKTOP_ROOT / "src" / "lib" / "onboarding.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        launcher_file = DESKTOP_ROOT / "src" / "features" / "launcher" / "LauncherPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        self.assertTrue(onboarding_file.exists(), "onboarding checklist helper must exist")
        onboarding_text = onboarding_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        launcher_text = launcher_file.read_text(encoding="utf-8")
        styles_text = read_styles(styles_file)

        self.assertIn("GatewayOnboardingStep", types_text)
        self.assertIn('\"done\" | \"pending\" | \"warning\"', types_text)
        self.assertIn("onboardingSteps: GatewayOnboardingStep[]", types_text)
        self.assertIn("export function buildGatewayOnboardingSteps", onboarding_text)
        self.assertIn("profileValidation.ok", onboarding_text)
        self.assertIn("pathCheck", onboarding_text)
        self.assertIn("profileNames.includes", onboarding_text)
        self.assertIn("processSnapshot.running", onboarding_text)
        self.assertIn("readyProbe?.ok", onboarding_text)
        self.assertIn("apiTestResult?.ok", onboarding_text)
        self.assertIn("buildGatewayOnboardingSteps", state_hook_text)
        self.assertIn("onboardingSteps", state_hook_text)
        self.assertIn("首次启动引导", launcher_text)
        self.assertIn("state.onboardingSteps.map", launcher_text)
        self.assertIn("nt-onboarding-list", styles_text)
        self.assertIn("nt-onboarding-step", styles_text)

    def test_desktop_profile_dirty_state_warns_about_unsaved_changes(self):
        profile_state_file = DESKTOP_ROOT / "src" / "lib" / "profileState.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayProfileState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        launcher_file = DESKTOP_ROOT / "src" / "features" / "launcher" / "LauncherPanel.tsx"
        onboarding_file = DESKTOP_ROOT / "src" / "lib" / "onboarding.ts"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        self.assertTrue(profile_state_file.exists(), "profile dirty-state helper must exist")
        profile_state_text = profile_state_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        launcher_text = launcher_file.read_text(encoding="utf-8")
        onboarding_text = onboarding_file.read_text(encoding="utf-8")
        styles_text = read_styles(styles_file)

        self.assertIn("export function normalizeGatewayProfileForComparison", profile_state_text)
        self.assertIn("export function areGatewayProfilesEqual", profile_state_text)
        self.assertIn("extraEnv", profile_state_text)
        self.assertIn("hasUnsavedProfileChanges: boolean", types_text)
        self.assertIn("savedProfileSnapshot", state_hook_text)
        self.assertIn("setSavedProfileSnapshot", state_hook_text)
        self.assertIn("areGatewayProfilesEqual(draftProfile, savedProfileSnapshot)", state_hook_text)
        self.assertIn("setSavedProfileSnapshot(profile)", state_hook_text)
        self.assertIn("setSavedProfileSnapshot(draftProfile)", state_hook_text)
        self.assertIn("hasUnsavedProfileChanges", onboarding_text)
        self.assertIn("未保存变更", config_text)
        self.assertIn("state.hasUnsavedProfileChanges", config_text)
        self.assertIn("Unsaved changes", launcher_text)
        self.assertIn("nt-dirty-badge", styles_text)

    def test_desktop_start_button_explains_unsaved_profile_auto_save(self):
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        app_file = DESKTOP_ROOT / "src" / "App.tsx"
        launcher_file = DESKTOP_ROOT / "src" / "features" / "launcher" / "LauncherPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        app_text = app_file.read_text(encoding="utf-8")
        launcher_text = launcher_file.read_text(encoding="utf-8")
        styles_text = read_styles(styles_file)

        self.assertIn("startWillSaveDraftProfile: boolean", types_text)
        self.assertIn("hasUnsavedProfileChanges && !processSnapshot.running", state_hook_text)
        self.assertIn("startWillSaveDraftProfile", state_hook_text)
        self.assertIn("启动会先保存当前草稿", app_text)
        self.assertIn("gatewayState.startWillSaveDraftProfile", app_text)
        self.assertIn("启动会先保存当前草稿", launcher_text)
        self.assertIn("state.startWillSaveDraftProfile", launcher_text)
        self.assertIn("nt-start-save-hint", styles_text)

    def test_desktop_api_test_result_is_invalidated_when_inputs_or_profile_change(self):
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        state_hook_text = state_hook_file.read_text(encoding="utf-8")

        self.assertIn("updateApiTestInputAndInvalidateResult", state_hook_text)
        self.assertIn("updateApiTestInput: updateApiTestInputAndInvalidateResult", state_hook_text)
        self.assertNotIn("updateApiTestInput: setApiTestInput", state_hook_text)
        self.assertIn("setApiTestResult(undefined)", state_hook_text)
        self.assertIn("setApiTestResult(result)", state_hook_text)
        self.assertIn("updateDraftProfileAndInvalidateDerivedState", state_hook_text)

    def test_desktop_api_test_result_records_request_context(self):
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        api_test_file = DESKTOP_ROOT / "src" / "features" / "api-test" / "ApiTestPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        api_test_text = api_test_file.read_text(encoding="utf-8")
        styles_text = read_styles(styles_file)

        for field in [
            "requestedAt: string",
            "profileName: string",
            "baseUrl: string",
            "model: string",
        ]:
            self.assertIn(field, types_text)
        self.assertIn("new Date().toISOString()", state_hook_text)
        self.assertIn("profileName: draftProfile.name", state_hook_text)
        self.assertIn("baseUrl", state_hook_text)
        self.assertIn("model: apiTestInput.model.trim()", state_hook_text)
        self.assertIn("nt-api-context", api_test_text)
        self.assertIn("Profile: {result.profileName}", api_test_text)
        self.assertIn("Base URL: {result.baseUrl}", api_test_text)
        self.assertIn("Model: {result.model}", api_test_text)
        self.assertIn("Requested: {result.requestedAt}", api_test_text)
        self.assertIn("nt-api-context", styles_text)

    def test_desktop_profile_source_changes_clear_profile_bound_derived_state(self):
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayProfileState.ts"
        desktop_state_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        desktop_state_text = desktop_state_file.read_text(encoding="utf-8")

        self.assertIn("clearProfileBoundDerivedState", state_hook_text)
        self.assertIn("setPathCheck(undefined)", state_hook_text)
        for stale_state_clear in [
            "setApiTestResult(undefined)",
            "setHealthProbe(undefined)",
            "setReadyProbe(undefined)",
            "setModelsProbe(undefined)",
        ]:
            self.assertIn(stale_state_clear, desktop_state_text)
        self.assertIn("setDraftProfileFromSource", state_hook_text)
        self.assertIn("setDraftProfileFromSource(profile)", state_hook_text)
        self.assertIn("setDraftProfileFromSource(initialProfile)", state_hook_text)
        self.assertIn("setDraftProfileFromSource(importedProfile)", state_hook_text)
        self.assertIn("setDraftProfileFromSource(profileFromTemplate)", state_hook_text)

    def test_desktop_can_copy_sanitized_diagnostics_bundle(self):
        diagnostics_file = DESKTOP_ROOT / "src" / "lib" / "diagnostics.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        logs_file = DESKTOP_ROOT / "src" / "features" / "logs" / "LogsPanel.tsx"

        self.assertTrue(diagnostics_file.exists(), "diagnostics helper must exist")
        diagnostics_text = diagnostics_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        logs_text = logs_file.read_text(encoding="utf-8")

        self.assertIn("export function buildGatewayDiagnosticsReport", diagnostics_text)
        self.assertIn("function maskSensitiveValue", diagnostics_text)
        self.assertIn("SENSITIVE_ENV_KEY_PATTERN", diagnostics_text)
        self.assertIn("GATEWAY_REDIS_URL", diagnostics_text)
        self.assertIn("recent log tail", diagnostics_text)
        self.assertIn("copyDiagnostics: () => Promise<void>", types_text)
        self.assertIn("navigator.clipboard.writeText", state_hook_text)
        self.assertIn("buildGatewayDiagnosticsReport", state_hook_text)
        self.assertIn("复制诊断", logs_text)
        self.assertIn("state.copyDiagnostics", logs_text)

    def test_desktop_diagnostics_include_api_test_context_summary(self):
        diagnostics_file = DESKTOP_ROOT / "src" / "lib" / "diagnostics.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"

        diagnostics_text = diagnostics_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")

        self.assertIn("GatewayApiTestResult", diagnostics_text)
        self.assertIn("apiTestResult?: GatewayApiTestResult", diagnostics_text)
        self.assertIn("function formatApiTestResult", diagnostics_text)
        self.assertIn('section("api test"', diagnostics_text)
        for required in [
            "apiTest.ok",
            "apiTest.status",
            "apiTest.durationMs",
            "apiTest.profileName",
            "apiTest.baseUrl",
            "apiTest.model",
            "apiTest.requestedAt",
            "apiTest.endpoint",
        ]:
            self.assertIn(required, diagnostics_text)
        self.assertIn("apiTestResult", state_hook_text)

    def test_desktop_diagnostics_truncate_long_errors_and_log_tail(self):
        diagnostics_file = DESKTOP_ROOT / "src" / "lib" / "diagnostics.ts"
        diagnostics_text = diagnostics_file.read_text(encoding="utf-8")

        for required in [
            "const DIAGNOSTIC_VALUE_MAX_CHARS",
            "const DIAGNOSTIC_LOG_LINE_MAX_CHARS",
            "const DIAGNOSTIC_LOG_LINE_LIMIT",
            "function truncateDiagnosticValue",
            "function formatRecentLogTail",
            "slice(-DIAGNOSTIC_LOG_LINE_LIMIT)",
            "DIAGNOSTIC_LOG_LINE_MAX_CHARS",
            "[truncated",
            "probe.error",
            "apiTestResult.error",
            "snapshot.lastError",
        ]:
            self.assertIn(required, diagnostics_text)

    def test_desktop_log_panel_opens_log_directory_and_copies_log_path(self):
        logs_backend_file = DESKTOP_ROOT / "src-tauri" / "src" / "logs.rs"
        tauri_lib_file = DESKTOP_ROOT / "src-tauri" / "src" / "lib.rs"
        tauri_frontend_file = DESKTOP_ROOT / "src" / "lib" / "tauri.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        logs_file = DESKTOP_ROOT / "src" / "features" / "logs" / "LogsPanel.tsx"

        logs_backend_text = logs_backend_file.read_text(encoding="utf-8")
        tauri_lib_text = tauri_lib_file.read_text(encoding="utf-8")
        tauri_frontend_text = tauri_frontend_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        logs_text = logs_file.read_text(encoding="utf-8")

        self.assertIn("open_gateway_log_directory", logs_backend_text)
        self.assertIn("gateway_log_dir", logs_backend_text)
        self.assertIn("explorer", logs_backend_text)
        self.assertIn("open_gateway_log_directory", tauri_lib_text)
        self.assertIn("openGatewayLogDirectory", tauri_frontend_text)
        self.assertIn("openLogDirectory: () => Promise<void>", types_text)
        self.assertIn("copyCurrentLogPath: () => Promise<void>", types_text)
        self.assertIn("openGatewayLogDirectory", state_hook_text)
        self.assertIn("const currentLogPath", state_hook_text)
        self.assertIn("复制日志路径", logs_text)
        self.assertIn("打开日志目录", logs_text)
        self.assertIn("state.copyCurrentLogPath", logs_text)
        self.assertIn("state.openLogDirectory", logs_text)

    def test_desktop_diagnostics_copy_fallback_keeps_report_visible(self):
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        logs_file = DESKTOP_ROOT / "src" / "features" / "logs" / "LogsPanel.tsx"

        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        logs_text = logs_file.read_text(encoding="utf-8")

        self.assertIn("diagnosticsReportText?: string", types_text)
        self.assertIn("setDiagnosticsReportText(report)", state_hook_text)
        self.assertIn("!navigator.clipboard?.writeText", state_hook_text)
        self.assertIn("诊断信息已生成在日志面板", state_hook_text)
        self.assertIn("state.diagnosticsReportText", logs_text)
        self.assertIn("nt-diagnostics-report", logs_text)
        self.assertIn("脱敏诊断文本", logs_text)

    def test_browser_preview_disables_local_runtime_actions(self):
        app_text = (DESKTOP_ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
        launcher_text = (
            DESKTOP_ROOT / "src" / "features" / "launcher" / "LauncherPanel.tsx"
        ).read_text(encoding="utf-8")
        config_text = (
            DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        ).read_text(encoding="utf-8")
        logs_text = (
            DESKTOP_ROOT / "src" / "features" / "logs" / "LogsPanel.tsx"
        ).read_text(encoding="utf-8")

        self.assertIn("浏览器预览模式", app_text)
        self.assertIn("!gatewayState.isTauriAvailable", app_text)
        self.assertIn("!state.isTauriAvailable", launcher_text)
        self.assertIn("!state.isTauriAvailable", config_text)
        self.assertIn("!state.isTauriAvailable", logs_text)

    def test_desktop_readiness_ui_surfaces_degraded_mode(self):
        app_text = (DESKTOP_ROOT / "src" / "App.tsx").read_text(encoding="utf-8")
        status_text = (
            DESKTOP_ROOT / "src" / "features" / "status" / "StatusPanel.tsx"
        ).read_text(encoding="utf-8")
        types_text = (DESKTOP_ROOT / "src" / "lib" / "types.ts").read_text(encoding="utf-8")

        self.assertIn("degraded?: boolean", types_text)
        self.assertIn("readyProbe?.data?.degraded", app_text)
        self.assertIn("DEGRADED", app_text)
        self.assertIn("degradedFlag(probe.data)", status_text)
        self.assertIn("degraded mode", status_text)


if __name__ == "__main__":
    unittest.main()
