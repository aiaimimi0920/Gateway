import hashlib
import os
import pathlib
import subprocess
import tempfile
import time
import unittest
import zipfile

try:
    from .gateway_release_publication_fixture import ReleasePublicationFixture
except ImportError:
    from gateway_release_publication_fixture import ReleasePublicationFixture


class GatewayReleasePublicationConcurrencyTests(ReleasePublicationFixture, unittest.TestCase):
    def test_ready_barrier_serializes_three_concurrent_publishers(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            cases = [
                self._windows_case(root / "windows", "concurrent-windows"),
                self._docker_case(
                    root / "docker", "concurrent-docker", payload_size=2 * 1024 * 1024
                ),
            ]
            for case in cases:
                with self.subTest(publisher=case["name"]):
                    gate = root / f"{case['name']}.gate"
                    processes: list[subprocess.Popen[str]] = []
                    try:
                        for index in range(3):
                            ready = root / f"{case['name']}.ready-{index}"
                            process = self._start_after_barrier(
                                case["script"],
                                case["args"],
                                gate,
                                ready,
                                case["cwd"],
                                case["env"],
                            )
                            processes.append(process)
                        for index in range(3):
                            self._wait_for_path(
                                root / f"{case['name']}.ready-{index}"
                            )
                        gate.write_text("go\n", encoding="ascii")
                        results = [self._communicate(process) for process in processes]
                    finally:
                        self._terminate(processes)

                    successes = [result for result in results if result[0] == 0]
                    failures = [result for result in results if result[0] != 0]
                    self.assertEqual(1, len(successes), msg=repr(results))
                    self.assertEqual(2, len(failures), msg=repr(results))
                    for failure in failures:
                        self.assertIn(
                            "immutable", "".join((failure[1] + failure[2]).split())
                        )

                    zip_path = case["zip"]
                    checksum_path = case["checksum"]
                    assert isinstance(zip_path, pathlib.Path)
                    assert isinstance(checksum_path, pathlib.Path)
                    with zipfile.ZipFile(zip_path) as archive:
                        self.assertIsNone(archive.testzip())
                    digest = hashlib.sha256(zip_path.read_bytes()).hexdigest()
                    with checksum_path.open("r", encoding="ascii", newline="") as file:
                        checksum_record = file.read()
                    if case["name"] == "windows":
                        self.assertEqual(
                            f"{digest}  {zip_path.name}{os.linesep}", checksum_record
                        )
                    else:
                        self.assertEqual(f"{digest} *{zip_path.name}\n", checksum_record)
                    self.assertFalse(case["journal"].exists())
                    self._assert_no_private_residue(
                        zip_path,
                        checksum_path,
                        case["staging"],
                        case["journal"],
                    )

    def test_same_version_different_output_directories_keep_docker_staging_isolated(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            fixture_root = root / "Gateway"
            staging_root = root / "temp"
            output_a = root / "packages-a"
            output_b = root / "packages-b"
            staging_root.mkdir()
            output_a.mkdir()
            output_b.mkdir()
            script = self._create_docker_fixture(
                fixture_root, payload_size=32 * 1024 * 1024, staging_barrier=True
            )
            environment = os.environ.copy()
            environment["TEMP"] = str(staging_root)
            environment["TMP"] = str(staging_root)
            stage_ready_a, stage_gate_a = root / "stage-a.ready", root / "stage-a.gate"
            stage_ready_b, stage_gate_b = root / "stage-b.ready", root / "stage-b.gate"
            environment_a = dict(
                environment, GATEWAY_TEST_STAGE_READY=str(stage_ready_a),
                GATEWAY_TEST_STAGE_GATE=str(stage_gate_a),
            )
            environment_b = dict(
                environment, GATEWAY_TEST_STAGE_READY=str(stage_ready_b),
                GATEWAY_TEST_STAGE_GATE=str(stage_gate_b),
            )
            version_id = "shared-version"
            gate_a = root / "publisher-a.gate"
            ready_a = root / "publisher-a.ready"
            gate_b = root / "publisher-b.gate"
            ready_b = root / "publisher-b.ready"
            processes: list[subprocess.Popen[str]] = []
            try:
                publisher_a = self._start_after_barrier(
                    script,
                    [("-VersionId", version_id), ("-OutputDir", output_a)],
                    gate_a,
                    ready_a,
                    fixture_root,
                    environment_a,
                )
                processes.append(publisher_a)
                self._wait_for_path(ready_a)
                gate_a.write_text("go\n", encoding="ascii")
                self._wait_for_path(stage_ready_a)
                first_staging = self._wait_for_glob(
                    staging_root, f"gateway-docker-deploy-{version_id}-*"
                )

                holder_ready = root / "staging-holder.ready"
                holder_release = root / "staging-holder.release"
                holder = self._start_exclusive_file_holder(
                    first_staging / "contract-hold.lock",
                    holder_ready,
                    holder_release,
                )
                processes.append(holder)
                self._wait_for_path(holder_ready)

                publisher_b = self._start_after_barrier(
                    script,
                    [("-VersionId", version_id), ("-OutputDir", output_b)],
                    gate_b,
                    ready_b,
                    fixture_root,
                    environment_b,
                )
                processes.append(publisher_b)
                self._wait_for_path(ready_b)
                gate_b.write_text("go\n", encoding="ascii")
                self._wait_for_path(stage_ready_b)
                self.assertTrue(first_staging.is_dir(), "publisher B removed A staging")
                self.assertIsNone(holder.poll(), "staging holder exited prematurely")

                second_staging = None
                deadline = time.monotonic() + 30
                while time.monotonic() < deadline and publisher_b.poll() is None:
                    candidates = [
                        path
                        for path in staging_root.glob(
                            f"gateway-docker-deploy-{version_id}-*"
                        )
                        if path != first_staging
                    ]
                    if candidates:
                        second_staging = candidates[0]
                        break
                    time.sleep(0.005)

                holder_release.write_text("release\n", encoding="ascii")
                holder_result = self._communicate(holder, timeout=15)
                self.assertEqual(0, holder_result[0], msg=repr(holder_result))
                stage_gate_a.write_text("go\n", encoding="ascii")
                stage_gate_b.write_text("go\n", encoding="ascii")
                result_a = self._communicate(publisher_a, timeout=120)
                result_b = self._communicate(publisher_b, timeout=120)
            finally:
                self._terminate(processes)

            self.assertIsNotNone(
                second_staging,
                msg=f"second publisher never created isolated staging: {result_b!r}",
            )
            self.assertEqual(0, result_a[0], msg=repr(result_a))
            self.assertEqual(0, result_b[0], msg=repr(result_b))
            for output_dir in (output_a, output_b):
                zip_path = output_dir / f"Gateway-{version_id}-docker-deploy.zip"
                checksum_path = pathlib.Path(str(zip_path) + ".sha256")
                with zipfile.ZipFile(zip_path) as archive:
                    self.assertIsNone(archive.testzip())
                digest = hashlib.sha256(zip_path.read_bytes()).hexdigest()
                self.assertEqual(
                    f"{digest} *{zip_path.name}\n",
                    checksum_path.read_text(encoding="utf-8"),
                )
            self.assertEqual(
                [], list(staging_root.glob(f"gateway-docker-deploy-{version_id}-*"))
            )

    def test_active_owner_times_out_without_touching_journal_then_crash_recovers(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            cases = [
                self._windows_case(root / "windows", "active-windows"),
                self._docker_case(root / "docker", "active-docker"),
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
                    journal_text, _ = self._journal_record(case, "a" * 64)
                    journal_path.write_text(journal_text, encoding="utf-8", newline="")
                    original_journal = journal_path.read_bytes()
                    holder_ready = root / f"{case['name']}.holder-ready"
                    holder_release = root / f"{case['name']}.holder-release"
                    holder = self._start_lock_holder(
                        lock_path, zip_path.name, holder_ready, holder_release
                    )
                    processes = [holder]
                    try:
                        self._wait_for_path(holder_ready)
                        environment = dict(case["env"])
                        environment[
                            "GATEWAY_RELEASE_PUBLICATION_LOCK_TIMEOUT_SECONDS"
                        ] = "1"
                        timeout_result = self._run_script(
                            case["script"],
                            case["args"],
                            case["cwd"],
                            environment,
                            timeout=15,
                        )
                        self.assertNotEqual(0, timeout_result[0], msg=repr(timeout_result))
                        combined = timeout_result[1] + timeout_result[2]
                        self.assertIn("Timed out waiting", combined)
                        self.assertIn("active-test-owner", combined)
                        self.assertEqual(original_journal, journal_path.read_bytes())
                        self.assertFalse(zip_path.exists())
                        self.assertFalse(checksum_path.exists())
                        self.assertTrue(lock_path.is_file())

                        observer_ready = root / f"{case['name']}.observer-ready"
                        observer_release = root / f"{case['name']}.observer-release"
                        observer = self._start_mutex_observer(
                            lock_path, observer_ready, observer_release
                        )
                        processes.append(observer)
                        self._wait_for_path(observer_ready)
                        holder.kill()
                        holder_result = self._communicate(holder, timeout=15)
                        self.assertNotEqual(0, holder_result[0])
                        recovery_result = self._run_script(
                            case["script"], case["args"], case["cwd"], case["env"]
                        )
                        self.assertEqual(0, recovery_result[0], msg=repr(recovery_result))
                        observer_release.write_text("release\n", encoding="ascii")
                        observer_result = self._communicate(observer, timeout=15)
                        self.assertEqual(0, observer_result[0], msg=repr(observer_result))
                    finally:
                        self._terminate(processes)

                    self.assertTrue(zip_path.is_file())
                    self.assertTrue(checksum_path.is_file())
                    self.assertFalse(journal_path.exists())
                    self._assert_no_private_residue(
                        zip_path,
                        checksum_path,
                        case["staging"],
                        journal_path,
                    )

    def test_real_publisher_crash_staging_is_cleaned_by_same_version_successor(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            cases = [
                self._windows_case(
                    root / "windows", "stale-windows", payload_size=16 * 1024 * 1024
                ),
                self._docker_case(
                    root / "docker", "stale-docker", payload_size=16 * 1024 * 1024
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
                    gate = root / f"{case['name']}.crash-gate"
                    ready = root / f"{case['name']}.crash-ready"
                    process = self._start_after_barrier(
                        case["script"],
                        case["args"],
                        gate,
                        ready,
                        case["cwd"],
                        case["env"],
                    )
                    processes = [process]
                    try:
                        self._wait_for_path(ready)
                        gate.write_text("go\n", encoding="ascii")
                        if case["name"] == "windows":
                            staging_parent = zip_path.parent
                            staging_pattern = f".gateway-compress-{case['version']}-*"
                        else:
                            staging_parent = case["staging"]
                            staging_pattern = f"gateway-docker-deploy-{case['version']}-*"
                        stale_staging = self._wait_for_glob(
                            staging_parent, staging_pattern
                        )
                        process.kill()
                        crash_result = self._communicate(process, timeout=15)
                        self.assertNotEqual(0, crash_result[0])
                    finally:
                        self._terminate(processes)

                    self.assertTrue(stale_staging.is_dir())
                    self.assertFalse(zip_path.exists())
                    self.assertFalse(checksum_path.exists())
                    self.assertFalse(journal_path.exists())
                    recovery_result = self._run_script(
                        case["script"], case["args"], case["cwd"], case["env"]
                    )
                    self.assertEqual(0, recovery_result[0], msg=repr(recovery_result))
                    self.assertFalse(stale_staging.exists())
                    self._assert_no_private_residue(
                        zip_path,
                        checksum_path,
                        case["staging"],
                        journal_path,
                    )


if __name__ == "__main__":
    unittest.main()
