import json
import pathlib
import re
import subprocess
import tempfile
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
        self.assertIn("cargo build --locked --release --bin neuro-gateway", script)
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


if __name__ == "__main__":
    unittest.main()
