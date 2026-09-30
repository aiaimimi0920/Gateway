"""Public candidates use examples; local route data stays in the source tree."""

import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import unittest
from unittest import mock

from gateway_package_fixture import GatewayPackageFixture, REPO_ROOT, RUNTIME_SMOKE, UI_SMOKE


LOCAL_ROUTES = b"routes: []\n# synthetic-local-only-configuration\n"
EXAMPLE_ROUTES = b"routes: []\n# synthetic-safe-example\n"


class GatewayPackageExampleRoutesTests(GatewayPackageFixture):
    def _isolate_fixture_git_root(self, source: pathlib.Path):
        # Keep ancestor checkouts from affecting this synthetic tree's provenance.
        for arguments in (
            ["init", "--quiet"],
            ["-c", "user.name=Gateway Test", "-c", "user.email=gateway-test@example.invalid",
             "commit", "--quiet", "--allow-empty", "-m", "synthetic fixture root"],
        ):
            subprocess.run(
                ["git", "-C", str(source), *arguments],
                capture_output=True, text=True, check=True,
            )

    def _exercise_route_mode(self, use_example_routes: bool, omit_local: bool = False):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            source, release = root / "source", root / "release"
            self._write_fixture(source)
            self._isolate_fixture_git_root(source)
            local = source / "routes.yaml"
            local.write_bytes(LOCAL_ROUTES)
            (source / "routes.example.yaml").write_bytes(EXAMPLE_ROUTES)
            if omit_local:
                local.unlink()
            result = self._run_packager(source, release, use_example_routes=use_example_routes)
            self.assertEqual(0, result.returncode, result.stdout + result.stderr)
            destination = release / "contract-v1"
            expected = EXAMPLE_ROUTES if use_example_routes else LOCAL_ROUTES
            self.assertEqual(expected, (destination / "routes.yaml").read_bytes())
            self.assertEqual(EXAMPLE_ROUTES, (destination / "routes.example.yaml").read_bytes())
            self.assertEqual(not omit_local, local.exists())
            if not omit_local:
                self.assertEqual(LOCAL_ROUTES, local.read_bytes())
            manifest = json.loads((destination / "manifest.json").read_text(encoding="utf-8"))
            routes = {item["path"]: item for item in manifest["supportFiles"] if item["kind"] == "route-config"}
            self.assertEqual({"routes.yaml", "routes.example.yaml"}, set(routes))
            checksums = dict(
                reversed(line.split("  ", 1))
                for line in (destination / "checksums.sha256").read_text(encoding="utf-8").splitlines()
            )
            payload_names = {
                path.relative_to(destination).as_posix()
                for path in destination.rglob("*")
                if path.is_file() and path.name != "checksums.sha256"
            }
            self.assertEqual(payload_names, set(checksums))
            for record in manifest["files"]:
                name = record["path"]
                digest = hashlib.sha256((destination / name).read_bytes()).hexdigest()
                self.assertEqual(digest, record["sha256"])
                self.assertEqual(digest, checksums[name])
            # Both integrity checks inspect metadata without launching the fake EXEs.
            for script, options in ((RUNTIME_SMOKE, ("-IntegrityOnly",)), (UI_SMOKE, ())):
                checked = self._run_smoke(script, destination, *options)
                self.assertEqual(0, checked.returncode, checked.stdout + checked.stderr)
            if use_example_routes:
                for path in destination.rglob("*"):
                    if path.is_file():
                        self.assertNotIn(LOCAL_ROUTES, path.read_bytes(), path.name)

    def test_example_mode_preserves_local_routes_and_excludes_them_from_every_payload(self):
        self._exercise_route_mode(True)

    def test_example_mode_does_not_require_a_local_routes_file(self):
        self._exercise_route_mode(True, omit_local=True)

    def test_local_packaging_keeps_its_existing_route_selection(self):
        self._exercise_route_mode(False)

    def test_missing_example_fails_closed_without_falling_back_to_local_routes(self):
        with tempfile.TemporaryDirectory() as temporary:
            root = pathlib.Path(temporary)
            source, release = root / "source", root / "release"
            self._write_fixture(source)
            self._isolate_fixture_git_root(source)
            local = source / "routes.yaml"
            local.write_bytes(LOCAL_ROUTES)
            (source / "routes.example.yaml").unlink()
            result = self._run_packager(source, release, use_example_routes=True)
            self.assertNotEqual(0, result.returncode)
            self.assertFalse((release / "contract-v1").exists())
            self.assertEqual(LOCAL_ROUTES, local.read_bytes())

    @unittest.skipUnless(os.name == "nt", "Windows PowerShell 5.1 is Windows-only")
    def test_example_mode_on_windows_powershell_51(self):
        executable = shutil.which("powershell")
        self.assertIsNotNone(executable)
        with mock.patch("gateway_package_fixture.powershell_executable", return_value=executable):
            self._exercise_route_mode(True)

    def test_automated_uploads_always_use_example_routes(self):
        for filename, step in (
            ("build-windows.yml", "Package Gateway release candidate"),
            ("release-tag.yml", "Package Gateway release"),
        ):
            with self.subTest(workflow=filename):
                workflow = (REPO_ROOT / ".github/workflows" / filename).read_text(encoding="utf-8")
                package = workflow.split(f"- name: {step}", 1)[1].split("- name:", 1)[0]
                self.assertIn("-UseExampleRoutes", package)


if __name__ == "__main__":
    unittest.main()
