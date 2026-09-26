import pathlib
import subprocess
import unittest

try:
    from .powershell_test_utils import powershell_executable
except ImportError:
    from powershell_test_utils import powershell_executable


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayStandaloneDeployTests(unittest.TestCase):
    def test_service_deploy_stack_exists_for_direct_docker_deployment(self):
        dockerfile = (GATEWAY_ROOT / "Dockerfile").read_text(encoding="utf-8")
        compose = (GATEWAY_ROOT / "deploy/docker-compose.yml").read_text(
            encoding="utf-8"
        )
        compose_local = (GATEWAY_ROOT / "deploy/docker-compose.local.yml").read_text(
            encoding="utf-8"
        )
        deploy_env = (GATEWAY_ROOT / "deploy/.env.example").read_text(
            encoding="utf-8"
        )
        deploy_readme = (GATEWAY_ROOT / "deploy/README.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("ENV GATEWAY_RUNTIME_ROLE=standalone", dockerfile)
        for compose_text in (compose, compose_local):
            self.assertIn("ghcr.io/aiaimimi0920/gateway", compose_text)
            self.assertIn("${GATEWAY_ENV_FILE:-.env}", compose_text)
            self.assertIn("redis:", compose_text)
            self.assertIn("redis://redis:6379/0", compose_text)
            self.assertIn("standalone", compose_text)
            self.assertIn("/healthz", compose_text)
        self.assertIn("IMAGE_TAG=latest", deploy_env)
        self.assertIn("docker compose -f docker-compose.local.yml up -d", deploy_readme)
        self.assertIn("redis_data", compose_local)
        self.assertIn("gateway_data", compose_local)

    def test_source_mounted_dev_docker_stack_exists_for_live_iteration(self):
        dockerfile_dev = (GATEWAY_ROOT / "Dockerfile.dev").read_text(
            encoding="utf-8"
        )
        compose_dev = (GATEWAY_ROOT / "deploy/docker-compose.dev.yml").read_text(
            encoding="utf-8"
        )
        entrypoint = (
            GATEWAY_ROOT / "deploy/docker-dev-entrypoint.sh"
        ).read_text(encoding="utf-8")
        rsbuild_config = (
            GATEWAY_ROOT / "apps/desktop/rsbuild.config.ts"
        ).read_text(encoding="utf-8")
        deploy_readme = (GATEWAY_ROOT / "deploy/README.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("FROM node:22-bookworm-slim", dockerfile_dev)
        self.assertIn("cargo install cargo-watch", dockerfile_dev)
        self.assertIn("dockerfile: Dockerfile.dev", compose_dev)
        self.assertIn("context: ..", compose_dev)
        self.assertIn("..:/workspace", compose_dev)
        self.assertIn("/workspace/target", compose_dev)
        self.assertIn("gateway-dev-entrypoint", compose_dev)
        self.assertIn("start_period: 600s", compose_dev)
        self.assertIn("cargo watch", entrypoint)
        self.assertIn("npm run build:web --prefix apps/desktop -- --watch", entrypoint)
        self.assertIn("apps/desktop/dist/.gateway-web-ready", entrypoint)
        self.assertNotIn("--watch apps/desktop/dist/web", entrypoint)
        self.assertIn("cleanup_frontend_staging", entrypoint)
        self.assertIn('-name "web-staging-*"', entrypoint)
        self.assertNotIn('rm -rf "${FRONTEND_ROOT}/dist"', entrypoint)
        self.assertGreaterEqual(entrypoint.count("cleanup_frontend_staging"), 3)
        self.assertIn("const stagedPublish = isWeb;", rsbuild_config)
        self.assertNotIn("process.env.GATEWAY_WEB_STAGED_PUBLISH", rsbuild_config)
        self.assertIn("randomUUID", rsbuild_config)
        self.assertIn("process.pid", rsbuild_config)
        self.assertIn("if (!stats || stats.hasErrors())", rsbuild_config)
        self.assertIn("pruneLive", rsbuild_config)
        self.assertIn('!process.argv.includes("--watch")', rsbuild_config)
        self.assertIn("maxRetries: 5", rsbuild_config)
        self.assertIn("retryDelay: 100", rsbuild_config)
        self.assertIn("docker compose -f docker-compose.dev.yml up -d", deploy_readme)
        self.assertIn("health: starting", deploy_readme)
        self.assertIn("-Mode dev", deploy_readme)

    def test_repository_build_entrypoints_enforce_single_process_without_incremental_compilation(self):
        cargo_config = (GATEWAY_ROOT / ".cargo/config.toml").read_text(
            encoding="utf-8"
        )
        dockerfile = (GATEWAY_ROOT / "Dockerfile").read_text(encoding="utf-8")
        dockerfile_dev = (GATEWAY_ROOT / "Dockerfile.dev").read_text(
            encoding="utf-8"
        )
        compose_dev = (GATEWAY_ROOT / "deploy/docker-compose.dev.yml").read_text(
            encoding="utf-8"
        )
        entrypoint = (
            GATEWAY_ROOT / "deploy/docker-dev-entrypoint.sh"
        ).read_text(encoding="utf-8")
        release_builder = (
            GATEWAY_ROOT / "tools/build-gateway-release.ps1"
        ).read_text(encoding="utf-8")

        self.assertIn("jobs = 1", cargo_config)
        self.assertIn("incremental = false", cargo_config)
        self.assertIn("COPY .cargo/config.toml ./.cargo/config.toml", dockerfile)
        self.assertIn("CARGO_BUILD_JOBS=1", dockerfile)
        self.assertIn("CARGO_INCREMENTAL=0", dockerfile)
        self.assertIn("GATEWAY_DEV_CARGO_BUILD_JOBS=4", dockerfile_dev)
        self.assertIn("GATEWAY_DEV_CARGO_INCREMENTAL=1", dockerfile_dev)
        self.assertNotIn("CARGO_BUILD_JOBS=1", dockerfile_dev)
        self.assertIn(
            "GATEWAY_DEV_CARGO_BUILD_JOBS=${GATEWAY_DEV_CARGO_BUILD_JOBS:-4}",
            compose_dev,
        )
        self.assertIn(
            "GATEWAY_DEV_CARGO_INCREMENTAL=${GATEWAY_DEV_CARGO_INCREMENTAL:-1}",
            compose_dev,
        )
        self.assertIn(': "${GATEWAY_DEV_CARGO_BUILD_JOBS:=4}"', entrypoint)
        self.assertIn(': "${GATEWAY_DEV_CARGO_INCREMENTAL:=1}"', entrypoint)
        self.assertIn(
            'export CARGO_BUILD_JOBS="${GATEWAY_DEV_CARGO_BUILD_JOBS}"', entrypoint
        )
        self.assertIn(
            'export CARGO_INCREMENTAL="${GATEWAY_DEV_CARGO_INCREMENTAL}"',
            entrypoint,
        )
        self.assertIn("$defaultCargoBuildJobs = 1", release_builder)
        self.assertIn('$env:CARGO_BUILD_JOBS = "1"', release_builder)
        self.assertIn('$env:CARGO_INCREMENTAL = "0"', release_builder)
        self.assertNotIn("using caller-provided CARGO_BUILD_JOBS", release_builder)

    def test_docker_readmes_document_one_click_deploy_script(self):
        for relative_path in ("README.md", "README.zh-CN.md", "deploy/README.md"):
            content = (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
            with self.subTest(path=relative_path):
                self.assertIn("docker-deploy.sh", content)

    def test_verify_docker_stack_script_avoids_unbraced_image_tag_interpolation(self):
        script = (
            GATEWAY_ROOT / "tools/verify-gateway-docker-stack.ps1"
        ).read_text(encoding="utf-8")

        self.assertIn('("{0}:{1}" -f $ImageName, $ImageTag)', script)
        self.assertNotIn('"$ImageName:$ImageTag"', script)

    def test_docker_helper_scripts_are_powershell_parseable(self):
        probe = (
            "$ErrorActionPreference='Stop'; "
            "$scripts=@("
            "'tools/deploy-gateway-docker.ps1',"
            "'tools/verify-gateway-docker-stack.ps1',"
            "'tools/export-gateway-docker-deploy-bundle.ps1'"
            "); "
            "foreach($relative in $scripts){ "
            "$tokens=$null; $errors=$null; "
            "[System.Management.Automation.Language.Parser]::ParseFile((Join-Path $pwd $relative), [ref]$tokens, [ref]$errors) | Out-Null; "
            "if($errors.Count -ne 0){ throw ($relative + ': ' + (($errors | ForEach-Object { $_.Message }) -join ' | ')) } "
            "}"
        )
        result = subprocess.run(
            [powershell_executable(), "-NoLogo", "-NoProfile", "-NonInteractive", "-Command", probe],
            cwd=GATEWAY_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=60,
            check=False,
        )
        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)

    def test_docker_helper_scripts_resolve_compose_filename_before_join_path(self):
        for relative_path in (
            "tools/deploy-gateway-docker.ps1",
            "tools/verify-gateway-docker-stack.ps1",
        ):
            script = (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
            with self.subTest(script=relative_path):
                if relative_path.endswith("deploy-gateway-docker.ps1"):
                    self.assertIn(
                        '$composeFileName = if ($Mode -eq "local") { "docker-compose.local.yml" } elseif ($Mode -eq "dev") { "docker-compose.dev.yml" } else { "docker-compose.yml" }',
                        script,
                    )
                else:
                    self.assertIn(
                        '$composeFileName = if ($Mode -eq "local") { "docker-compose.local.yml" } else { "docker-compose.yml" }',
                        script,
                    )
                self.assertNotIn("Join-Path $deployDir (", script)

    def test_deploy_helper_supports_dev_mode_for_source_mounted_gateway_stack(self):
        script = (GATEWAY_ROOT / "tools/deploy-gateway-docker.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn('[ValidateSet("named", "local", "dev")]', script)
        self.assertIn(
            '$composeFileName = if ($Mode -eq "local") { "docker-compose.local.yml" } elseif ($Mode -eq "dev") { "docker-compose.dev.yml" } else { "docker-compose.yml" }',
            script,
        )

    def test_verify_docker_stack_script_restores_compose_env_file(self):
        script = (
            GATEWAY_ROOT / "tools/verify-gateway-docker-stack.ps1"
        ).read_text(encoding="utf-8")

        self.assertIn('$composeEnvFile = Join-Path $deployDir ".env"', script)
        self.assertIn("Copy-Item -LiteralPath $composeEnvFile -Destination $composeEnvBackupPath -Force", script)
        self.assertIn("Move-Item -LiteralPath $composeEnvBackupPath -Destination $composeEnvFile -Force", script)

    def test_docker_helper_env_writers_accept_blank_lines_from_env_templates(self):
        for relative_path in (
            "tools/deploy-gateway-docker.ps1",
            "tools/verify-gateway-docker-stack.ps1",
        ):
            script = (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
            with self.subTest(script=relative_path):
                self.assertIn("[AllowEmptyString()][AllowEmptyCollection()][string[]]$Lines", script)

    def test_deploy_helper_avoids_non_dotnet_regex_escape_sequences(self):
        script = (GATEWAY_ROOT / "tools/deploy-gateway-docker.ps1").read_text(
            encoding="utf-8"
        )

        self.assertNotIn("\\Q", script)
        self.assertIn("[Regex]::Escape($Key)", script)

    def test_deploy_helper_seeds_loopback_console_defaults_for_fresh_local_users(self):
        script = (GATEWAY_ROOT / "tools/deploy-gateway-docker.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn("function Test-IsLoopbackBindHost", script)
        self.assertIn('Set-DotEnvValueIfMissing -Path $envPath -Key "GATEWAY_MANAGEMENT_TOKEN" -Value "123456"', script)
        self.assertIn('Set-DotEnvValueIfMissing -Path $envPath -Key "GATEWAY_CONSOLE_REMOTE_ACCESS" -Value "true"', script)

    def test_readmes_explain_the_default_loopback_console_login_for_the_root_helper(self):
        readme = (GATEWAY_ROOT / "README.md").read_text(encoding="utf-8")
        deploy_readme = (GATEWAY_ROOT / "deploy/README.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("GATEWAY_MANAGEMENT_TOKEN=123456", readme)
        self.assertIn("127.0.0.1", readme)
        self.assertIn("GATEWAY_MANAGEMENT_TOKEN=123456", deploy_readme)
        self.assertIn("/ui/", deploy_readme)
