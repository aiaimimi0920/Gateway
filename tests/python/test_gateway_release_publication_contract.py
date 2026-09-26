import hashlib
import json
import os
import pathlib
import subprocess
import tempfile
import unittest
import zipfile

try:
    from .gateway_release_publication_fixture import ReleasePublicationFixture, GATEWAY_ROOT, POWERSHELL, POWERSHELL_51
except ImportError:
    from gateway_release_publication_fixture import ReleasePublicationFixture, GATEWAY_ROOT, POWERSHELL, POWERSHELL_51


class GatewayReleasePublicationContractTests(ReleasePublicationFixture, unittest.TestCase):
    def test_publication_contract_runs_on_windows_powershell_51_when_available(self):
        if os.name == "nt":
            self.assertIsNotNone(POWERSHELL_51)
            result = subprocess.run(
                [
                    POWERSHELL,
                    "-NoLogo",
                    "-NoProfile",
                    "-NonInteractive",
                    "-Command",
                    "[pscustomobject]@{edition=$PSVersionTable.PSEdition;"
                    "major=$PSVersionTable.PSVersion.Major;"
                    "minor=$PSVersionTable.PSVersion.Minor}|ConvertTo-Json -Compress",
                ],
                text=True,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
                timeout=15,
                check=False,
            )
            self.assertEqual(0, result.returncode, msg=result.stdout + result.stderr)
            version = json.loads(result.stdout)
            self.assertEqual("Desktop", version["edition"])
            self.assertEqual(5, version["major"])
            self.assertEqual(1, version["minor"])

    def test_publishers_use_owner_state_locks_and_never_roll_back_final_paths(self):
        lock_source = (GATEWAY_ROOT / "tools/release-publication/lock.ps1").read_text(
            encoding="utf-8"
        )
        contracts = {
            "tools/compress-gateway-release.ps1": (
                "Remove-OwnedCompressionPublication",
                "$zipPath",
                "$checksumPath",
                "Complete-InterruptedCompressionPublication",
                "Write-EncodedFileNew",
                "Release ZIP/checksum final publication verification failed.",
            ),
            "tools/export-gateway-docker-deploy-bundle.ps1": (
                "Remove-OwnedDockerPublication",
                "$zipPath",
                "$hashPath",
                "Complete-InterruptedPublication",
                "Write-Utf8NoBomNew",
                "Docker deploy bundle final publication verification failed.",
            ),
        }
        for relative_path, contract in contracts.items():
            with self.subTest(script=relative_path):
                (
                    rollback_function,
                    zip_marker,
                    checksum_marker,
                    recovery_marker,
                    journal_writer,
                    final_verify_marker,
                ) = contract
                script = (GATEWAY_ROOT / relative_path).read_text(encoding="utf-8")
                self.assertIn(
                    '. (Join-Path $PSScriptRoot "release-publication/lock.ps1")', script
                )
                script += "\n" + lock_source
                self.assertIn("function Get-PublicationMutexName", script)
                self.assertIn('return "Global\\GatewayReleasePublication-$hex"', script)
                self.assertIn("[System.Threading.Mutex]::new", script)
                self.assertIn("$mutex.WaitOne", script)
                self.assertIn("[System.IO.FileMode]::Create", script)
                self.assertIn("[System.IO.FileShare]::Read", script)
                self.assertIn('state = "active"', script)
                self.assertIn('state = "releasing"', script)
                self.assertIn("ownerToken", script)
                self.assertIn("processId", script)
                self.assertIn("$Lock.Mutex.ReleaseMutex()", script)
                self.assertIn('$Path + ".write-"', script)
                self.assertIn("[System.IO.File]::Move($temporaryPath, $Path)", script)
                self.assertNotIn(rollback_function, script)
                self.assertNotIn(f"Remove-Item -LiteralPath {zip_marker}", script)
                self.assertNotIn(f"Remove-Item -LiteralPath {checksum_marker}", script)
                self.assertNotIn(f"[System.IO.File]::Delete({zip_marker})", script)
                self.assertNotIn(f"[System.IO.File]::Delete({checksum_marker})", script)
                self.assertNotIn("[switch]$Force", script)
                main_start = script.rindex("$publicationLock = Enter-PublicationLock")
                main = script[main_start:]
                ordered_markers = (
                    "$publicationLock = Enter-PublicationLock",
                    recovery_marker,
                    "$stagingRoot =",
                    journal_writer,
                    final_verify_marker,
                    "Exit-PublicationLock -Lock $publicationLock",
                )
                positions = [main.index(marker) for marker in ordered_markers]
                self.assertEqual(sorted(positions), positions)

    def test_windows_release_artifacts_are_immutable_with_or_without_force(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            case = self._windows_case(pathlib.Path(temporary_directory), "immutable-v1")
            zip_path = case["zip"]
            checksum_path = case["checksum"]
            assert isinstance(zip_path, pathlib.Path)
            assert isinstance(checksum_path, pathlib.Path)
            original_zip = b"immutable zip\n"
            original_checksum = b"immutable checksum\n"
            zip_path.write_bytes(original_zip)
            checksum_path.write_bytes(original_checksum)

            result = self._run_script(case["script"], case["args"], case["cwd"])
            self.assertNotEqual(0, result[0], msg=repr(result))
            self.assertIn("immutable", "".join((result[1] + result[2]).split()))
            self.assertEqual(original_zip, zip_path.read_bytes())
            self.assertEqual(original_checksum, checksum_path.read_bytes())
            self.assertFalse(case["journal"].exists())
            self._assert_no_private_residue(
                zip_path, checksum_path, journal_path=case["journal"]
            )

            force_args = [*case["args"], ("-Force", None)]
            force_result = self._run_script(
                case["script"], force_args, case["cwd"]
            )
            self.assertNotEqual(0, force_result[0], msg=repr(force_result))
            self.assertIn("Force", force_result[1] + force_result[2])
            self.assertEqual(original_zip, zip_path.read_bytes())
            self.assertEqual(original_checksum, checksum_path.read_bytes())
            self.assertFalse(case["journal"].exists())
            self._assert_no_private_residue(
                zip_path, checksum_path, journal_path=case["journal"]
            )

    def test_docker_bundle_entries_are_portable_exact_and_deterministic(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            case = self._docker_case(
                pathlib.Path(temporary_directory), "portable-docker"
            )
            result = self._run_script(
                case["script"], case["args"], case["cwd"], case["env"]
            )
            self.assertEqual(0, result[0], msg=repr(result))

            zip_path = case["zip"]
            checksum_path = case["checksum"]
            assert isinstance(zip_path, pathlib.Path)
            assert isinstance(checksum_path, pathlib.Path)
            bundle_root = "Gateway-portable-docker-docker-deploy"
            expected_entries = sorted(
                (
                    f"{bundle_root}/{relative_path}"
                    for relative_path in (
                        "LICENSE",
                        "README.md",
                        "README.zh-CN.md",
                        "deploy/.env.example",
                        "deploy/README.md",
                        "deploy/docker-compose.local.yml",
                        "deploy/docker-compose.yml",
                        "deploy/docker-deploy.sh",
                        "deploy/docker-entrypoint.sh",
                        "tools/deploy-gateway-docker.ps1",
                    )
                ),
                key=str.lower,
            )
            with zipfile.ZipFile(zip_path) as archive:
                infos = archive.infolist()
                self.assertIsNone(archive.testzip())
            self.assertEqual(expected_entries, [info.filename for info in infos])
            for info in infos:
                self.assertNotIn("\\", info.filename)
                self.assertFalse(info.is_dir())
                self.assertEqual((1980, 1, 1, 0, 0, 0), info.date_time)

            digest = hashlib.sha256(zip_path.read_bytes()).hexdigest()
            self.assertEqual(
                f"{digest} *{zip_path.name}\n",
                checksum_path.read_text(encoding="utf-8"),
            )
            self._assert_no_private_residue(
                zip_path,
                checksum_path,
                case["staging"],
                case["journal"],
            )


if __name__ == "__main__":
    unittest.main()
