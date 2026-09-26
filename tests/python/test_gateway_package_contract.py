import json
import pathlib
import subprocess
import tempfile
import time
import unittest

from gateway_package_fixture import (
    GATEWAY_ROOT,
    PACKAGER,
    REPO_ROOT,
    RUNTIME_SMOKE,
    UI_SMOKE,
    GatewayPackageFixture,
)
from powershell_test_utils import powershell_executable


class GatewayPackageContractTests(GatewayPackageFixture):
    def test_packager_source_fingerprint_ignores_git_ignored_local_files(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            (source_root / ".gitignore").write_text(
                "/.env\n/apps/desktop/dist/\n__pycache__/\n",
                encoding="utf-8",
            )
            ignored_files = {
                ".env": "GATEWAY_SECRET=fixture-only\n",
                "apps/desktop/dist/bundle.js": "generated-v1\n",
                "tests/python/__pycache__/contract.pyc": "cache-v1\n",
            }
            for relative, content in ignored_files.items():
                path = source_root / pathlib.PurePosixPath(relative)
                path.parent.mkdir(parents=True, exist_ok=True)
                path.write_text(content, encoding="utf-8")

            subprocess.run(
                ["git", "init"],
                cwd=source_root,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                check=True,
            )
            subprocess.run(
                ["git", "add", "."],
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
            self._write_build_provenance(source_root)

            for relative in ignored_files:
                path = source_root / pathlib.PurePosixPath(relative)
                path.write_text(path.read_text(encoding="utf-8") + "changed\n", encoding="utf-8")

            result = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            manifest = json.loads(
                (release_root / "contract-v1" / "manifest.json").read_text(encoding="utf-8")
            )
            self.assertEqual(
                manifest["sourceTreeFingerprint"],
                self._source_tree_fingerprint(source_root),
            )

    def test_repository_local_release_root_does_not_change_source_fingerprint(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            source_root = pathlib.Path(temporary_directory) / "Gateway-source"
            release_root = source_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)

            result = self._run_packager(
                source_root,
                release_root,
                allow_custom_release_root=False,
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            destination = release_root / "contract-v1"
            self.assertTrue(destination.is_dir())
            manifest = json.loads(
                (destination / "manifest.json").read_text(encoding="utf-8")
            )
            provenance = json.loads(
                (destination / "gateway-build-provenance.json").read_text(
                    encoding="utf-8"
                )
            )
            self.assertEqual(
                manifest["sourceTreeFingerprint"],
                provenance["sourceTreeFingerprint"],
            )

    def test_gateway_source_owns_packaged_canary_and_runner_resolves_it_locally(self):
        canary = GATEWAY_ROOT / "scripts" / "invoke-gateway-live-provider-canary.ps1"
        runner = GATEWAY_ROOT / "tools" / "run-gateway-line-evidence.ps1"

        self.assertTrue(canary.is_file())
        self.assertTrue((GATEWAY_ROOT / ".env.example").is_file())
        self.assertIn(
            'Join-Path $GatewayRoot "scripts\\invoke-gateway-live-provider-canary.ps1"',
            runner.read_text(encoding="utf-8"),
        )

    def test_portable_docs_mark_the_full_evidence_runner_as_source_only(self):
        documentation_paths = (
            GATEWAY_ROOT / "README.md",
            GATEWAY_ROOT / "docs" / "operations-manual.md",
            GATEWAY_ROOT / "docs" / "provider-evidence.md",
            GATEWAY_ROOT / "docs" / "evidence" / "README.md",
        )

        for path in documentation_paths:
            content = path.read_text(encoding="utf-8")
            self.assertIn("source repository only", content, msg=str(path))
            self.assertIn(
                "scripts/invoke-gateway-live-provider-canary.ps1",
                content,
                msg=str(path),
            )

    def test_skip_build_rejects_missing_or_stale_provenance_without_partial_output(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)

            # No provenance at all: packaging must fail before creating a
            # destination or leaving a staging directory behind.
            provenance = source_root / "target" / "release" / "gateway-build-provenance.json"
            result = self._run_packager_without_provenance(source_root, release_root)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse(release_root.exists())
            self.assertFalse(provenance.exists())

            self._write_build_provenance(source_root)
            payload = json.loads(provenance.read_text(encoding="utf-8"))
            payload["sourceTreeFingerprint"] = "0" * 64
            provenance.write_text(json.dumps(payload), encoding="utf-8")
            result = self._run_packager(source_root, release_root, write_provenance=False)
            self.assertNotEqual(result.returncode, 0)
            self.assertFalse((release_root / "contract-v1").exists())

    def test_skip_build_rejects_inconsistent_nested_source_provenance(self):
        mutations = {
            "algorithm": "sha256-incompatible-source-list",
            "fingerprint": "f" * 64,
            "fileCount": -1,
        }
        for field, invalid_value in mutations.items():
            with self.subTest(field=field), tempfile.TemporaryDirectory() as temporary_directory:
                temporary_root = pathlib.Path(temporary_directory)
                source_root = temporary_root / "Gateway-source"
                release_root = temporary_root / "release" / "Gateway"
                source_root.mkdir(parents=True)
                self._write_fixture(source_root)
                self._write_build_provenance(source_root)
                provenance = (
                    source_root / "target" / "release" / "gateway-build-provenance.json"
                )
                payload = json.loads(provenance.read_text(encoding="utf-8"))
                payload["sourceTree"][field] = invalid_value
                provenance.write_text(json.dumps(payload), encoding="utf-8")

                result = self._run_packager(
                    source_root,
                    release_root,
                    write_provenance=False,
                )

                self.assertNotEqual(result.returncode, 0, msg=result.stdout + result.stderr)
                self.assertIn("build provenance", (result.stdout + result.stderr).lower())
                self.assertFalse((release_root / "contract-v1").exists())

    def test_existing_evidence_provenance_blocks_release_without_overwrite(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            self._write_build_provenance(source_root)

            evidence_provenance = (
                source_root
                / "target/release-evidence/contract-v1/gateway-build-provenance.json"
            )
            evidence_provenance.parent.mkdir(parents=True)
            sentinel = b"immutable-evidence-sentinel\n"
            evidence_provenance.write_bytes(sentinel)

            result = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("immutable", result.stderr.lower())
            self.assertEqual(evidence_provenance.read_bytes(), sentinel)
            self.assertFalse((release_root / "contract-v1").exists())
            if release_root.exists():
                self.assertFalse(
                    any(
                        path.name.startswith(".contract-v1.staging-")
                        for path in release_root.iterdir()
                    )
                )

    def test_packager_rolls_back_a_new_release_when_evidence_publish_fails(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            self._write_build_provenance(source_root)

            evidence_root = source_root / "target" / "release-evidence"
            sentinel = b"evidence-parent-obstruction\n"
            evidence_root.write_bytes(sentinel)

            failed = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )

            self.assertNotEqual(failed.returncode, 0)
            # Ignore PowerShell host wrapping without weakening the error phrase.
            normalized_failure = "".join((failed.stdout + failed.stderr).lower().split())
            self.assertIn(
                "".join("evidence provenance publication failed; the new release was rolled back".split()),
                normalized_failure,
            )
            self.assertFalse((release_root / "contract-v1").exists())
            self.assertTrue(evidence_root.is_file())
            self.assertEqual(evidence_root.read_bytes(), sentinel)
            if release_root.exists():
                self.assertFalse(any(release_root.glob(".contract-v1.staging-*")))

            evidence_root.unlink()
            retry = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )
            self.assertEqual(
                retry.returncode,
                0,
                msg=f"stdout:\n{retry.stdout}\nstderr:\n{retry.stderr}",
            )
            self.assertTrue((release_root / "contract-v1").is_dir())

    def test_packager_rejects_source_changes_during_staging(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            for index in range(250):
                path = source_root / "scripts" / f"mutation-padding-{index:03d}.txt"
                path.write_bytes((f"padding-{index}\n" * 64).encode("ascii"))
            self._write_build_provenance(source_root)

            command = [
                powershell_executable(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(PACKAGER),
                "-SourceRoot",
                str(source_root),
                "-VersionId",
                "contract-v1",
                "-ReleaseRoot",
                str(release_root),
                "-StageOnly",
                "-SkipBuild",
                "-AllowCustomReleaseRoot",
            ]
            process = subprocess.Popen(
                command,
                cwd=REPO_ROOT,
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )

            staging_path = None
            deadline = time.monotonic() + 30
            while process.poll() is None and time.monotonic() < deadline:
                if release_root.exists():
                    staging_candidates = list(release_root.glob(".contract-v1.staging-*"))
                    if staging_candidates:
                        staging_path = staging_candidates[0]
                        break
                time.sleep(0.002)

            self.assertIsNotNone(staging_path, "packager completed before staging could be observed")
            release_source = source_root / "docs" / "release" / "source-notes.md"
            release_source.write_bytes(
                release_source.read_bytes() + b"changed during staging\n"
            )
            stdout, stderr = process.communicate(timeout=60)

            self.assertNotEqual(process.returncode, 0, msg=stdout + stderr)
            self.assertIn("source tree changed during packaging", (stdout + stderr).lower())
            self.assertFalse((release_root / "contract-v1").exists())
            self.assertFalse(
                (
                    source_root
                    / "target/release-evidence/contract-v1/gateway-build-provenance.json"
                ).exists()
            )
            self.assertFalse(
                any(release_root.glob(".contract-v1.staging-*")),
                msg=f"staging directory was not cleaned: {staging_path}",
            )

    def test_missing_environment_template_creates_no_release_or_evidence(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            (source_root / ".env.example").unlink()
            self._write_build_provenance(source_root)

            result = self._run_packager(
                source_root,
                release_root,
                write_provenance=False,
            )

            self.assertNotEqual(result.returncode, 0)
            self.assertIn("environment template", result.stderr.lower())
            self.assertFalse((release_root / "contract-v1").exists())
            self.assertFalse(
                (
                    source_root
                    / "target/release-evidence/contract-v1/gateway-build-provenance.json"
                ).exists()
            )

    def test_custom_release_root_requires_explicit_test_opt_in(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)

            result = self._run_packager(
                source_root,
                release_root,
                allow_custom_release_root=False,
            )

            self.assertNotEqual(result.returncode, 0)
            # Windows PowerShell may wrap even a parameter name mid-word.
            self.assertIn("AllowCustomReleaseRoot", "".join((result.stderr + result.stdout).split()))
            self.assertFalse(release_root.exists())

    def test_runtime_and_ui_integrity_smokes_reject_checksum_path_aliases(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            temporary_root = pathlib.Path(temporary_directory)
            source_root = temporary_root / "Gateway-source"
            release_root = temporary_root / "release" / "Gateway"
            source_root.mkdir(parents=True)
            self._write_fixture(source_root)
            packaged = self._run_packager(source_root, release_root)
            self.assertEqual(packaged.returncode, 0, msg=packaged.stderr)
            destination = release_root / "contract-v1"

            runtime_valid = self._run_smoke(RUNTIME_SMOKE, destination, "-IntegrityOnly")
            ui_valid = self._run_smoke(UI_SMOKE, destination)
            self.assertEqual(runtime_valid.returncode, 0, msg=runtime_valid.stderr)
            self.assertEqual(ui_valid.returncode, 0, msg=ui_valid.stderr)

            checksums = destination / "checksums.sha256"
            checksum_text = checksums.read_text(encoding="ascii")
            checksums.write_text(
                checksum_text.replace("  gateway.exe", "  alias-gateway.exe"),
                encoding="ascii",
                newline="",
            )

            runtime_tampered = self._run_smoke(RUNTIME_SMOKE, destination, "-IntegrityOnly")
            ui_tampered = self._run_smoke(UI_SMOKE, destination)
            self.assertNotEqual(runtime_tampered.returncode, 0)
            self.assertNotEqual(ui_tampered.returncode, 0)
            self.assertIn("missing packaged file", runtime_tampered.stderr + runtime_tampered.stdout)
            self.assertIn("missing packaged file", ui_tampered.stderr + ui_tampered.stdout)


if __name__ == "__main__":
    unittest.main()
