"""Daily packaging keeps the release trust boundary without compiling paid/live workloads."""
import hashlib
import json
import pathlib
import subprocess
import shutil
import tempfile
import tomllib

from gateway_package_fixture import GatewayPackageFixture, PACKAGER, REPO_ROOT, UI_SMOKE
from powershell_test_utils import powershell_environment, powershell_executable


class GatewayDevtestTests(GatewayPackageFixture):
    def run_ps(self, *args):
        exe = powershell_executable()
        return subprocess.run(
            [exe, "-NoProfile", "-ExecutionPolicy", "Bypass", *map(str, args)],
            cwd=REPO_ROOT, env=powershell_environment(exe),
            capture_output=True, text=True, check=False,
        )

    def test_profiles_are_independent_and_incremental(self):
        for relative in ("Cargo.toml", "apps/desktop/src-tauri/Cargo.toml"):
            profile = tomllib.loads((REPO_ROOT / relative).read_text(encoding="utf-8"))["profile"]["devtest"]
            self.assertEqual(profile["opt-level"], 1)
            self.assertFalse(profile["lto"])
            self.assertTrue(profile["incremental"])
            self.assertFalse(profile["debug-assertions"])

    def test_dry_run_respects_jobs_and_edition(self):
        for edition in ("api", "web", "desktop"):
            result = self.run_ps("-File", REPO_ROOT / "tools/package-gateway-devtest.ps1",
                                 "-DryRun", "-Edition", edition, "-Jobs", "6")
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("--jobs 6", result.stdout)
            self.assertIn("--profile devtest", result.stdout)
            self.assertEqual("build:web" in result.stdout, edition != "api")
            self.assertEqual("tauri/custom-protocol" in result.stdout, edition == "desktop")
            self.assertNotIn("--release", result.stdout)

    def write_devtest_receipt(self, source, kind="devtest"):
        relative = "target/devtest-editions/web/gateway.exe"
        artifact = source / relative
        artifact.parent.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(b"devtest-fixture")
        state = self._source_tree_state(source)
        receipt = {
            "schemaVersion": 1, "edition": "web", "buildKind": kind,
            "capabilities": {"managementUi": True, "desktopUi": False},
            "sourceTreeFingerprint": state["fingerprint"], "sourceTree": state,
            "artifacts": [{"path": relative, "bytes": artifact.stat().st_size,
                           "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest()}],
        }
        (artifact.parent / "gateway-build-provenance.json").write_text(json.dumps(receipt), encoding="utf-8")

    def test_real_packager_enforces_kind_and_integrity(self):
        for recorded_kind in ("devtest", "release"):
            with self.subTest(kind=recorded_kind), tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                source, releases = root / "source", root / "releases"
                self._write_fixture(source)
                self.write_devtest_receipt(source, recorded_kind)
                result = self.run_ps("-File", PACKAGER, "-SourceRoot", source,
                                     "-ReleaseRoot", releases, "-AllowCustomReleaseRoot",
                                     "-VersionId", "gateway-devtest-contract", "-Edition", "web",
                                     "-BuildKind", "devtest", "-SkipBuild")
                if recorded_kind == "release":
                    self.assertNotEqual(result.returncode, 0)
                    self.assertIn("build kind mismatch", result.stdout + result.stderr)
                    continue
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                package = releases / "gateway-devtest-contract"
                manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
                self.assertEqual(manifest["buildKind"], "devtest")
                smoke = self._run_smoke(UI_SMOKE, package)
                self.assertEqual(smoke.returncode, 0, smoke.stdout + smoke.stderr)

    def test_devtest_cannot_use_a_release_name(self):
        result = self.run_ps("-File", REPO_ROOT / "tools/package-gateway-devtest.ps1",
                             "-DryRun", "-VersionId", "gateway-release-wrong")
        self.assertNotEqual(result.returncode, 0)

    def test_release_packager_rejects_devtest_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            source = pathlib.Path(directory) / "source"
            self._write_fixture(source)
            self._write_build_provenance(source)
            path = source / "target/release/gateway-build-provenance.json"
            receipt = json.loads(path.read_text(encoding="utf-8"))
            receipt["buildKind"] = "devtest"
            path.write_text(json.dumps(receipt), encoding="utf-8")
            result = self._run_packager(source, pathlib.Path(directory) / "releases", write_provenance=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("build kind mismatch", result.stdout + result.stderr)

    def test_builder_writes_receipt_and_fails_closed_on_compiler_error(self):
        for exit_code in (0, 1):
            with self.subTest(exit_code=exit_code), tempfile.TemporaryDirectory() as directory:
                source = pathlib.Path(directory)
                for name in ("build-gateway-devtest.ps1", "release-editions.ps1",
                             "package-release/artifact-copy.ps1", "package-release/source-provenance.ps1"):
                    dest = source / "tools" / name
                    dest.parent.mkdir(parents=True, exist_ok=True)
                    shutil.copyfile(REPO_ROOT / "tools" / name, dest)
                # Substitute only the compiler process; execute the actual builder/receipt code.
                command = f"""
function rtk {{
    New-Item -ItemType Directory -Path target/devtest -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path (Get-Location) 'target/devtest/gateway.exe'), 'fixture')
    $global:LASTEXITCODE={exit_code}
}}
& '{source.as_posix()}/tools/build-gateway-devtest.ps1' -Edition api -Jobs 2
"""
                result = self.run_ps("-Command", command)
                path = source / "target/devtest-editions/api/gateway-build-provenance.json"
                if exit_code:
                    self.assertNotEqual(result.returncode, 0)
                    self.assertFalse(path.exists())
                else:
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    receipt = json.loads(path.read_text(encoding="utf-8"))
                    self.assertEqual(receipt["buildKind"], "devtest")
                    self.assertEqual(receipt["buildSettings"]["jobs"], 2)
