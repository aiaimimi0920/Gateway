import hashlib
import io
import json
import os
import pathlib
import tempfile
import unittest
import zipfile

try:
    from .gateway_release_publication_fixture import ReleasePublicationFixture
except ImportError:
    from gateway_release_publication_fixture import ReleasePublicationFixture


class GatewayReleasePublicationRecoveryTests(ReleasePublicationFixture, unittest.TestCase):
    def test_dead_owner_lock_recovers_missing_checksum(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            cases = [
                self._windows_case(root / "windows", "recover-windows"),
                self._docker_case(root / "docker", "recover-docker"),
            ]
            for case in cases:
                with self.subTest(publisher=case["name"]):
                    zip_path = case["zip"]
                    checksum_path = case["checksum"]
                    journal_path = case["journal"]
                    lock_path = case["lock"]
                    assert isinstance(zip_path, pathlib.Path)
                    assert isinstance(checksum_path, pathlib.Path)
                    assert isinstance(journal_path, pathlib.Path)
                    assert isinstance(lock_path, pathlib.Path)
                    buffer = io.BytesIO()
                    with zipfile.ZipFile(buffer, "w") as archive:
                        archive.writestr("payload.txt", f"interrupted-{case['name']}\n")
                    zip_bytes = buffer.getvalue()
                    zip_path.write_bytes(zip_bytes)
                    digest = hashlib.sha256(zip_bytes).hexdigest()
                    journal_text, checksum_record = self._journal_record(case, digest)
                    journal_path.write_text(journal_text, encoding="utf-8", newline="")
                    pathlib.Path(str(journal_path) + ".write-stale").write_bytes(
                        b"stale private journal write\n"
                    )
                    pathlib.Path(str(checksum_path) + ".recover-stale").write_bytes(
                        b"stale private checksum recovery\n"
                    )
                    lock_path.write_text(
                        json.dumps(
                            {
                                "schemaVersion": 1,
                                "ownerToken": "dead-owner",
                                "processId": 999999,
                                "machineName": "contract-test",
                                "startedUtc": "2026-07-28T00:00:00Z",
                                "artifactName": zip_path.name,
                                "state": "active",
                            }
                        )
                        + "\n",
                        encoding="utf-8",
                    )

                    result = self._run_script(
                        case["script"], case["args"], case["cwd"], case["env"]
                    )
                    self.assertEqual(0, result[0], msg=repr(result))
                    self.assertEqual(zip_bytes, zip_path.read_bytes())
                    with checksum_path.open("r", encoding="ascii", newline="") as file:
                        self.assertEqual(checksum_record, file.read())
                    self.assertFalse(journal_path.exists())
                    with zipfile.ZipFile(zip_path) as archive:
                        self.assertIsNone(archive.testzip())
                    self._assert_no_private_residue(
                        zip_path,
                        checksum_path,
                        case["staging"],
                        journal_path,
                    )

    def test_mismatched_interrupted_states_fail_closed_without_deleting_artifacts(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            for publisher in ("windows", "docker"):
                for mismatch in ("zip", "checksum", "journal"):
                    with self.subTest(publisher=publisher, mismatch=mismatch):
                        case_root = root / publisher / mismatch
                        version = f"mismatch-{publisher}-{mismatch}"
                        case = (
                            self._windows_case(case_root, version)
                            if publisher == "windows"
                            else self._docker_case(case_root, version)
                        )
                        zip_path = case["zip"]
                        checksum_path = case["checksum"]
                        journal_path = case["journal"]
                        assert isinstance(zip_path, pathlib.Path)
                        assert isinstance(checksum_path, pathlib.Path)
                        assert isinstance(journal_path, pathlib.Path)
                        buffer = io.BytesIO()
                        with zipfile.ZipFile(buffer, "w") as archive:
                            archive.writestr("payload.txt", "expected interrupted zip\n")
                        expected_zip = buffer.getvalue()
                        expected_digest = hashlib.sha256(expected_zip).hexdigest()
                        journal_text, checksum_record = self._journal_record(
                            case, expected_digest
                        )
                        if mismatch == "zip":
                            zip_path.write_bytes(b"competitor zip\n")
                            journal_path.write_text(
                                journal_text, encoding="utf-8", newline=""
                            )
                        elif mismatch == "checksum":
                            zip_path.write_bytes(expected_zip)
                            checksum_path.write_bytes(b"competitor checksum\n")
                            journal_path.write_text(
                                journal_text, encoding="utf-8", newline=""
                            )
                        else:
                            zip_path.write_bytes(expected_zip)
                            checksum_path.write_text(
                                checksum_record, encoding="ascii", newline=""
                            )
                            journal_path.write_bytes(b"{invalid-journal\n")
                        paths = (zip_path, checksum_path, journal_path)
                        original = {
                            path: (path.exists(), path.read_bytes() if path.exists() else None)
                            for path in paths
                        }

                        result = self._run_script(
                            case["script"],
                            case["args"],
                            case["cwd"],
                            case["env"],
                        )
                        self.assertNotEqual(0, result[0], msg=repr(result))
                        expected_error = {
                            "zip": "does not match its journal",
                            "checksum": "checksum does not match its journal",
                            "journal": "journal is invalid",
                        }[mismatch]
                        self.assertIn(
                            "".join(expected_error.split()),
                            "".join((result[1] + result[2]).split()),
                        )
                        for path, (existed, payload) in original.items():
                            self.assertEqual(existed, path.exists())
                            if existed:
                                self.assertEqual(payload, path.read_bytes())
                        self._assert_no_private_residue(
                            zip_path,
                            checksum_path,
                            case["staging"],
                            journal_path,
                        )

    def test_completed_interrupted_publication_removes_only_validated_journal(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            cases = [
                self._windows_case(root / "windows", "complete-windows"),
                self._docker_case(root / "docker", "complete-docker"),
            ]
            for case in cases:
                with self.subTest(publisher=case["name"]):
                    zip_path = case["zip"]
                    checksum_path = case["checksum"]
                    journal_path = case["journal"]
                    assert isinstance(zip_path, pathlib.Path)
                    assert isinstance(checksum_path, pathlib.Path)
                    assert isinstance(journal_path, pathlib.Path)
                    buffer = io.BytesIO()
                    with zipfile.ZipFile(buffer, "w") as archive:
                        archive.writestr("payload.txt", f"complete-{case['name']}\n")
                    zip_bytes = buffer.getvalue()
                    zip_path.write_bytes(zip_bytes)
                    digest = hashlib.sha256(zip_bytes).hexdigest()
                    journal_text, checksum_record = self._journal_record(case, digest)
                    checksum_path.write_text(
                        checksum_record, encoding="ascii", newline=""
                    )
                    journal_path.write_text(journal_text, encoding="utf-8", newline="")
                    original_checksum = checksum_path.read_bytes()

                    result = self._run_script(
                        case["script"], case["args"], case["cwd"], case["env"]
                    )
                    self.assertEqual(0, result[0], msg=repr(result))
                    self.assertEqual(zip_bytes, zip_path.read_bytes())
                    self.assertEqual(original_checksum, checksum_path.read_bytes())
                    self.assertFalse(journal_path.exists())
                    with zipfile.ZipFile(zip_path) as archive:
                        self.assertIsNone(archive.testzip())
                    self._assert_no_private_residue(
                        zip_path,
                        checksum_path,
                        case["staging"],
                        journal_path,
                    )

    @unittest.skipUnless(os.name == "nt", "requires Windows file sharing semantics")
    def test_cleanup_failure_keeps_journal_for_same_version_recovery(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            cases = [
                self._windows_case(
                    root / "windows", "cleanup-windows", payload_size=16 * 1024 * 1024
                ),
                self._docker_case(
                    root / "docker", "cleanup-docker", payload_size=16 * 1024 * 1024
                ),
            ]
            for case in cases:
                with self.subTest(publisher=case["name"]):
                    zip_path = case["zip"]
                    checksum_path = case["checksum"]
                    journal_path = case["journal"]
                    assert isinstance(zip_path, pathlib.Path)
                    assert isinstance(checksum_path, pathlib.Path)
                    assert isinstance(journal_path, pathlib.Path)
                    if case["name"] == "windows":
                        staging_parent = zip_path.parent
                        staging_pattern = f".gateway-compress-{case['version']}-*"
                    else:
                        staging_parent = case["staging"]
                        staging_pattern = (
                            f"gateway-docker-deploy-{case['version']}-*"
                        )
                    assert isinstance(staging_parent, pathlib.Path)

                    gate = root / f"{case['name']}.cleanup-gate"
                    publisher_ready = root / f"{case['name']}.cleanup-ready"
                    holder_ready = root / f"{case['name']}.holder-ready"
                    holder_release = root / f"{case['name']}.holder-release"
                    holder = self._start_staging_file_holder(
                        staging_parent,
                        staging_pattern,
                        holder_ready,
                        holder_release,
                    )
                    publisher = self._start_after_barrier(
                        case["script"],
                        case["args"],
                        gate,
                        publisher_ready,
                        case["cwd"],
                        case["env"],
                    )
                    processes = [holder, publisher]
                    try:
                        self._wait_for_path(publisher_ready)
                        gate.write_text("go\n", encoding="ascii")
                        self._wait_for_path(holder_ready)
                        staging_root = pathlib.Path(
                            holder_ready.read_text(encoding="utf-8")
                        )
                        first_result = self._communicate(publisher, timeout=120)
                        first_zip = zip_path.read_bytes()
                        first_checksum = checksum_path.read_bytes()
                        journal_survived = journal_path.is_file()
                        staging_survived = staging_root.is_dir()
                    finally:
                        holder_release.write_text("release\n", encoding="ascii")
                        holder_result = self._communicate(holder, timeout=15)
                        self._terminate(processes)

                    self.assertNotEqual(0, first_result[0], msg=repr(first_result))
                    self.assertEqual(0, holder_result[0], msg=repr(holder_result))
                    self.assertTrue(journal_survived, msg=repr(first_result))
                    self.assertTrue(staging_survived, msg=repr(first_result))
                    with zipfile.ZipFile(io.BytesIO(first_zip)) as archive:
                        self.assertIsNone(archive.testzip())
                    digest = hashlib.sha256(first_zip).hexdigest()
                    if case["name"] == "windows":
                        expected_checksum = (
                            f"{digest}  {zip_path.name}{os.linesep}".encode("ascii")
                        )
                    else:
                        expected_checksum = (
                            f"{digest} *{zip_path.name}\n".encode("ascii")
                        )
                    self.assertEqual(expected_checksum, first_checksum)

                    recovery_result = self._run_script(
                        case["script"], case["args"], case["cwd"], case["env"]
                    )
                    self.assertEqual(0, recovery_result[0], msg=repr(recovery_result))
                    self.assertIn("recover", (recovery_result[1] + recovery_result[2]).lower())
                    self.assertEqual(first_zip, zip_path.read_bytes())
                    self.assertEqual(first_checksum, checksum_path.read_bytes())
                    self.assertFalse(staging_root.exists())
                    self.assertFalse(journal_path.exists())
                    self._assert_no_private_residue(
                        zip_path,
                        checksum_path,
                        case["staging"],
                        journal_path,
                    )


if __name__ == "__main__":
    unittest.main()
