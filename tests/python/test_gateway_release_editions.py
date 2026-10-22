"""Edition plans and real package selection must not depend on stale UI artifacts."""
import json
import hashlib
import pathlib
import subprocess
import tempfile

from gateway_package_fixture import GatewayPackageFixture, PACKAGER, REPO_ROOT, UI_SMOKE
from powershell_test_utils import powershell_environment, powershell_executable


class GatewayReleaseEditionsTests(GatewayPackageFixture):
    def _write_edition_provenance(self, source, edition, *, wrong_capabilities=False):
        relative = pathlib.PurePosixPath("target/release-editions") / edition / "gateway.exe"
        artifact = source / relative
        artifact.parent.mkdir(parents=True, exist_ok=True)
        artifact.write_bytes(f"{edition}-gateway\n".encode())
        state = self._source_tree_state(source)
        provenance = artifact.parent / "gateway-build-provenance.json"
        management_ui = edition != "api"
        desktop_ui = edition == "desktop"
        provenance.write_text(
            json.dumps({
                "schemaVersion": 1,
                "edition": edition,
                "capabilities": {
                    "managementUi": not management_ui if wrong_capabilities else management_ui,
                    "desktopUi": desktop_ui,
                },
                "sourceTreeFingerprint": state["fingerprint"],
                "sourceTree": state,
                "artifacts": [{
                    "path": relative.as_posix(),
                    "bytes": artifact.stat().st_size,
                    "sha256": hashlib.sha256(artifact.read_bytes()).hexdigest(),
                }],
            }),
            encoding="utf-8",
        )

    def _run_edition_packager(self, source, releases, edition):
        executable = powershell_executable()
        return subprocess.run(
            [executable, "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(PACKAGER),
             "-SourceRoot", str(source), "-VersionId", f"{edition}-v1", "-ReleaseRoot", str(releases),
             "-StageOnly", "-SkipBuild", "-Clean", "-Edition", edition, "-AllowCustomReleaseRoot"],
            cwd=REPO_ROOT, env=powershell_environment(executable), text=True,
            stdout=subprocess.PIPE, stderr=subprocess.PIPE,
        )

    def test_build_plans_separate_frontend_and_desktop_capabilities(self):
        executable = powershell_executable()
        for edition in ("api", "web", "desktop", "all"):
            with self.subTest(edition=edition):
                result = subprocess.run(
                    [executable, "-NoProfile", "-File",
                     str(REPO_ROOT / "tools/build-gateway-release.ps1"),
                     "-Edition", edition, "-DryRun"],
                    capture_output=True, text=True,
                    env=powershell_environment(executable), check=False,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertEqual("build:web" in result.stdout, edition != "api")
                self.assertEqual("tauri" in result.stdout, edition in ("desktop", "all"))
                if edition == "api":
                    self.assertIn("--no-default-features --features runtime-defaults", result.stdout)

    def test_release_excludes_tests_and_generated_screenshots_everywhere(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source, releases = root / "source", root / "releases"
            self._write_fixture(source)
            forbidden = (
                "scripts/tests/.gemini-canvas-browser-pool-testable-fixture.mjs",
                "scripts/nested/tests/test.mjs", "scripts/worker.test.mjs",
                "scripts/undefined/screenshot.png", "scripts/artifacts/report.json",
            )
            for name in forbidden:
                path = source / name
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_bytes(b"must not ship")
            result = self._run_packager(source, releases)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            package = releases / "contract-v1"
            manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
            records = {record["path"] for record in manifest["files"]}
            checksums = (package / "checksums.sha256").read_text(encoding="utf-8")
            for name in forbidden:
                self.assertFalse((package / name).exists(), name)
                self.assertNotIn(name, records)
                self.assertNotIn(name, checksums)

    def test_api_and_web_packages_ignore_stale_desktop_artifacts(self):
        for edition in ("api", "web"):
            with self.subTest(edition=edition), tempfile.TemporaryDirectory() as directory:
                root = pathlib.Path(directory)
                source, releases = root / "source", root / "releases"
                self._write_fixture(source)
                self._write_edition_provenance(source, edition)
                result = self._run_edition_packager(source, releases, edition)
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                package = releases / f"{edition}-v1"
                self.assertTrue((package / "gateway.exe").is_file())
                self.assertFalse((package / "gateway-ui.exe").exists())
                manifest = json.loads((package / "manifest.json").read_text(encoding="utf-8"))
                self.assertEqual(manifest["edition"], edition)
                self.assertEqual(manifest["capabilities"], {
                    "managementUi": edition == "web", "desktopUi": False,
                })
                self.assertEqual({item["name"] for item in manifest["exes"]}, {"gateway.exe"})
                smoke = self._run_smoke(UI_SMOKE, package)
                self.assertEqual(smoke.returncode, 0, smoke.stdout + smoke.stderr)
                launch = self._run_smoke(UI_SMOKE, package, "-LaunchUi")
                self.assertNotEqual(launch.returncode, 0)

    def test_packager_rejects_edition_capability_mismatch(self):
        with tempfile.TemporaryDirectory() as directory:
            root = pathlib.Path(directory)
            source, releases = root / "source", root / "releases"
            self._write_fixture(source)
            self._write_edition_provenance(source, "api", wrong_capabilities=True)
            result = self._run_edition_packager(source, releases, "api")
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("edition/capability mismatch", result.stdout + result.stderr)
            self.assertFalse((releases / "api-v1").exists())
