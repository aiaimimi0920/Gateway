import hashlib
import json
import os
import pathlib
import re
import subprocess
import tempfile
import time
import unittest

try:
    from .powershell_test_utils import powershell_executable
except ImportError:
    from powershell_test_utils import powershell_executable


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]


class GatewayStandaloneRepositoryContractTests(unittest.TestCase):
    @staticmethod
    def _powershell_function_block(script: str, function_name: str) -> str:
        start = script.index(f"function {function_name}")
        end = script.find("\nfunction ", start + 1)
        if end < 0:
            end = len(script)
        return script[start:end]

    @staticmethod
    def _workflow_job_block(workflow: str, job_name: str) -> str:
        match = re.search(
            rf"(?ms)^  {re.escape(job_name)}:\r?$.*?(?=^  [A-Za-z0-9_-]+:\r?$|\Z)",
            workflow,
        )
        if match is None:
            raise AssertionError(f"missing workflow job: {job_name}")
        return match.group(0)

    def test_repository_metadata_and_workflows_exist(self):
        required = [
            ".gitattributes",
            "LICENSE",
            "README.md",
            "README.zh-CN.md",
            "rust-toolchain.toml",
            "Dockerfile.dev",
            "deploy/.env.example",
            "deploy/README.md",
            "deploy/docker-compose.yml",
            "deploy/docker-compose.local.yml",
            "deploy/docker-compose.dev.yml",
            "deploy/docker-deploy.sh",
            "deploy/docker-dev-entrypoint.sh",
            ".github/workflows/ci.yml",
            ".github/workflows/build-windows.yml",
            ".github/workflows/docker.yml",
            ".github/workflows/release-tag.yml",
            "tools/compress-gateway-release.ps1",
            "tools/verify-gateway-line.ps1",
        ]

        missing = [path for path in required if not (GATEWAY_ROOT / path).is_file()]
        self.assertEqual([], missing, f"missing standalone repository files: {missing}")

    def test_cargo_metadata_identifies_the_independent_repository(self):
        cargo = (GATEWAY_ROOT / "Cargo.toml").read_text(encoding="utf-8")
        toolchain = (GATEWAY_ROOT / "rust-toolchain.toml").read_text(
            encoding="utf-8"
        )

        self.assertIn('description = "Production AI gateway runtime for Neuro"', cargo)
        self.assertIn('license = "MIT"', cargo)
        self.assertIn('repository = "https://github.com/aiaimimi0920/Gateway"', cargo)
        self.assertIn('readme = "README.md"', cargo)
        self.assertIn('rust-version = "1.91.1"', cargo)
        self.assertIn('channel = "1.91.1"', toolchain)

    def test_build_script_uses_gateway_repository_root(self):
        script = (GATEWAY_ROOT / "tools/build-gateway-release.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn('Join-Path $PSScriptRoot ".."', script)
        self.assertIn("cargo build --locked --release --bin gateway", script)
        self.assertNotIn('Join-Path $PSScriptRoot "..\\.."', script)
        self.assertNotIn("Gateway/Cargo.toml", script)
        self.assertNotIn('"Gateway\\target', script)

    def test_build_source_state_uses_dot_for_the_repository_root_pathspec(self):
        script = (GATEWAY_ROOT / "tools/build-gateway-release.ps1").read_text(
            encoding="utf-8"
        )
        function_start = script.index("function Get-RelativeUnixPath")
        function_end = script.index("function Get-SourceTreeState", function_start)
        function_text = script[function_start:function_end]

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            probe = pathlib.Path(temp_dir) / "probe-relative-root.ps1"
            probe.write_text(
                function_text
                + "\nGet-RelativeUnixPath -BasePath $args[0] -Path $args[0]\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(probe),
                    str(GATEWAY_ROOT),
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
        self.assertEqual(result.stdout.strip(), ".")

    def test_package_source_state_uses_dot_for_the_repository_root_pathspec(self):
        script = (GATEWAY_ROOT / "tools/package-gateway-release.ps1").read_text(
            encoding="utf-8"
        )
        resolve_start = script.index("function Resolve-FullPath")
        resolve_end = script.index("function Resolve-GatewayRoot", resolve_start)
        function_start = script.index("function Get-RelativeUnixPath")
        function_end = script.index("function Write-Utf8NoBom", function_start)
        function_text = (
            script[resolve_start:resolve_end] + script[function_start:function_end]
        )

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            probe = pathlib.Path(temp_dir) / "probe-package-relative-root.ps1"
            probe.write_text(
                function_text
                + "\nGet-RelativeUnixPath -BasePath $args[0] -Path $args[0]\n",
                encoding="utf-8",
            )
            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(probe),
                    str(GATEWAY_ROOT),
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
        self.assertEqual(result.stdout.strip(), ".")

    def test_build_and_package_source_state_match_git_file_semantics(self):
        build_script = (GATEWAY_ROOT / "tools/build-gateway-release.ps1").read_text(
            encoding="utf-8"
        )
        package_script = (
            GATEWAY_ROOT / "tools/package-gateway-release.ps1"
        ).read_text(encoding="utf-8")

        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            source_root.mkdir()
            (source_root / ".gitignore").write_text(
                "/.env\n/generated/\n/ignored-tracked.txt\n",
                encoding="utf-8",
            )
            (source_root / "tracked.txt").write_text("tracked-v1\n", encoding="utf-8")
            (source_root / "ignored-tracked.txt").write_text(
                "ignored-but-tracked-v1\n", encoding="utf-8"
            )
            (source_root / ".env").write_text("SECRET=fixture\n", encoding="utf-8")
            generated = source_root / "generated" / "cache.txt"
            generated.parent.mkdir()
            generated.write_text("cache-v1\n", encoding="utf-8")

            subprocess.run(
                ["git", "init"],
                cwd=source_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            subprocess.run(
                ["git", "add", ".gitignore", "tracked.txt"],
                cwd=source_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            subprocess.run(
                ["git", "add", "-f", "ignored-tracked.txt"],
                cwd=source_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            subprocess.run(
                [
                    "git",
                    "-c",
                    "user.name=Gateway Contract",
                    "-c",
                    "user.email=gateway-contract@example.invalid",
                    "commit",
                    "-m",
                    "fixture",
                ],
                cwd=source_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            untracked = source_root / "untracked.txt"
            untracked.write_text("untracked-v1\n", encoding="utf-8")

            probes = {
                "build": "\n".join(
                    (
                        self._powershell_function_block(
                            build_script, "Get-RelativeUnixPath"
                        ),
                        self._powershell_function_block(
                            build_script, "Get-SourceTreeState"
                        ),
                    )
                ),
                "package": "\n".join(
                    (
                        self._powershell_function_block(
                            package_script, "Resolve-FullPath"
                        ),
                        self._powershell_function_block(
                            package_script, "Get-RelativeUnixPath"
                        ),
                        self._powershell_function_block(
                            package_script, "Get-GitRepositoryRoot"
                        ),
                        self._powershell_function_block(
                            package_script, "Get-SourceTreeState"
                        ),
                    )
                ),
            }
            probe_paths = {}
            for name, functions in probes.items():
                probe = temporary_root / f"probe-{name}.ps1"
                probe.write_text(
                    functions
                    + "\nGet-SourceTreeState -Root $args[0] | ConvertTo-Json -Compress\n",
                    encoding="utf-8",
                )
                probe_paths[name] = probe

            def read_states() -> dict[str, dict[str, object]]:
                states = {}
                for name, probe in probe_paths.items():
                    result = subprocess.run(
                        [
                            powershell_executable(),
                            "-NoLogo",
                            "-NoProfile",
                            "-NonInteractive",
                            "-ExecutionPolicy",
                            "Bypass",
                            "-File",
                            str(probe),
                            str(source_root),
                        ],
                        cwd=GATEWAY_ROOT,
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                        timeout=60,
                        check=False,
                    )
                    self.assertEqual(
                        result.returncode, 0, msg=result.stdout + result.stderr
                    )
                    states[name] = json.loads(result.stdout)
                return states

            initial = read_states()
            self.assertEqual(initial["build"], initial["package"])
            self.assertEqual(
                initial["build"]["algorithm"], "sha256-git-source-list-v2"
            )

            (source_root / ".env").write_text("SECRET=changed\n", encoding="utf-8")
            generated.write_text("cache-v2\n", encoding="utf-8")
            ignored_changed = read_states()
            self.assertEqual(ignored_changed["build"], ignored_changed["package"])
            self.assertEqual(
                initial["build"]["fingerprint"],
                ignored_changed["build"]["fingerprint"],
            )

            untracked.write_text("untracked-v2\n", encoding="utf-8")
            untracked_changed = read_states()
            self.assertEqual(untracked_changed["build"], untracked_changed["package"])
            self.assertNotEqual(
                ignored_changed["build"]["fingerprint"],
                untracked_changed["build"]["fingerprint"],
            )

            (source_root / "ignored-tracked.txt").write_text(
                "ignored-but-tracked-v2\n", encoding="utf-8"
            )
            tracked_changed = read_states()
            self.assertEqual(tracked_changed["build"], tracked_changed["package"])
            self.assertNotEqual(
                untracked_changed["build"]["fingerprint"],
                tracked_changed["build"]["fingerprint"],
            )

    def test_release_artifacts_are_excluded_from_source_fingerprints(self):
        scripts = {
            "build": GATEWAY_ROOT / "tools/build-gateway-release.ps1",
            "package": GATEWAY_ROOT / "tools/package-gateway-release.ps1",
        }
        for name, path in scripts.items():
            script = path.read_text(encoding="utf-8")
            function_start = script.index("function Get-SourceTreeState")
            function_end = script.index("\nfunction ", function_start + 1)
            function_text = script[function_start:function_end]
            with self.subTest(script=name):
                self.assertIn('$excludedRootDirectoryNames = @("release")', function_text)
                self.assertIn("$index -eq 0", function_text)

    def test_source_fingerprints_follow_git_ignore_rules(self):
        scripts = {
            "build": GATEWAY_ROOT / "tools/build-gateway-release.ps1",
            "package": GATEWAY_ROOT / "tools/package-gateway-release.ps1",
        }
        git_file_list = "ls-files --cached --others --exclude-standard"
        for name, path in scripts.items():
            script = path.read_text(encoding="utf-8")
            function_start = script.index("function Get-SourceTreeState")
            function_end = script.index("\nfunction ", function_start + 1)
            function_text = script[function_start:function_end]
            with self.subTest(script=name):
                self.assertIn(git_file_list, function_text)
                self.assertIn('algorithm = "sha256-git-source-list-v2"', function_text)
                self.assertIn('if ($null -eq $gitCommand)', function_text)
                self.assertIn('if ($LASTEXITCODE -ne 0)', function_text)
                self.assertIn(
                    'throw "Unable to enumerate Gateway source files with Git',
                    function_text,
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

    def test_packager_defaults_to_repository_local_release_root(self):
        script = (GATEWAY_ROOT / "tools/package-gateway-release.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn('Join-Path $gatewayRoot "release\\Gateway"', script)
        self.assertNotIn('Join-Path $PSScriptRoot "..\\..\\release\\Gateway"', script)

    def test_evidence_runner_owns_its_line_verifier(self):
        script = (GATEWAY_ROOT / "tools/run-gateway-line-evidence.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn('Join-Path $GatewayRoot "tools\\verify-gateway-line.ps1"', script)
        self.assertNotIn('Join-Path $RepoRoot "deploy\\verify-gateway-line.ps1"', script)

    def test_cli_help_uses_repository_owned_validation_entrypoints(self):
        source = (GATEWAY_ROOT / "src/main.rs").read_text(encoding="utf-8")

        self.assertIn(
            "powershell -File tools/verify-gateway-release-candidate.ps1",
            source,
        )
        self.assertIn(
            "powershell -File tools/verify-gateway-line.ps1 -All",
            source,
        )
        self.assertNotIn("deploy/verify-gateway-line.ps1", source)

    def test_browser_probe_scripts_resolve_the_gateway_root_from_scripts(self):
        probe_scripts = sorted((GATEWAY_ROOT / "scripts").glob("probe-*.mjs"))
        self.assertTrue(probe_scripts, "expected Gateway browser probe scripts")
        for script_path in probe_scripts:
            script = script_path.read_text(encoding="utf-8")
            with self.subTest(script=script_path.name):
                self.assertNotIn('path.resolve(scriptDir, "..", "..")', script)

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

    def test_readme_documents_independent_clone_and_release(self):
        readme = (GATEWAY_ROOT / "README.md").read_text(encoding="utf-8")

        self.assertIn("https://github.com/aiaimimi0920/Gateway", readme)
        self.assertIn("git clone", readme)
        self.assertIn(".\\tools\\build-gateway-release.ps1", readme)
        self.assertNotIn("Build and package from the monorepo root", readme)

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

        self.assertIn("FROM node:20-bookworm-slim", dockerfile_dev)
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
        self.assertIn("CARGO_BUILD_JOBS=1", dockerfile_dev)
        self.assertIn("CARGO_INCREMENTAL=0", dockerfile_dev)
        self.assertNotIn("CARGO_BUILD_JOBS=2", dockerfile_dev)
        self.assertIn("CARGO_BUILD_JOBS=1", compose_dev)
        self.assertNotIn("CARGO_BUILD_JOBS=${CARGO_BUILD_JOBS", compose_dev)
        self.assertIn("CARGO_INCREMENTAL=0", compose_dev)
        self.assertIn("export CARGO_BUILD_JOBS=1", entrypoint)
        self.assertIn("export CARGO_INCREMENTAL=0", entrypoint)
        self.assertNotIn(': "${CARGO_BUILD_JOBS:=1}"', entrypoint)
        self.assertIn("$defaultCargoBuildJobs = 1", release_builder)
        self.assertIn('$env:CARGO_BUILD_JOBS = "1"', release_builder)
        self.assertIn('$env:CARGO_INCREMENTAL = "0"', release_builder)
        self.assertNotIn("using caller-provided CARGO_BUILD_JOBS", release_builder)

    def test_web_dist_publisher_keeps_previous_assets_until_new_index_is_published(self):
        publisher = (
            GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"
        )

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            (staging / "static/js").mkdir(parents=True)
            (live / "static/js").mkdir(parents=True)
            (staging / "index.html").write_text(
                '<script src="/ui/static/js/new.js"></script>', encoding="utf-8"
            )
            (staging / "static/js/new.js").write_text("new", encoding="utf-8")
            (live / "index.html").write_text(
                '<script src="/ui/static/js/old.js"></script>', encoding="utf-8"
            )
            (live / "static/js/old.js").write_text("old", encoding="utf-8")

            result = subprocess.run(
                [
                    "node",
                    str(publisher),
                    "--staging",
                    str(staging),
                    "--live",
                    str(live),
                    "--ready",
                    str(ready),
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertEqual(
                0,
                result.returncode,
                f"publisher failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            self.assertEqual(
                '<script src="/ui/static/js/new.js"></script>',
                (live / "index.html").read_text(encoding="utf-8"),
            )
            self.assertEqual(
                "new", (live / "static/js/new.js").read_text(encoding="utf-8")
            )
            self.assertEqual(
                "old", (live / "static/js/old.js").read_text(encoding="utf-8")
            )
            self.assertTrue(ready.is_file())

    def test_web_dist_ready_marker_hashes_every_published_file(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            (staging / "static/js").mkdir(parents=True)
            (staging / "static/css").mkdir(parents=True)
            (staging / "index.html").write_text(
                '<link rel="icon" href="/ui/favicon.svg">'
                '<link rel="stylesheet" href="/ui/static/css/app.css">'
                '<script src="/ui/static/js/app.js"></script>',
                encoding="utf-8",
            )
            (staging / "favicon.svg").write_text("icon", encoding="utf-8")
            (staging / "static/css/app.css").write_text(
                "body { color: white; }", encoding="utf-8"
            )
            (staging / "static/js/app.js").write_text(
                "console.log('ready');", encoding="utf-8"
            )

            result = subprocess.run(
                [
                    "node",
                    str(publisher),
                    "--staging",
                    str(staging),
                    "--live",
                    str(live),
                    "--ready",
                    str(ready),
                    "--prune",
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertEqual(
                0,
                result.returncode,
                f"publisher failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            marker = json.loads(ready.read_text(encoding="utf-8"))
            expected_files = {
                path.relative_to(staging).as_posix(): hashlib.sha256(
                    path.read_bytes()
                ).hexdigest()
                for path in sorted(staging.rglob("*"))
                if path.is_file()
            }
            self.assertEqual(1, marker.get("schemaVersion"))
            self.assertEqual(expected_files, marker.get("files"))
            self.assertEqual(
                expected_files["index.html"], marker.get("indexSha256")
            )

    def test_web_dist_publisher_invalidates_old_marker_while_lock_is_held(self):
        publisher = (GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs").as_uri()

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            publish_lock_owner = root / ".gateway-web-publish.lock" / "owner.json"
            staging.mkdir()
            live.mkdir()
            old_index = b"old-index"
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_bytes(old_index)
            ready.write_text(
                json.dumps(
                    {
                        "indexSha256": hashlib.sha256(old_index).hexdigest(),
                        "publishedAt": "2026-07-28T00:00:00.000Z",
                    }
                ),
                encoding="utf-8",
            )

            script = f"""
                import {{ access, writeFile }} from 'node:fs/promises';
                import {{ publishWebDist }} from {json.dumps(publisher)};
                await publishWebDist(
                  {{
                    stagingDir: {json.dumps(str(staging))},
                    liveDir: {json.dumps(str(live))},
                    readyFile: {json.dumps(str(ready))},
                  }},
                  {{
                    replaceReadyMarker: async (readyFile, marker) => {{
                      try {{
                        await access(readyFile);
                        throw new Error('old ready marker remained valid while publish lock was held');
                      }} catch (error) {{
                        if (error?.code !== 'ENOENT') {{
                          throw error;
                        }}
                      }}
                      await access({json.dumps(str(publish_lock_owner))});
                      await writeFile(readyFile, marker, 'utf8');
                    }},
                  }},
                );
            """
            result = subprocess.run(
                ["node", "--input-type=module", "--eval", script],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertEqual(
                0,
                result.returncode,
                f"publisher failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            self.assertTrue(ready.is_file())

    def test_web_dist_publisher_does_not_steal_an_old_lock_without_owner_release(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            publish_lock = root / ".gateway-web-publish.lock"
            staging.mkdir()
            live.mkdir()
            publish_lock.mkdir()
            lock_owner = publish_lock / "owner.json"
            lock_owner.write_text(
                json.dumps({"ownerToken": "active-owner"}), encoding="utf-8"
            )
            old_timestamp = time.time() - 120
            os.utime(publish_lock, (old_timestamp, old_timestamp))
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")

            process = subprocess.Popen(
                [
                    "node",
                    str(publisher),
                    "--staging",
                    str(staging),
                    "--live",
                    str(live),
                    "--ready",
                    str(ready),
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            try:
                time.sleep(0.3)
                self.assertIsNone(
                    process.poll(), "publisher ignored the active publish lock"
                )
                self.assertEqual(
                    "old-index", (live / "index.html").read_text(encoding="utf-8")
                )
                lock_owner.unlink()
                publish_lock.rmdir()
                stdout, stderr = process.communicate(timeout=30)
            finally:
                if publish_lock.exists():
                    if lock_owner.exists():
                        lock_owner.unlink()
                    publish_lock.rmdir()
                if process.poll() is None:
                    process.kill()
                    process.wait(timeout=10)
                process.communicate(timeout=10)

            self.assertEqual(
                0,
                process.returncode,
                f"publisher failed:\nstdout:\n{stdout}\nstderr:\n{stderr}",
            )
            marker = json.loads(ready.read_text(encoding="utf-8"))
            index_bytes = (live / "index.html").read_bytes()
            self.assertEqual(
                hashlib.sha256(index_bytes).hexdigest(), marker["indexSha256"]
            )

    def test_web_dist_publisher_recovers_a_lock_owned_by_a_dead_process(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"
        exited_owner = subprocess.Popen(["node", "-e", "process.exit(0)"])
        exited_owner.wait(timeout=10)

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            publish_lock = root / ".gateway-web-publish.lock"
            staging.mkdir()
            live.mkdir()
            publish_lock.mkdir()
            (publish_lock / "owner.json").write_text(
                json.dumps(
                    {
                        "ownerToken": "dead-owner",
                        "pid": exited_owner.pid,
                        "acquiredAt": "2026-07-28T00:00:00.000Z",
                    }
                ),
                encoding="utf-8",
            )
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")

            result = subprocess.run(
                [
                    "node",
                    str(publisher),
                    "--staging",
                    str(staging),
                    "--live",
                    str(live),
                    "--ready",
                    str(ready),
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertEqual(
                0,
                result.returncode,
                f"publisher failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            self.assertFalse(publish_lock.exists())
            self.assertEqual("new-index", (live / "index.html").read_text(encoding="utf-8"))
            self.assertTrue(ready.is_file())

    def test_two_concurrent_web_publishers_commit_a_consistent_live_revision(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            live = root / "live"
            ready = root / ".gateway-web-ready"
            barrier = root / ".gateway-web-publish.lock"
            live.mkdir()
            barrier.mkdir()
            (live / "index.html").write_text("old-index", encoding="utf-8")

            processes = []
            for name in ("alpha", "beta"):
                staging = root / f"staging-{name}"
                (staging / "static/js").mkdir(parents=True)
                (staging / "index.html").write_text(
                    f'<script src="/ui/static/js/{name}.js"></script>',
                    encoding="utf-8",
                )
                (staging / f"static/js/{name}.js").write_text(
                    name, encoding="utf-8"
                )
                processes.append(
                    subprocess.Popen(
                        [
                            "node",
                            str(publisher),
                            "--staging",
                            str(staging),
                            "--live",
                            str(live),
                            "--ready",
                            str(ready),
                        ],
                        cwd=GATEWAY_ROOT,
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                    )
                )

            try:
                time.sleep(0.3)
                self.assertTrue(all(process.poll() is None for process in processes))
                barrier.rmdir()
                results = [process.communicate(timeout=30) for process in processes]
            finally:
                if barrier.exists():
                    barrier.rmdir()
                for process in processes:
                    if process.poll() is None:
                        process.kill()
                    process.communicate(timeout=10)

            for process, (stdout, stderr) in zip(processes, results, strict=True):
                self.assertEqual(
                    0,
                    process.returncode,
                    f"publisher failed:\nstdout:\n{stdout}\nstderr:\n{stderr}",
                )

            marker = json.loads(ready.read_text(encoding="utf-8"))
            index_bytes = (live / "index.html").read_bytes()
            self.assertEqual(
                hashlib.sha256(index_bytes).hexdigest(), marker["indexSha256"]
            )
            index = index_bytes.decode("utf-8")
            winner = "alpha" if "alpha.js" in index else "beta"
            self.assertEqual(
                winner,
                (live / f"static/js/{winner}.js").read_text(encoding="utf-8"),
            )

    def test_two_concurrent_pruning_publishers_leave_only_the_winning_revision(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            live = root / "live"
            ready = root / ".gateway-web-ready"
            barrier = root / ".gateway-web-publish.lock"
            (live / "static/js").mkdir(parents=True)
            (live / "index.html").write_text("old-index", encoding="utf-8")
            (live / "static/js/old.js").write_text("old", encoding="utf-8")
            barrier.mkdir()

            processes = []
            for name in ("alpha", "beta"):
                staging = root / f"staging-{name}"
                (staging / "static/js").mkdir(parents=True)
                (staging / "index.html").write_text(
                    f'<script src="/ui/static/js/{name}.js"></script>',
                    encoding="utf-8",
                )
                (staging / f"static/js/{name}.js").write_text(
                    name, encoding="utf-8"
                )
                processes.append(
                    subprocess.Popen(
                        [
                            "node",
                            str(publisher),
                            "--staging",
                            str(staging),
                            "--live",
                            str(live),
                            "--ready",
                            str(ready),
                            "--prune",
                        ],
                        cwd=GATEWAY_ROOT,
                        text=True,
                        stdout=subprocess.PIPE,
                        stderr=subprocess.PIPE,
                    )
                )

            try:
                time.sleep(0.3)
                self.assertTrue(all(process.poll() is None for process in processes))
                barrier.rmdir()
                results = [process.communicate(timeout=30) for process in processes]
            finally:
                if barrier.exists():
                    barrier.rmdir()
                for process in processes:
                    if process.poll() is None:
                        process.kill()
                    process.communicate(timeout=10)

            for process, (stdout, stderr) in zip(processes, results, strict=True):
                self.assertEqual(
                    0,
                    process.returncode,
                    f"publisher failed:\nstdout:\n{stdout}\nstderr:\n{stderr}",
                )

            marker = json.loads(ready.read_text(encoding="utf-8"))
            index_bytes = (live / "index.html").read_bytes()
            self.assertEqual(
                hashlib.sha256(index_bytes).hexdigest(), marker["indexSha256"]
            )
            index = index_bytes.decode("utf-8")
            winner = "alpha" if "alpha.js" in index else "beta"
            loser = "beta" if winner == "alpha" else "alpha"
            self.assertTrue((live / f"static/js/{winner}.js").is_file())
            self.assertFalse((live / f"static/js/{loser}.js").exists())
            self.assertFalse((live / "static/js/old.js").exists())

    def test_web_dist_publisher_restores_overwritten_and_added_assets_when_marker_commit_fails(self):
        publisher = (GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs").as_uri()

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            (staging / "static/js").mkdir(parents=True)
            (staging / "static/new-revision").mkdir(parents=True)
            (live / "static/js").mkdir(parents=True)
            old_index = b'<script src="/ui/static/js/app.js"></script>'
            old_marker = {
                "indexSha256": hashlib.sha256(old_index).hexdigest(),
                "publishedAt": "2026-07-28T00:00:00.000Z",
            }
            (staging / "index.html").write_bytes(
                b'<script src="/ui/static/js/app.js"></script>'
                b'<script src="/ui/static/new-revision/new-only.js"></script>'
            )
            (staging / "static/js/app.js").write_text(
                "new-app", encoding="utf-8"
            )
            (staging / "static/new-revision/new-only.js").write_text(
                "new-only", encoding="utf-8"
            )
            (live / "index.html").write_bytes(old_index)
            (live / "static/js/app.js").write_text("old-app", encoding="utf-8")
            (live / "static/js/old-only.js").write_text(
                "old-only", encoding="utf-8"
            )
            ready.write_text(json.dumps(old_marker), encoding="utf-8")

            script = f"""
                import {{ writeFile }} from 'node:fs/promises';
                import {{ publishWebDist }} from {json.dumps(publisher)};
                await publishWebDist(
                  {{
                    stagingDir: {json.dumps(str(staging))},
                    liveDir: {json.dumps(str(live))},
                    readyFile: {json.dumps(str(ready))},
                    pruneLive: true,
                  }},
                  {{
                    replaceReadyMarker: async (readyFile) => {{
                      await writeFile(readyFile, 'corrupt-marker', 'utf8');
                      throw new Error('injected ready marker failure');
                    }},
                  }},
                );
            """
            result = subprocess.run(
                ["node", "--input-type=module", "--eval", script],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertNotEqual(0, result.returncode)
            self.assertIn("injected ready marker failure", result.stderr)
            self.assertEqual(old_index, (live / "index.html").read_bytes())
            self.assertEqual(old_marker, json.loads(ready.read_text(encoding="utf-8")))
            self.assertEqual(
                "old-app", (live / "static/js/app.js").read_text(encoding="utf-8")
            )
            self.assertEqual(
                "old-only",
                (live / "static/js/old-only.js").read_text(encoding="utf-8"),
            )
            self.assertFalse((live / "static/new-revision/new-only.js").exists())
            self.assertFalse((live / "static/new-revision").exists())

    def test_web_dist_publisher_commits_ready_marker_after_prune_completes(self):
        publisher = (GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs").as_uri()

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            stale_asset = live / "static/js/stale.js"
            (staging / "static/js").mkdir(parents=True)
            stale_asset.parent.mkdir(parents=True)
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (staging / "static/js/new.js").write_text("new", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")
            stale_asset.write_text("stale", encoding="utf-8")

            script = f"""
                import {{ access, writeFile }} from 'node:fs/promises';
                import {{ publishWebDist }} from {json.dumps(publisher)};
                await publishWebDist(
                  {{
                    stagingDir: {json.dumps(str(staging))},
                    liveDir: {json.dumps(str(live))},
                    readyFile: {json.dumps(str(ready))},
                    pruneLive: true,
                  }},
                  {{
                    replaceReadyMarker: async (readyFile, marker) => {{
                      try {{
                        await access({json.dumps(str(stale_asset))});
                        throw new Error('ready marker committed before prune completed');
                      }} catch (error) {{
                        if (error?.code !== 'ENOENT') {{
                          throw error;
                        }}
                      }}
                      await writeFile(readyFile, marker, 'utf8');
                    }},
                  }},
                );
            """
            result = subprocess.run(
                ["node", "--input-type=module", "--eval", script],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertEqual(
                0,
                result.returncode,
                f"publisher failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            self.assertFalse(stale_asset.exists())
            self.assertTrue(ready.is_file())

    def test_web_dist_publisher_rolls_back_index_and_marker_when_marker_commit_fails(self):
        publisher = (GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs").as_uri()

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            staging.mkdir()
            live.mkdir()
            old_index = b"old-index"
            old_marker = {
                "indexSha256": hashlib.sha256(old_index).hexdigest(),
                "publishedAt": "2026-07-28T00:00:00.000Z",
            }
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (live / "index.html").write_bytes(old_index)
            ready.write_text(json.dumps(old_marker), encoding="utf-8")

            script = f"""
                import {{ writeFile }} from 'node:fs/promises';
                import {{ publishWebDist }} from {json.dumps(publisher)};
                await publishWebDist(
                  {{
                    stagingDir: {json.dumps(str(staging))},
                    liveDir: {json.dumps(str(live))},
                    readyFile: {json.dumps(str(ready))},
                  }},
                  {{
                    replaceReadyMarker: async (readyFile) => {{
                      await writeFile(readyFile, 'corrupt-marker', 'utf8');
                      throw new Error('injected ready marker failure');
                    }},
                  }},
                );
            """
            result = subprocess.run(
                ["node", "--input-type=module", "--eval", script],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertNotEqual(0, result.returncode)
            self.assertIn("injected ready marker failure", result.stderr)
            self.assertEqual(old_index, (live / "index.html").read_bytes())
            self.assertEqual(old_marker, json.loads(ready.read_text(encoding="utf-8")))
            residue = [
                item.name
                for item in root.rglob("*")
                if ".next-" in item.name
                or ".publish-" in item.name
                or item.name == ".gateway-web-publish.lock"
            ]
            self.assertEqual([], residue)

    def test_web_dist_publisher_prune_mode_removes_stale_live_assets(self):
        publisher = GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"

        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            root = pathlib.Path(temp_dir)
            staging = root / "staging"
            live = root / "live"
            ready = root / ".gateway-web-ready"
            (staging / "static/js").mkdir(parents=True)
            (live / "static/js").mkdir(parents=True)
            (staging / "index.html").write_text("new-index", encoding="utf-8")
            (staging / "static/js/new.js").write_text("new", encoding="utf-8")
            (live / "index.html").write_text("old-index", encoding="utf-8")
            (live / "static/js/old.js").write_text("old", encoding="utf-8")

            result = subprocess.run(
                [
                    "node",
                    str(publisher),
                    "--staging",
                    str(staging),
                    "--live",
                    str(live),
                    "--ready",
                    str(ready),
                    "--prune",
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=30,
                check=False,
            )

            self.assertEqual(
                0,
                result.returncode,
                f"publisher failed:\nstdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            self.assertFalse((live / "static/js/old.js").exists())
            self.assertTrue((live / "static/js/new.js").is_file())

    def test_web_dist_publisher_retries_transient_cleanup_failures(self):
        publisher = (
            GATEWAY_ROOT / "apps/desktop/tools/publish-web-dist.mjs"
        ).read_text(encoding="utf-8")

        self.assertIn("CLEANUP_MAX_ATTEMPTS", publisher)
        self.assertIn("CLEANUP_RETRY_CODES", publisher)
        self.assertIn("async function removePathWithRetry", publisher)
        self.assertIn("await removePathWithRetry", publisher)

    def test_docker_builder_uses_node20_ui_stage_and_prebuilt_web_assets(self):
        dockerfile = (GATEWAY_ROOT / "Dockerfile").read_text(encoding="utf-8")
        build_script = (GATEWAY_ROOT / "build.rs").read_text(encoding="utf-8")
        cargo = (GATEWAY_ROOT / "Cargo.toml").read_text(encoding="utf-8")

        self.assertIn("FROM node:20-bookworm-slim AS ui-builder", dockerfile)
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

    def test_docker_helper_scripts_and_release_bundle_are_repository_owned(self):
        required = [
            "tools/deploy-gateway-docker.ps1",
            "tools/verify-gateway-docker-stack.ps1",
            "tools/export-gateway-docker-deploy-bundle.ps1",
        ]
        missing = [path for path in required if not (GATEWAY_ROOT / path).is_file()]
        self.assertEqual([], missing, f"missing Docker helper scripts: {missing}")

        release = (GATEWAY_ROOT / ".github/workflows/release-tag.yml").read_text(
            encoding="utf-8"
        )
        docker = (GATEWAY_ROOT / ".github/workflows/docker.yml").read_text(
            encoding="utf-8"
        )

        self.assertIn("verify-gateway-docker-stack.ps1", docker)
        self.assertIn("export-gateway-docker-deploy-bundle.ps1", release)
        self.assertIn("Gateway-${{ env.GATEWAY_TAG }}-docker-deploy.zip", release)
        self.assertIn(
            "release/Gateway/packages/Gateway-${{ env.GATEWAY_TAG }}-docker-deploy.zip.sha256",
            release,
        )

    def test_packager_includes_deploy_directory_in_release_layout(self):
        script = (GATEWAY_ROOT / "tools/package-gateway-release.ps1").read_text(
            encoding="utf-8"
        )

        self.assertIn('$deploySource = Join-Path $gatewayRoot "deploy"', script)
        self.assertIn("$deployPayloadRelativePaths", script)
        for relative_path in (
            ".env.example",
            "README.md",
            "docker-compose.local.yml",
            "docker-compose.yml",
            "docker-deploy.sh",
            "docker-entrypoint.sh",
        ):
            self.assertIn(f'"{relative_path}"', script)
        self.assertNotIn(
            'Copy-FilteredTree -Source $deploySource -Destination (Join-Path $staging "deploy")',
            script,
        )
        self.assertIn('deploy = "deploy/"', script)
        self.assertIn('-Kind "docker-deploy"', script)

    def test_docker_deploy_bundle_uses_a_release_payload_allowlist(self):
        script = (
            GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"
        ).read_text(encoding="utf-8")

        self.assertIn("$deployPayloadRelativePaths", script)
        for relative_path in (
            ".env.example",
            "README.md",
            "docker-compose.local.yml",
            "docker-compose.yml",
            "docker-deploy.sh",
            "docker-entrypoint.sh",
        ):
            self.assertIn(f'"{relative_path}"', script)
        self.assertNotIn(
            'Copy-RepositoryPayload -Source (Join-Path $gatewayRoot "deploy")',
            script,
        )

    def test_docker_deploy_bundle_rejects_unsafe_version_ids(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"),
                    "-VersionId",
                    "unsafe/version",
                    "-OutputDir",
                    temporary_directory,
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )

            self.assertNotEqual(0, result.returncode)
            self.assertIn("unsupported path characters", result.stdout + result.stderr)
            self.assertEqual([], list(pathlib.Path(temporary_directory).iterdir()))

    def test_docker_deploy_bundle_refuses_to_overwrite_existing_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            command = [
                powershell_executable(),
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"),
                "-VersionId",
                "contract-v1",
                "-OutputDir",
                temporary_directory,
            ]
            first = subprocess.run(
                command,
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )
            self.assertEqual(0, first.returncode, msg=first.stdout + first.stderr)

            output_root = pathlib.Path(temporary_directory)
            zip_path = output_root / "Gateway-contract-v1-docker-deploy.zip"
            hash_path = output_root / "Gateway-contract-v1-docker-deploy.zip.sha256"
            original_zip = zip_path.read_bytes()
            original_hash = hash_path.read_bytes()

            second = subprocess.run(
                command,
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )

            self.assertNotEqual(0, second.returncode)
            self.assertIn("already exists and is immutable", second.stdout + second.stderr)
            self.assertEqual(original_zip, zip_path.read_bytes())
            self.assertEqual(original_hash, hash_path.read_bytes())

    def test_docker_deploy_bundle_resumes_an_interrupted_checksum_publication(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            output_root = pathlib.Path(temporary_directory)
            version_id = "contract-interrupted-publication"
            zip_path = output_root / f"Gateway-{version_id}-docker-deploy.zip"
            hash_path = pathlib.Path(str(zip_path) + ".sha256")
            journal_path = pathlib.Path(str(zip_path) + ".publishing.json")
            zip_bytes = b"interrupted-but-complete-zip"
            zip_path.write_bytes(zip_bytes)
            zip_hash = hashlib.sha256(zip_bytes).hexdigest()
            hash_record = f"{zip_hash} *{zip_path.name}\n"
            journal_path.write_text(
                json.dumps(
                    {
                        "schemaVersion": 1,
                        "zipName": zip_path.name,
                        "zipSha256": zip_hash,
                        "hashRecord": hash_record,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"),
                    "-VersionId",
                    version_id,
                    "-OutputDir",
                    temporary_directory,
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )

            self.assertEqual(0, result.returncode, msg=result.stdout + result.stderr)
            self.assertEqual(zip_bytes, zip_path.read_bytes())
            self.assertEqual(hash_record, hash_path.read_text(encoding="utf-8"))
            self.assertFalse(journal_path.exists())

    def test_windows_release_compressor_resumes_an_interrupted_checksum_publication(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            release_dir = root / "release"
            output_dir = root / "packages"
            release_dir.mkdir()
            output_dir.mkdir()
            for name in ("manifest.json", "checksums.sha256", "gateway.exe", "gateway-ui.exe"):
                (release_dir / name).write_bytes(b"fixture")

            version_id = "contract-interrupted-windows-publication"
            zip_path = output_dir / f"Gateway-{version_id}-windows-x64.zip"
            checksum_path = pathlib.Path(str(zip_path) + ".sha256")
            journal_path = pathlib.Path(str(zip_path) + ".publishing.json")
            zip_bytes = b"interrupted-but-complete-windows-zip"
            zip_path.write_bytes(zip_bytes)
            zip_hash = hashlib.sha256(zip_bytes).hexdigest()
            checksum_record = f"{zip_hash}  {zip_path.name}{os.linesep}"
            journal_path.write_text(
                json.dumps(
                    {
                        "schemaVersion": 1,
                        "zipName": zip_path.name,
                        "zipSha256": zip_hash,
                        "checksumRecord": checksum_record,
                    }
                )
                + "\n",
                encoding="utf-8",
            )

            result = subprocess.run(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(GATEWAY_ROOT / "tools/compress-gateway-release.ps1"),
                    "-ReleaseDir",
                    str(release_dir),
                    "-OutputDir",
                    str(output_dir),
                    "-VersionId",
                    version_id,
                ],
                cwd=GATEWAY_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=60,
                check=False,
            )

            self.assertEqual(0, result.returncode, msg=result.stdout + result.stderr)
            self.assertEqual(zip_bytes, zip_path.read_bytes())
            with checksum_path.open("r", encoding="ascii", newline="") as checksum_file:
                self.assertEqual(checksum_record, checksum_file.read())
            self.assertFalse(journal_path.exists())

    def test_docker_deploy_bundle_rolls_back_new_zip_when_hash_publish_fails(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            fixture_root = temporary_root / "Gateway"
            fixture_tools = fixture_root / "tools"
            fixture_deploy = fixture_root / "deploy"
            fixture_tools.mkdir(parents=True)
            fixture_deploy.mkdir(parents=True)

            exporter = GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"
            (fixture_tools / exporter.name).write_bytes(exporter.read_bytes())
            fixture_files = {
                "README.md": b"x" * (16 * 1024 * 1024),
                "README.zh-CN.md": b"# Gateway\n",
                "LICENSE": b"fixture license\n",
                "tools/deploy-gateway-docker.ps1": b"param()\n",
                "deploy/.env.example": b"IMAGE_TAG=latest\n",
                "deploy/README.md": b"# Deploy\n",
                "deploy/docker-compose.local.yml": b"services: {}\n",
                "deploy/docker-compose.yml": b"services: {}\n",
                "deploy/docker-deploy.sh": b"#!/usr/bin/env bash\n",
                "deploy/docker-entrypoint.sh": b"#!/usr/bin/env sh\n",
            }
            for relative_path, payload in fixture_files.items():
                destination = fixture_root / pathlib.PurePosixPath(relative_path)
                destination.parent.mkdir(parents=True, exist_ok=True)
                destination.write_bytes(payload)

            output_root = temporary_root / "output"
            staging_parent = temporary_root / "temp"
            staging_parent.mkdir()
            version_id = "contract-hash-publish-failure"
            zip_path = output_root / f"Gateway-{version_id}-docker-deploy.zip"
            hash_path = pathlib.Path(str(zip_path) + ".sha256")
            environment = os.environ.copy()
            environment["TEMP"] = str(staging_parent)
            environment["TMP"] = str(staging_parent)
            process = subprocess.Popen(
                [
                    powershell_executable(),
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-ExecutionPolicy",
                    "Bypass",
                    "-File",
                    str(fixture_tools / exporter.name),
                    "-VersionId",
                    version_id,
                    "-OutputDir",
                    str(output_root),
                ],
                cwd=fixture_root,
                env=environment,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

            staging_root = None
            deadline = time.monotonic() + 30
            while process.poll() is None and time.monotonic() < deadline:
                candidates = list(staging_parent.glob("gateway-docker-deploy-*"))
                if candidates:
                    staging_root = candidates[0]
                    break
                time.sleep(0.002)

            self.assertIsNotNone(staging_root, "exporter completed before staging was observable")
            hash_path.mkdir(parents=True)
            collision_marker = hash_path / "existing-artifact.txt"
            collision_marker.write_text("preserve me\n", encoding="utf-8")
            stdout, stderr = process.communicate(timeout=60)

            self.assertNotEqual(0, process.returncode, msg=stdout + stderr)
            self.assertFalse(zip_path.exists(), "new ZIP was not rolled back")
            self.assertTrue(hash_path.is_dir(), "existing hash-path artifact was removed")
            self.assertEqual("preserve me\n", collision_marker.read_text(encoding="utf-8"))
            self.assertFalse(staging_root.exists(), "export staging directory was not cleaned")

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


if __name__ == "__main__":
    unittest.main()
