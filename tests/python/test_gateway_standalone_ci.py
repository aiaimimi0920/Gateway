import hashlib
import json
import pathlib
import subprocess
import tempfile
import unittest

try:
    from .powershell_test_utils import powershell_executable
    from .gateway_repository_text_fixture import GatewayRepositoryTextFixture
except ImportError:
    from powershell_test_utils import powershell_executable
    from gateway_repository_text_fixture import GatewayRepositoryTextFixture


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayStandaloneCiTests(GatewayRepositoryTextFixture, unittest.TestCase):
    def test_windows_python_contracts_fail_before_rust_without_serial_line_matrix(self):
        workflow = (GATEWAY_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        windows = self._workflow_job_block(workflow, "windows")
        self.assertEqual(windows.count("- name: Run python-tests"), 1)
        self._assert_markers_in_order(
            windows, "- name: Run python-tests", "- name: Check Rust targets"
        )
        self.assertNotIn("verify-gateway-line.ps1", windows)

    def test_both_ci_jobs_run_desktop_regressions(self):
        workflow = (GATEWAY_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        for job_name in ("windows", "linux"):
            with self.subTest(job=job_name):
                job = self._workflow_job_block(workflow, job_name)
                self.assertEqual(job.count("npm test -- --run"), 1)
                self._assert_markers_in_order(job, "npm ci --prefix apps/desktop", "npm test -- --run")

    def test_baseline_validating_workflows_fetch_repository_history(self):
        workflow_jobs = {
            "ci.yml": ("windows", "linux"),
            "build-windows.yml": ("build",),
            "release-tag.yml": ("release",),
        }
        for filename, jobs in workflow_jobs.items():
            workflow = (GATEWAY_ROOT / ".github/workflows" / filename).read_text(
                encoding="utf-8"
            )
            for job_name in jobs:
                with self.subTest(workflow=filename, job=job_name):
                    job = self._workflow_job_block(workflow, job_name)
                    checkout_start = job.index("- name: Checkout")
                    checkout_end = job.index("\n      - name:", checkout_start + 1)
                    checkout = job[checkout_start:checkout_end]
                    self.assertIn("uses: actions/checkout@", checkout)
                    self.assertRegex(checkout, r"(?m)^\s+fetch-depth: 0\s*$")

    def test_linux_standalone_gate_discovers_all_responsibility_suites(self):
        workflow = (GATEWAY_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        linux = self._workflow_job_block(workflow, "linux")
        self.assertIn(
            'python -m unittest discover -s tests/python -p "test_gateway_standalone*.py" -v',
            linux,
        )
        self.assertNotIn("python -m unittest tests.python.test_gateway_standalone_repository_contract -v", linux)

    def test_docker_builder_uses_node22_ui_stage_and_prebuilt_web_assets(self):
        dockerfile = (GATEWAY_ROOT / "Dockerfile").read_text(encoding="utf-8")
        build_sources = [
            GATEWAY_ROOT / "build.rs",
            *sorted((GATEWAY_ROOT / "build_support").glob("*.rs")),
        ]
        build_entry = build_sources[0].read_text(encoding="utf-8")
        for owner in ("embed_source.rs", "manifest.rs", "publish_lock.rs", "snapshot.rs"):
            self.assertIn(f'#[path = "build_support/{owner}"]', build_entry)
        build_script = "\n".join(
            path.read_text(encoding="utf-8") for path in build_sources
        )
        cargo = (GATEWAY_ROOT / "Cargo.toml").read_text(encoding="utf-8")

        self.assertIn("FROM node:22-bookworm-slim AS ui-builder", dockerfile)
        self.assertIn("FROM node:22-bookworm-slim", dockerfile)
        self.assertNotIn("FROM node:20-bookworm-slim", dockerfile)
        self.assertIn("COPY --from=ui-builder /app/apps/desktop/dist/web", dockerfile)
        self.assertIn(
            "COPY --from=ui-builder /app/apps/desktop/dist/.gateway-web-ready",
            dockerfile,
        )
        self.assertIn("ENV GATEWAY_PREBUILT_WEB_UI=1", dockerfile)
        self.assertIn("npm ci --prefix apps/desktop --no-audit --no-fund", dockerfile)
        self.assertNotIn("apt-get install -y --no-install-recommends cmake pkg-config clang nodejs npm", dockerfile)
        self.assertIn(
            'const PREBUILT_WEB_UI_ENV: &str = "GATEWAY_PREBUILT_WEB_UI";',
            build_script,
        )
        self.assertIn(
            'println!("cargo:rerun-if-env-changed={PREBUILT_WEB_UI_ENV}");',
            build_script,
        )
        self.assertIn("prebuilt_web_ui_enabled", build_script)
        self.assertIn("prebuilt web console assets requested", build_script)
        self.assertIn("use sha2::{Digest, Sha256};", build_script)
        self.assertIn('"indexSha256"', build_script)
        self.assertIn('"schemaVersion"', build_script)
        self.assertIn('"files"', build_script)
        self.assertIn("DIST_WEB_PUBLISH_LOCK_PATH", build_script)
        self.assertIn("struct PublishValidationLock", build_script)
        self.assertIn("with_publish_validation_lock", build_script)
        self.assertIn("snapshot_prebuilt_web_ui", build_script)
        self.assertIn("write_embedded_ui_source", build_script)
        self.assertIn("index_referenced_asset_paths", build_script)
        self.assertIn("marker changed during validation", build_script)
        self.assertIn("thread::sleep", build_script)
        self.assertIn('env("GATEWAY_WEB_PRUNE_LIVE", "1")', build_script)
        self.assertIn('"apps/desktop/public"', build_script)
        self.assertIn("GATEWAY_WEB_PRUNE_LIVE=1", dockerfile)
        self.assertIn("[build-dependencies]", cargo)

    def test_docker_verifier_build_failure_does_not_compose_down_existing_project(self):
        verifier = GATEWAY_ROOT / "tools/verify-gateway-docker-stack.ps1"
        source = verifier.read_text(encoding="utf-8")
        original_env = b"COMPOSE_PROJECT_NAME=must-not-be-used\n"
        expected_hash = hashlib.sha256(original_env).hexdigest().upper()

        with tempfile.TemporaryDirectory(prefix="gateway-verifier-preflight-") as raw_root:
            root = pathlib.Path(raw_root)
            tools_dir = root / "tools"
            deploy_dir = root / "deploy"
            tools_dir.mkdir()
            deploy_dir.mkdir()
            test_verifier = tools_dir / verifier.name
            test_verifier.write_text(source, encoding="utf-8")
            (deploy_dir / "docker-compose.yml").write_text(
                "services: {}\n", encoding="utf-8"
            )
            env_path = deploy_dir / ".env"
            env_path.write_bytes(original_env)

            script = r"""
$ErrorActionPreference = 'Stop'
$script:dockerCalls = [System.Collections.Generic.List[string]]::new()
function global:docker {
    $script:dockerCalls.Add(($args | ForEach-Object { [string]$_ }) -join ' ')
    $global:LASTEXITCODE = 23
}
$scriptFailed = $false
try {
    & '__VERIFIER__' -BuildImage -ImageName 'example.invalid/gateway' -ImageTag 'preflight'
} catch {
    $scriptFailed = $true
}
if (-not $scriptFailed) { throw 'Expected the simulated image build to fail.' }
if (@($script:dockerCalls | Where-Object { $_ -like 'compose *' }).Count -ne 0) {
    throw 'Compose was called before the temporary verification environment was prepared.'
}
if ((Get-FileHash -LiteralPath '__ENV__' -Algorithm SHA256).Hash -ne '__ENV_HASH__') {
    throw 'The pre-existing deployment environment was not restored byte-for-byte.'
}
Write-Output 'preflight failure skipped Compose cleanup and preserved deploy/.env'
"""
            script = script.replace("__VERIFIER__", str(test_verifier))
            script = script.replace("__ENV__", str(env_path))
            script = script.replace("__ENV_HASH__", expected_hash)
            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-Command",
                    script,
                ],
                cwd=root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
        self.assertIn(
            "preflight failure skipped Compose cleanup and preserved deploy/.env",
            result.stdout,
        )

    def test_python_validation_dependencies_are_pinned_and_installed(self):
        requirements_path = GATEWAY_ROOT / "tests/python/requirements.txt"
        self.assertTrue(requirements_path.is_file())
        requirements = [
            line.strip()
            for line in requirements_path.read_text(encoding="utf-8").splitlines()
            if line.strip() and not line.lstrip().startswith("#")
        ]
        self.assertEqual(requirements, ["jsonschema==4.25.1"])

        install_command = (
            "python -m pip install --disable-pip-version-check "
            "-r tests/python/requirements.txt"
        )
        workflow_jobs = {
            ".github/workflows/ci.yml": ("windows", "linux"),
            ".github/workflows/build-windows.yml": ("build",),
            ".github/workflows/release-tag.yml": ("release",),
        }
        for relative_path, job_names in workflow_jobs.items():
            workflow = (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
            for job_name in job_names:
                job = self._workflow_job_block(workflow, job_name)
                with self.subTest(workflow=relative_path, job=job_name):
                    self.assertEqual(job.count(install_command), 1)
                    setup_index = job.index("uses: actions/setup-python@")
                    install_index = job.index(install_command)
                    python_use_indices = [
                        job.index(marker)
                        for marker in (
                            "run: python tools/",
                            "run: python -m unittest",
                        )
                        if marker in job
                    ]
                    self.assertTrue(python_use_indices)
                    self.assertLess(setup_index, install_index)
                    self.assertLess(install_index, min(python_use_indices))

    def test_readmes_install_python_validation_dependencies(self):
        install_command = (
            "python -m pip install --disable-pip-version-check "
            "-r tests/python/requirements.txt"
        )
        for relative_path in ("README.md", "README.zh-CN.md"):
            readme = (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
            with self.subTest(readme=relative_path):
                self.assertIn(install_command, readme)

    def test_both_node_production_dependency_trees_define_high_severity_audits(self):
        expected_audit = "npm audit --omit=dev --audit-level=high"
        for relative_path in (
            "apps/desktop/package.json",
            "scripts/package.json",
        ):
            package = json.loads(
                (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
            )
            with self.subTest(package=relative_path):
                self.assertEqual(
                    package.get("scripts", {}).get("audit:prod"),
                    expected_audit,
                )

    def test_release_workflows_install_then_audit_both_node_trees(self):
        workflow_jobs = {
            ".github/workflows/build-windows.yml": ("build",),
            ".github/workflows/release-tag.yml": ("release",),
        }
        scripts_install = "npm ci --prefix scripts --no-audit --no-fund"
        scripts_audit = "npm run audit:prod --prefix scripts"
        desktop_install = "npm ci --prefix apps/desktop --no-audit --no-fund"
        desktop_audit = "npm run audit:prod --prefix apps/desktop"

        for relative_path, job_names in workflow_jobs.items():
            workflow = (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
            for job_name in job_names:
                job = self._workflow_job_block(workflow, job_name)
                with self.subTest(workflow=relative_path, job=job_name):
                    self.assertIn('node-version: "22.22.0"', job)
                    self.assertEqual(job.count(scripts_install), 1)
                    self.assertEqual(job.count(scripts_audit), 1)
                    self.assertEqual(job.count(desktop_install), 1)
                    self.assertEqual(job.count(desktop_audit), 1)
                    self.assertEqual(
                        job.count("cargo test --locked -- --test-threads=1"), 1
                    )
                    self._assert_markers_in_order(
                        job,
                        scripts_install,
                        scripts_audit,
                        "node --test scripts/tests/*.test.mjs",
                    )
                    self._assert_markers_in_order(
                        job,
                        desktop_install,
                        desktop_audit,
                        "run: cargo",
                    )

    def test_development_audits_are_independent_of_product_validation(self):
        workflow = (GATEWAY_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        quality = self._workflow_job_block(workflow, "quality")
        self.assertIn("check: [npm-scripts, npm-desktop, format, lines]", quality)
        self.assertIn("node scripts/development-quality.mjs", quality)
        self.assertIn("if-no-files-found: error", quality)
        for name in ("windows", "linux"):
            job = self._workflow_job_block(workflow, name)
            self.assertNotIn("needs:", job)
            self.assertNotIn("audit:prod", job)
            self.assertNotIn("cargo fmt", job)
            self.assertIn("npm ci --prefix scripts --no-audit --no-fund", job)
            self.assertIn("npm ci --prefix apps/desktop --no-audit --no-fund", job)
            self.assertIn("cargo test --locked -- --test-threads=1", job)

    def test_docker_workflow_audits_both_node_trees_outside_buildkit_cache(self):
        workflow = (GATEWAY_ROOT / ".github/workflows/docker.yml").read_text(
            encoding="utf-8"
        )
        job = self._workflow_job_block(workflow, "docker")
        first_docker_build = "uses: docker/build-push-action@v6"

        self.assertIn('node-version: "22.22.0"', job)
        self._assert_markers_in_order(
            job,
            "uses: actions/setup-node@v6",
            "npm ci --prefix scripts --no-audit --no-fund",
            'node scripts/npm-audit.mjs "$NPM_AUDIT_MODE" scripts',
            first_docker_build,
        )
        self._assert_markers_in_order(
            job,
            "uses: actions/setup-node@v6",
            "npm ci --prefix apps/desktop --no-audit --no-fund",
            'node scripts/npm-audit.mjs "$NPM_AUDIT_MODE" apps/desktop',
            first_docker_build,
        )

        dockerfile = (GATEWAY_ROOT / "Dockerfile").read_text(encoding="utf-8")
        self._assert_markers_in_order(
            dockerfile,
            "npm ci --prefix apps/desktop --no-audit --no-fund",
            'node /tmp/gateway-audit/npm-audit.mjs "$GATEWAY_AUDIT_MODE" /app/apps/desktop',
            "npm run build:web --prefix apps/desktop",
        )
        self._assert_markers_in_order(
            dockerfile,
            "npm ci --omit=dev --no-audit --no-fund",
            'node /tmp/gateway-audit/npm-audit.mjs "$GATEWAY_AUDIT_MODE" /app/scripts',
            "npm cache clean --force",
        )

    def test_release_builder_checks_node_and_installs_then_audits_both_node_trees(self):
        script_path = GATEWAY_ROOT / "tools/build-gateway-release.ps1"
        script = script_path.read_text(encoding="utf-8")
        main = script[script.index("Push-Location -LiteralPath $repoRoot") :]
        npm_ci_function = self._powershell_function_block(
            script, "Invoke-NpmCiWithRetry"
        )

        self.assertIn('$minimumNodeVersion = [version]"22.22.0"', script)
        self.assertIn("function Assert-MinimumNodeVersion", script)
        self.assertIn(
            '-Arguments @("ci", "--no-audit", "--no-fund")', npm_ci_function
        )
        self._assert_markers_in_order(
            main,
            "Assert-MinimumNodeVersion",
            'Invoke-GatewayReleaseStep -Name "install browser worker dependencies"',
            'Invoke-GatewayReleaseStep -Name "audit production browser worker dependencies"',
            'Invoke-GatewayReleaseStep -Name "install desktop dependencies"',
            'Invoke-GatewayReleaseStep -Name "audit production desktop dependencies"',
            'Invoke-GatewayReleaseStep -Name "typecheck desktop UI"',
            'Invoke-GatewayReleaseStep -Name "build headless gateway"',
        )
        self.assertIn("-WorkingDirectory $scriptsRoot", main)
        self.assertIn("-WorkingDirectory $desktopRoot", main)

        result = subprocess.run(
            [
                powershell_executable(),
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(script_path),
                "-DryRun",
            ],
            cwd=GATEWAY_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
        self._assert_markers_in_order(
            result.stdout,
            "Node.js >= 22.22.0",
            "npm ci --prefix scripts --no-audit --no-fund",
            "npm run audit:prod --prefix scripts",
            "npm ci --prefix apps/desktop --no-audit --no-fund",
            "npm run audit:prod --prefix apps/desktop",
            "npm run typecheck --prefix apps/desktop",
            "cargo build --locked --release --bin gateway",
        )

    def test_readmes_require_the_supported_node_patch_release(self):
        for relative_path in ("README.md", "README.zh-CN.md"):
            readme = (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
            with self.subTest(readme=relative_path):
                self.assertIn("Node.js `>=22.22.0`", readme)

    def test_workflows_cover_validation_build_container_and_release(self):
        ci = (GATEWAY_ROOT / ".github/workflows/ci.yml").read_text(encoding="utf-8")
        build = (GATEWAY_ROOT / ".github/workflows/build-windows.yml").read_text(
            encoding="utf-8"
        )
        docker = (GATEWAY_ROOT / ".github/workflows/docker.yml").read_text(
            encoding="utf-8"
        )
        release = (GATEWAY_ROOT / ".github/workflows/release-tag.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("pull_request:", ci)
        self.assertIn("python-tests", ci)
        self.assertIn("browser-worker-tests", ci)
        self.assertIn("cargo test --locked", ci)
        self.assertIn("package-gateway-release.ps1", build)
        self.assertIn("actions/upload-artifact", build)
        self.assertIn("ghcr.io/aiaimimi0920/gateway", docker)
        self.assertIn("packages: write", docker)
        self.assertIn("^V[0-9]+\\.[0-9]+\\.[0-9]+$", release)
        self.assertIn("compress-gateway-release.ps1", release)
        self.assertIn("softprops/action-gh-release", release)

    def test_docker_publish_is_serialized_per_ref(self):
        docker = (GATEWAY_ROOT / ".github/workflows/docker.yml").read_text(
            encoding="utf-8"
        )

        concurrency_pattern = (
            r"(?m)^concurrency:\s*\r?\n"
            r"\s+group:\s+gateway-docker-\$\{\{\s*github\.ref\s*\}\}\s*\r?\n"
            r"\s+cancel-in-progress:\s+true\s*$"
        )
        self.assertRegex(docker, concurrency_pattern)

    def test_docker_rejects_tags_outside_the_release_tag_contract(self):
        docker = (GATEWAY_ROOT / ".github/workflows/docker.yml").read_text(
            encoding="utf-8"
        )

        validation_marker = "- name: Validate release tag"
        self.assertIn(validation_marker, docker)
        validation_start = docker.index(validation_marker)
        validation_end = docker.index("\n      - name:", validation_start + 1)
        validation = docker[validation_start:validation_end]
        login_start = docker.index("- name: Log in to GitHub Container Registry")
        release = (GATEWAY_ROOT / ".github/workflows/release-tag.yml").read_text(
            encoding="utf-8"
        )
        strict_tag_pattern = r"^V[0-9]+\.[0-9]+\.[0-9]+$"

        self.assertIn("GITHUB_REF_NAME", validation)
        self.assertIn(strict_tag_pattern, validation)
        self.assertIn(strict_tag_pattern, release)
        self.assertIn("exit 1", validation)
        self.assertLess(validation_start, login_start)
