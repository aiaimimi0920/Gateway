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


class GatewayStandaloneSourceStateTests(GatewayRepositoryTextFixture, unittest.TestCase):
    def test_release_artifacts_are_excluded_from_source_fingerprints(self):
        scripts = {
            "build": GATEWAY_ROOT / "tools/build-gateway-release.ps1",
            "package": GATEWAY_ROOT / "tools/package-release/source-provenance.ps1",
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
            "package": GATEWAY_ROOT / "tools/package-release/source-provenance.ps1",
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
        package_script += "\n" + (
            GATEWAY_ROOT / "tools/package-release/source-provenance.ps1"
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
            self.assertEqual(initial["build"]["fileCount"], 4)

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
