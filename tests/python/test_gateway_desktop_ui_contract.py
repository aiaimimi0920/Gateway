import json
import re
import unittest
from pathlib import Path


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

        self.assertEqual(package.get("name"), "neuro-gateway-ui")
        scripts = package.get("scripts", {})
        for script_name in ["build", "typecheck", "tauri"]:
            self.assertIn(script_name, scripts)
        deps = package.get("dependencies", {})
        dev_deps = package.get("devDependencies", {})
        self.assertIn("@tauri-apps/api", deps)
        self.assertIn("react", deps)
        self.assertIn("react-dom", deps)
        self.assertIn("@rsbuild/core", dev_deps)
        self.assertIn("@tauri-apps/cli", dev_deps)
        self.assertIn("typescript", dev_deps)

    def test_desktop_tauri_binary_is_named_neuro_gateway_ui(self):
        cargo_toml = DESKTOP_ROOT / "src-tauri" / "Cargo.toml"
        tauri_conf = DESKTOP_ROOT / "src-tauri" / "tauri.conf.json"
        self.assertTrue(cargo_toml.exists(), "Tauri Cargo.toml must exist")
        self.assertTrue(tauri_conf.exists(), "tauri.conf.json must exist")

        cargo_text = cargo_toml.read_text(encoding="utf-8")
        self.assertRegex(cargo_text, r'name\s*=\s*"neuro-gateway-ui"')
        self.assertIn('tauri = { version = "=2.11.2"', cargo_text)
        self.assertIn('reqwest = { version = "0.12"', cargo_text)

        config = json.loads(tauri_conf.read_text(encoding="utf-8"))
        self.assertEqual(config["productName"], "Neuro Gateway")
        self.assertEqual(config["identifier"], "com.vmjcv.neuro.gateway")
        self.assertEqual(config["build"]["frontendDist"], "../dist")

    def test_desktop_uses_neuroterminal_theme_tokens(self):
        styles = DESKTOP_ROOT / "src" / "styles.css"
        self.assertTrue(styles.exists(), "Gateway desktop styles.css must exist")
        css = styles.read_text(encoding="utf-8")
        for required in ["--nt-signal: #d9ff38", "--nt-cyan", "--nt-graphite", "--nt-canvas"]:
            self.assertIn(required, css)
        for selector in [".nt-shell", ".nt-rail", ".nt-board", ".nt-btn", ".nt-input", ".nt-kicker"]:
            self.assertIn(selector, css)
        self.assertNotIn("linear-gradient(135deg, #8b5cf6, #d946ef", css)

    def test_release_plan_includes_headless_and_ui_gateway_exes(self):
        release_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = release_script.read_text(encoding="utf-8")
        self.assertIn("cargo build --locked --release --bin neuro-gateway", script_text)
        self.assertIn('"target\\\\release\\\\neuro-gateway.exe"', script_text)
        self.assertIn(
            '"apps\\\\desktop\\\\src-tauri\\\\target\\\\release\\\\neuro-gateway-ui.exe"',
            script_text,
        )

    def test_gateway_owned_release_builder_builds_ui_and_headless(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        self.assertTrue(build_script.exists(), "Gateway-owned release builder must exist")
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("cargo build --locked --release --bin neuro-gateway", script_text)
        self.assertIn("Push-Location -LiteralPath $desktopRoot", script_text)
        self.assertIn("npm ci", script_text)
        self.assertIn("npm run typecheck", script_text)
        self.assertIn("npm run tauri -- build --no-bundle", script_text)
        self.assertIn("neuro-gateway-ui.exe", script_text)

    def test_gateway_owned_release_builder_retries_transient_npm_ci_file_locks(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("function Invoke-NpmCiWithRetry", script_text)
        self.assertIn("$maxNpmCiAttempts = 3", script_text)
        self.assertIn("EPERM", script_text)
        self.assertIn("EBUSY", script_text)
        self.assertIn("ENOTEMPTY", script_text)
        self.assertIn("Start-Sleep", script_text)
        self.assertIn("npm ci failed for Gateway desktop UI after", script_text)

    def test_gateway_owned_release_builder_has_stage_logs_and_artifact_hash_summary(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        script_text = build_script.read_text(encoding="utf-8")
        self.assertIn("function Invoke-GatewayReleaseStep", script_text)
        self.assertIn("[gateway-release] BEGIN", script_text)
        self.assertIn("[gateway-release] END", script_text)
        self.assertIn("function Write-ArtifactHashSummary", script_text)
        self.assertIn("Get-FileHash", script_text)
        self.assertIn("sha256=", script_text)

    def test_gateway_owned_ui_release_smoke_script_validates_artifacts_and_optional_launch(self):
        smoke_script = GATEWAY_ROOT / "tools" / "smoke-gateway-ui-release.ps1"
        self.assertTrue(smoke_script.exists(), "Gateway UI release smoke script must exist")
        script_text = smoke_script.read_text(encoding="utf-8")

        for required in [
            "param(",
            "$ReleaseDir",
            "$LaunchUi",
            "manifest.json",
            "checksums.sha256",
            "neuro-gateway.exe",
            "neuro-gateway-ui.exe",
            "Get-FileHash",
            "bytesMatch",
            "shaMatch",
            "Start-Process",
            "-WindowStyle Hidden",
            "Stop-Process",
            "neuro-gateway-ui",
            "neuro-gateway",
            "must not auto-start",
        ]:
            self.assertIn(required, script_text)

    def test_gateway_ui_release_smoke_only_detects_and_cleans_release_owned_sidecars(self):
        smoke_script = GATEWAY_ROOT / "tools" / "smoke-gateway-ui-release.ps1"
        script_text = smoke_script.read_text(encoding="utf-8")

        for required in [
            "function Get-ReleaseOwnedGatewayProcesses",
            "Get-CimInstance",
            "Win32_Process",
            "ExecutablePath",
            "CommandLine",
            "$expectedGatewayPath",
            "Test-PathOwnedByRelease",
            "ReleaseRoot = $ReleaseRoot",
        ]:
            self.assertIn(required, script_text)

        self.assertNotIn("function Get-ProcessIdsByName", script_text)
        self.assertNotIn('Get-Process -Name $Name', script_text)
        self.assertNotIn('Get-ProcessIdsByName -Name "neuro-gateway"', script_text)

        self.assertIn("function Read-ChecksumIndex", script_text)
        self.assertIn("function Assert-PackagedReleaseIntegrity", script_text)
        self.assertIn("newGatewayProcessIds", script_text)
        self.assertIn("Stop-Process -Id", script_text)
        self.assertIn("finally", script_text)

    def test_gateway_owned_release_builder_lets_tauri_run_frontend_build_once(self):
        build_script = GATEWAY_ROOT / "tools" / "build-gateway-release.ps1"
        tauri_conf = DESKTOP_ROOT / "src-tauri" / "tauri.conf.json"
        script_text = build_script.read_text(encoding="utf-8")
        config = json.loads(tauri_conf.read_text(encoding="utf-8"))

        self.assertEqual(config["build"]["beforeBuildCommand"], "npm run build")
        self.assertNotIn("& npm run build", script_text)
        self.assertIn("& npm run typecheck", script_text)

    def test_desktop_state_supports_explicit_profile_reload_preference(self):
        state_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        state_text = state_file.read_text(encoding="utf-8")
        self.assertIn("reloadProfiles = useCallback(async (preferredProfileName?: string) =>", state_text)
        self.assertIn("preferredProfileName?.trim()", state_text)
        self.assertIn("await reloadProfiles(draftProfile.name)", state_text)
        self.assertIn("await reloadProfiles(DEFAULT_PROFILE_NAME)", state_text)

    def test_desktop_process_waits_for_graceful_shutdown_before_kill(self):
        process_file = DESKTOP_ROOT / "src-tauri" / "src" / "process.rs"
        process_text = process_file.read_text(encoding="utf-8")
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

        process_text = process_file.read_text(encoding="utf-8")
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
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        launcher_file = DESKTOP_ROOT / "src" / "features" / "launcher" / "LauncherPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        self.assertTrue(validation_file.exists(), "profile validation helper must exist")
        validation_text = validation_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        launcher_text = launcher_file.read_text(encoding="utf-8")
        styles_text = styles_file.read_text(encoding="utf-8")

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
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        self.assertTrue(transfer_file.exists(), "profile transfer helper must exist")
        transfer_text = transfer_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        styles_text = styles_file.read_text(encoding="utf-8")

        self.assertIn("export function buildGatewayProfileExportText", transfer_text)
        self.assertIn("export function parseGatewayProfileTransferText", transfer_text)
        self.assertIn("SENSITIVE_ENV_KEY_PATTERN", transfer_text)
        self.assertIn("sanitizeProfileForExport", transfer_text)
        self.assertIn("schemaVersion", transfer_text)
        self.assertIn("neuro-gateway-ui-profile", transfer_text)
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
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        profile_text = profile_file.read_text(encoding="utf-8")
        tauri_lib_text = tauri_lib_file.read_text(encoding="utf-8")
        tauri_frontend_text = tauri_frontend_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        styles_text = styles_file.read_text(encoding="utf-8")

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
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        state_hook_text = state_hook_file.read_text(encoding="utf-8")

        self.assertIn("updateDraftProfileAndInvalidateDerivedState", state_hook_text)
        self.assertIn("setPathCheck(undefined)", state_hook_text)
        self.assertIn("updateDraftProfile: updateDraftProfileAndInvalidateDerivedState", state_hook_text)
        self.assertNotIn("updateDraftProfile: setDraftProfile", state_hook_text)

    def test_desktop_profile_templates_update_draft_without_saving(self):
        templates_file = DESKTOP_ROOT / "src" / "lib" / "profileTemplates.ts"
        types_file = DESKTOP_ROOT / "src" / "lib" / "types.ts"
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        config_file = DESKTOP_ROOT / "src" / "features" / "config" / "ConfigPanel.tsx"
        styles_file = DESKTOP_ROOT / "src" / "styles.css"

        self.assertTrue(templates_file.exists(), "profile templates helper must exist")
        templates_text = templates_file.read_text(encoding="utf-8")
        types_text = types_file.read_text(encoding="utf-8")
        state_hook_text = state_hook_file.read_text(encoding="utf-8")
        config_text = config_file.read_text(encoding="utf-8")
        styles_text = styles_file.read_text(encoding="utf-8")

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
        styles_text = styles_file.read_text(encoding="utf-8")

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
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
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
        styles_text = styles_file.read_text(encoding="utf-8")

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
        styles_text = styles_file.read_text(encoding="utf-8")

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
        styles_text = styles_file.read_text(encoding="utf-8")

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
        state_hook_file = DESKTOP_ROOT / "src" / "state" / "useGatewayDesktopState.ts"
        state_hook_text = state_hook_file.read_text(encoding="utf-8")

        self.assertIn("clearProfileBoundDerivedState", state_hook_text)
        for stale_state_clear in [
            "setPathCheck(undefined)",
            "setApiTestResult(undefined)",
            "setHealthProbe(undefined)",
            "setReadyProbe(undefined)",
            "setModelsProbe(undefined)",
        ]:
            self.assertIn(stale_state_clear, state_hook_text)
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

    def test_tauri_profile_validation_rejects_invalid_paths_and_env_keys(self):
        profile_file = DESKTOP_ROOT / "src-tauri" / "src" / "profile.rs"
        profile_text = profile_file.read_text(encoding="utf-8")

        self.assertIn("pub(crate) const DESKTOP_OWNED_ENV_KEYS", profile_text)
        self.assertIn("reserved.contains(normalized_key.as_str())", profile_text)
        self.assertIn("fn validate_env_key", profile_text)
        self.assertIn("std::collections::HashSet", profile_text)
        self.assertIn("gateway routes file does not exist", profile_text)
        self.assertIn("working directory does not exist", profile_text)
        self.assertIn("duplicate extra env key", profile_text)
        self.assertIn("reserved extra env key", profile_text)

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


if __name__ == "__main__":
    unittest.main()
