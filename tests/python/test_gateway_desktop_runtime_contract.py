import unittest
from pathlib import Path

from gateway_desktop_source import read_native_module, read_styles


GATEWAY_ROOT = Path(__file__).resolve().parents[2]
DESKTOP_ROOT = GATEWAY_ROOT / "apps" / "desktop"


class GatewayDesktopRuntimeContractTests(unittest.TestCase):
    def test_desktop_state_auto_starts_gateway_when_backend_health_is_missing(self):
        state_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        state_text = state_file.read_text(encoding="utf-8")

        self.assertIn("autoStartAttempted", state_text)
        self.assertIn("setAutoStartAttempted(false)", state_text)
        self.assertIn("!healthProbe.ok", state_text)
        self.assertIn("void startGateway()", state_text)
        self.assertIn("未检测到 Gateway 后端，正在自动启动 gateway.exe", state_text)

    def test_desktop_state_supports_explicit_profile_reload_preference(self):
        state_file = DESKTOP_ROOT / "src" / "state" / "useGatewayProfileState.ts"
        state_text = state_file.read_text(encoding="utf-8")
        self.assertIn("reloadProfiles = useCallback(async (preferredProfileName?: string) =>", state_text)
        self.assertIn("preferredProfileName?.trim()", state_text)
        self.assertIn("await reloadProfiles(draftProfile.name)", state_text)
        self.assertIn("await reloadProfiles(DEFAULT_PROFILE_NAME)", state_text)

    def test_desktop_process_waits_for_graceful_shutdown_before_kill(self):
        process_file = DESKTOP_ROOT / "src-tauri" / "src" / "process.rs"
        process_text = read_native_module(process_file)
        self.assertIn("const GRACEFUL_SHUTDOWN_TIMEOUT_MS", process_text)
        self.assertIn("const GRACEFUL_SHUTDOWN_POLL_INTERVAL_MS", process_text)
        self.assertIn("fn wait_for_graceful_exit", process_text)
        self.assertIn("std::thread::sleep", process_text)
        self.assertIn("fn shutdown_state_for_exit", process_text)
        self.assertIn("status.success()", process_text)
        self.assertIn("terminate_process_tree", process_text)

    def test_desktop_process_waits_for_startup_probe_and_reports_fast_exit_logs(self):
        process_file = DESKTOP_ROOT / "src-tauri" / "src" / "process.rs"
        state_file = DESKTOP_ROOT / "src-tauri" / "src" / "state.rs"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        launcher_file = DESKTOP_ROOT / "src" / "features" / "launcher" / "LauncherPanel.tsx"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"

        process_text = read_native_module(process_file)
        state_text = state_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        launcher_text = launcher_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")

        self.assertIn("const STARTUP_PROBE_TIMEOUT_MS", process_text)
        self.assertIn("fn ensure_port_available", process_text)
        self.assertIn("std::net::TcpStream::connect", process_text)
        self.assertIn("fn wait_for_startup_probe", process_text)
        self.assertIn("/healthz", process_text)
        self.assertIn("/readyz", process_text)
        self.assertIn("tail_log_lines", process_text)
        self.assertIn('startup_state: "exited"', process_text)
        self.assertIn("pub startup_state: Option<String>", state_text)
        self.assertIn("pub last_error: Option<String>", state_text)
        self.assertIn("pub recent_log_lines: Vec<String>", state_text)
        self.assertIn("startupState?: string | null", types_text)
        self.assertIn("lastError?: string | null", types_text)
        self.assertIn("recentLogLines: string[]", types_text)
        self.assertIn("snapshot.startupState", state_hook_text)
        self.assertIn("recentLogLines", launcher_text)

    def test_desktop_profile_validation_blocks_bad_config_before_runtime_actions(self):
        validation_file = DESKTOP_ROOT / "src" / "lib" / "profileValidation.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayProfileState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        launcher_file = DESKTOP_ROOT / "src" / "features" / "launcher" / "LauncherPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        self.assertTrue(validation_file.exists(), "profile validation helper must exist")
        validation_text = validation_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        launcher_text = launcher_file.read_text(encoding="utf-8")
        styles_text = read_styles(styles_file)

        self.assertIn("export function validateGatewayProfile", validation_text)
        self.assertIn("profile.name.trim()", validation_text)
        self.assertIn("profile.port < 1 || profile.port > 65535", validation_text)
        self.assertIn("redis://", validation_text)
        self.assertIn("rediss://", validation_text)
        self.assertIn("ENV_KEY_PATTERN", validation_text)
        self.assertIn("RESERVED_ENV_KEYS", validation_text)
        self.assertIn("GatewayProfileValidation", types_text)
        self.assertIn("profileValidation: GatewayProfileValidation", types_text)
        self.assertIn("canSaveProfile: boolean", types_text)
        self.assertIn("canStartGateway: boolean", types_text)
        self.assertIn("validateGatewayProfile(draftProfile)", state_hook_text)
        self.assertIn("state.profileValidation.errors.length", config_text)
        self.assertIn("aria-invalid", config_text)
        self.assertIn("!state.canSaveProfile", config_text)
        self.assertIn("!state.canStartGateway", launcher_text)
        self.assertIn(".nt-validation-list", styles_text)

    def test_desktop_profile_validation_reports_extra_env_errors_by_row(self):
        validation_file = DESKTOP_ROOT / "src" / "lib" / "profileValidation.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"

        validation_text = validation_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")

        self.assertIn("extraEnvErrors: Record<number, string>", types_text)
        self.assertIn("pushExtraEnvError", validation_text)
        self.assertIn("validation.extraEnvErrors[index]", validation_text)
        self.assertIn("const envRowError = validation.extraEnvErrors[index]", config_text)
        self.assertIn("aria-invalid={Boolean(envRowError)}", config_text)
        self.assertIn("{envRowError ? <small>{envRowError}</small> : null}", config_text)
        self.assertNotIn("error.includes(entry.key)", config_text)

    def test_desktop_profile_import_export_uses_sanitized_json_transfer(self):
        transfer_file = DESKTOP_ROOT / "src" / "lib" / "profileTransfer.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayProfileState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        self.assertTrue(transfer_file.exists(), "profile transfer helper must exist")
        transfer_text = transfer_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        styles_text = read_styles(styles_file)

        self.assertIn("export function buildGatewayProfileExportText", transfer_text)
        self.assertIn("export function parseGatewayProfileTransferText", transfer_text)
        self.assertIn("SENSITIVE_ENV_KEY_PATTERN", transfer_text)
        self.assertIn("sanitizeProfileForExport", transfer_text)
        self.assertIn("schemaVersion", transfer_text)
        self.assertIn("gateway-ui-profile", transfer_text)
        self.assertIn("GatewayProfileTransferPayload", types_text)
        self.assertIn("profileTransferText: string", types_text)
        self.assertIn("importProfileText: string", types_text)
        self.assertIn("exportDraftProfile: () => Promise<void>", types_text)
        self.assertIn("importDraftProfile: () => Promise<void>", types_text)
        self.assertIn("updateImportProfileText: (value: string) => void", types_text)
        self.assertIn("buildGatewayProfileExportText", state_hook_text)
        self.assertIn("parseGatewayProfileTransferText", state_hook_text)
        self.assertIn("navigator.clipboard.writeText(exportText)", state_hook_text)
        self.assertIn("setProfileTransferText(exportText)", state_hook_text)
        self.assertIn("state.exportDraftProfile", config_text)
        self.assertIn("state.importDraftProfile", config_text)
        self.assertIn("state.updateImportProfileText", config_text)
        self.assertIn("导出脱敏 JSON", config_text)
        self.assertIn("导入到草稿", config_text)
        self.assertIn("nt-transfer-box", styles_text)

    def test_desktop_profile_path_precheck_uses_tauri_backend_resolution(self):
        profile_file = DESKTOP_ROOT / "src-tauri" / "src" / "profile.rs"
        tauri_lib_file = DESKTOP_ROOT / "src-tauri" / "src" / "lib.rs"
        tauri_frontend_file = DESKTOP_ROOT / "src" / "lib" / "tauri.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayProfileState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        profile_text = read_native_module(profile_file)
        tauri_lib_text = tauri_lib_file.read_text(encoding="utf-8")
        tauri_frontend_text = tauri_frontend_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        styles_text = read_styles(styles_file)

        self.assertIn("GatewayProfilePathCheck", profile_text)
        self.assertIn("GatewayProfilePathCheckItem", profile_text)
        self.assertIn("check_gateway_profile_paths", profile_text)
        self.assertIn("profile_working_directory_path(profile)", profile_text)
        self.assertIn("resolve_profile_file_path(profile, routes_file)", profile_text)
        self.assertIn("is_file", profile_text)
        self.assertIn("is_dir", profile_text)
        self.assertIn("check_gateway_profile_paths", tauri_lib_text)
        self.assertIn("checkGatewayProfilePaths", tauri_frontend_text)
        self.assertIn("GatewayProfilePathCheck", types_text)
        self.assertIn("GatewayProfilePathCheckItem", types_text)
        self.assertIn("pathCheck?: GatewayProfilePathCheck", types_text)
        self.assertIn("checkProfilePaths: () => Promise<void>", types_text)
        self.assertIn("checkGatewayProfilePaths(draftProfile)", state_hook_text)
        self.assertIn("setPathCheck", state_hook_text)
        self.assertIn("state.checkProfilePaths", config_text)
        self.assertIn("检查路径", config_text)
        self.assertIn("renderPathCheckItem", config_text)
        self.assertIn("nt-path-check", styles_text)

    def test_desktop_profile_edits_invalidate_stale_path_check(self):
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayProfileState.ts"
        desktop_state_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        desktop_state_text = desktop_state_file.read_text(encoding="utf-8")

        self.assertIn("updateDraftProfileAndInvalidateDerivedState", state_hook_text)
        self.assertIn("setPathCheck(undefined)", state_hook_text)
        self.assertIn("updateDraftProfile: updateDraftProfileAndInvalidateDerivedState", desktop_state_text)
        self.assertNotIn("updateDraftProfile: setDraftProfile", desktop_state_text)

    def test_desktop_profile_templates_update_draft_without_saving(self):
        templates_file = DESKTOP_ROOT / "src" / "lib" / "profileTemplates.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayProfileState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        self.assertTrue(templates_file.exists(), "profile templates helper must exist")
        templates_text = templates_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        styles_text = read_styles(styles_file)

        self.assertIn("GatewayProfileTemplate", types_text)
        self.assertIn("PROFILE_TEMPLATES", templates_text)
        self.assertIn("local-default", templates_text)
        self.assertIn("routes-yaml", templates_text)
        self.assertIn("local-debug", templates_text)
        self.assertIn("createProfileFromTemplate", templates_text)
        self.assertIn("routes.yaml", templates_text)
        self.assertIn("RUST_LOG", templates_text)
        self.assertIn("applyProfileTemplate: (templateId: string) => void", types_text)
        self.assertIn("PROFILE_TEMPLATES", state_hook_text)
        self.assertIn("createProfileFromTemplate(templateId)", state_hook_text)
        self.assertIn("setPathCheck(undefined)", state_hook_text)
        self.assertNotIn("saveProfile(profileFromTemplate", state_hook_text)
        self.assertIn("Profile 快速模板", config_text)
        self.assertIn("state.applyProfileTemplate", config_text)
        self.assertIn("nt-template-grid", styles_text)
        self.assertIn("nt-template-card", styles_text)

    def test_tauri_profile_validation_rejects_invalid_paths_and_env_keys(self):
        profile_file = DESKTOP_ROOT / "src-tauri" / "src" / "profile.rs"
        profile_text = read_native_module(profile_file)

        self.assertIn("pub(crate) const DESKTOP_OWNED_ENV_KEYS", profile_text)
        self.assertIn("reserved.contains(normalized_key.as_str())", profile_text)
        self.assertIn("fn validate_env_key", profile_text)
        self.assertIn("std::collections::HashSet", profile_text)
        self.assertIn("gateway routes file does not exist", profile_text)
        self.assertIn("working directory does not exist", profile_text)
        self.assertIn("duplicate extra env key", profile_text)
        self.assertIn("reserved extra env key", profile_text)


if __name__ == "__main__":
    unittest.main()
