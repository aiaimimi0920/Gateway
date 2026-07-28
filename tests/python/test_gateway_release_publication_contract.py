import hashlib
import io
import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import time
import unittest
import zipfile

try:
    from .powershell_test_utils import powershell_executable
except ImportError:
    from powershell_test_utils import powershell_executable


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
POWERSHELL_51 = shutil.which("powershell")
POWERSHELL = POWERSHELL_51 or powershell_executable()


class GatewayReleasePublicationContractTests(unittest.TestCase):
    @staticmethod
    def _ps_quote(value: os.PathLike[str] | str) -> str:
        return "'" + str(value).replace("'", "''") + "'"

    @staticmethod
    def _create_windows_release(release_dir: pathlib.Path, payload_size: int = 0) -> None:
        release_dir.mkdir(parents=True)
        for name in (
            "manifest.json",
            "checksums.sha256",
            "gateway.exe",
            "gateway-ui.exe",
        ):
            (release_dir / name).write_bytes((name + "\n").encode("ascii"))
        if payload_size:
            (release_dir / "publication-payload.bin").write_bytes(
                os.urandom(payload_size)
            )

    @staticmethod
    def _create_docker_fixture(
        fixture_root: pathlib.Path, payload_size: int = 0
    ) -> pathlib.Path:
        tools_dir = fixture_root / "tools"
        deploy_dir = fixture_root / "deploy"
        tools_dir.mkdir(parents=True)
        deploy_dir.mkdir(parents=True)
        source_script = (
            GATEWAY_ROOT / "tools/export-gateway-docker-deploy-bundle.ps1"
        )
        script_path = tools_dir / source_script.name
        script_path.write_bytes(source_script.read_bytes())
        files = {
            "README.md": b"# Gateway\n",
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
        for relative_path, payload in files.items():
            destination = fixture_root / pathlib.PurePosixPath(relative_path)
            destination.parent.mkdir(parents=True, exist_ok=True)
            destination.write_bytes(payload)
        if payload_size:
            (fixture_root / "README.md").write_bytes(os.urandom(payload_size))
        return script_path

    @classmethod
    def _start_after_barrier(
        cls,
        script_path: pathlib.Path,
        arguments: list[tuple[str, os.PathLike[str] | str | None]],
        gate_path: pathlib.Path,
        ready_path: pathlib.Path,
        cwd: pathlib.Path,
        environment: dict[str, str] | None = None,
    ) -> subprocess.Popen[str]:
        invocation = ["&", cls._ps_quote(script_path)]
        for name, value in arguments:
            invocation.append(name)
            if value is not None:
                invocation.append(cls._ps_quote(value))
        command = (
            "$ErrorActionPreference='Stop'; "
            f"[System.IO.File]::WriteAllText({cls._ps_quote(ready_path)},'ready'); "
            f"while(-not (Test-Path -LiteralPath {cls._ps_quote(gate_path)}))"
            "{ Start-Sleep -Milliseconds 10 }; "
            "try { "
            + " ".join(invocation)
            + "; exit 0 } catch { Write-Error $_; exit 1 }"
        )
        return subprocess.Popen(
            [
                POWERSHELL,
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ],
            cwd=cwd,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    @classmethod
    def _run_script(
        cls,
        script_path: pathlib.Path,
        arguments: list[tuple[str, os.PathLike[str] | str | None]],
        cwd: pathlib.Path,
        environment: dict[str, str] | None = None,
        timeout: float = 90,
    ) -> tuple[int, str, str]:
        command = [
            POWERSHELL,
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-ExecutionPolicy",
            "Bypass",
            "-File",
            str(script_path),
        ]
        for name, value in arguments:
            command.append(name)
            if value is not None:
                command.append(str(value))
        result = subprocess.run(
            command,
            cwd=cwd,
            env=environment,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=timeout,
            check=False,
        )
        return result.returncode, result.stdout, result.stderr

    @staticmethod
    def _communicate(
        process: subprocess.Popen[str], timeout: float = 90
    ) -> tuple[int, str, str]:
        try:
            stdout, stderr = process.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            process.kill()
            process.communicate(timeout=10)
            raise
        return process.returncode, stdout, stderr

    @staticmethod
    def _terminate(processes: list[subprocess.Popen[str]]) -> None:
        for process in processes:
            if process.poll() is None:
                process.kill()
        for process in processes:
            try:
                process.communicate(timeout=10)
            except subprocess.TimeoutExpired:
                process.kill()
                process.communicate(timeout=10)

    @staticmethod
    def _wait_for_path(path: pathlib.Path, timeout: float = 15) -> None:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            if path.exists():
                return
            time.sleep(0.01)
        raise AssertionError(f"timed out waiting for path: {path}")

    @staticmethod
    def _wait_for_glob(
        parent: pathlib.Path, pattern: str, timeout: float = 30
    ) -> pathlib.Path:
        deadline = time.monotonic() + timeout
        while time.monotonic() < deadline:
            matches = list(parent.glob(pattern))
            if matches:
                return matches[0]
            time.sleep(0.005)
        raise AssertionError(f"timed out waiting for {pattern} under {parent}")

    @classmethod
    def _mutex_name(cls, lock_path: pathlib.Path) -> str:
        normalized_path = str(lock_path.resolve()).lower()
        prefix = "Global\\" if os.name == "nt" else ""
        return prefix + "GatewayReleasePublication-" + hashlib.sha256(
            normalized_path.encode("utf-8")
        ).hexdigest()

    @classmethod
    def _start_lock_holder(
        cls,
        lock_path: pathlib.Path,
        artifact_name: str,
        ready_path: pathlib.Path,
        release_path: pathlib.Path,
    ) -> subprocess.Popen[str]:
        mutex_name = cls._mutex_name(lock_path)
        command = (
            "$ErrorActionPreference='Stop'; "
            f"$mutex=[System.Threading.Mutex]::new($false,{cls._ps_quote(mutex_name)}); "
            "$mutexHeld=$false; $stream=$null; "
            "try { $mutexHeld=$mutex.WaitOne(); "
            "$stream=[System.IO.File]::Open("
            f"{cls._ps_quote(lock_path)},"
            "[System.IO.FileMode]::Create,"
            "[System.IO.FileAccess]::ReadWrite,"
            "[System.IO.FileShare]::Read); "
            "$record=[ordered]@{schemaVersion=1;ownerToken='active-test-owner';"
            "processId=$PID;machineName=[Environment]::MachineName;"
            "startedUtc=[DateTime]::UtcNow.ToString('o');"
            f"artifactName={cls._ps_quote(artifact_name)};state='active'}}"
            "|ConvertTo-Json -Compress; $record += \"`n\"; "
            "$bytes=[System.Text.UTF8Encoding]::new($false).GetBytes($record); "
            "$stream.SetLength(0); $stream.Write($bytes,0,$bytes.Length); "
            "$stream.Flush($true); "
            f"[System.IO.File]::WriteAllText({cls._ps_quote(ready_path)},'ready'); "
            f"while(-not (Test-Path -LiteralPath {cls._ps_quote(release_path)}))"
            "{ Start-Sleep -Milliseconds 10 } "
            "} finally { if($null -ne $stream){$stream.Dispose()}; "
            "if($mutexHeld){$mutex.ReleaseMutex()}; $mutex.Dispose() }"
        )
        return subprocess.Popen(
            [
                POWERSHELL,
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    @classmethod
    def _start_mutex_observer(
        cls,
        lock_path: pathlib.Path,
        ready_path: pathlib.Path,
        release_path: pathlib.Path,
    ) -> subprocess.Popen[str]:
        command = (
            "$ErrorActionPreference='Stop'; "
            "$mutex=[System.Threading.Mutex]::OpenExisting("
            f"{cls._ps_quote(cls._mutex_name(lock_path))}); "
            "try { "
            f"[System.IO.File]::WriteAllText({cls._ps_quote(ready_path)},'ready'); "
            f"while(-not (Test-Path -LiteralPath {cls._ps_quote(release_path)}))"
            "{ Start-Sleep -Milliseconds 10 } "
            "} finally { $mutex.Dispose() }"
        )
        return subprocess.Popen(
            [
                POWERSHELL,
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    @classmethod
    def _start_exclusive_file_holder(
        cls,
        path: pathlib.Path,
        ready_path: pathlib.Path,
        release_path: pathlib.Path,
    ) -> subprocess.Popen[str]:
        command = (
            "$ErrorActionPreference='Stop'; "
            "$stream=[System.IO.File]::Open("
            f"{cls._ps_quote(path)},"
            "[System.IO.FileMode]::Create,"
            "[System.IO.FileAccess]::ReadWrite,"
            "[System.IO.FileShare]::None); "
            "try { "
            f"[System.IO.File]::WriteAllText({cls._ps_quote(ready_path)},'ready'); "
            f"while(-not (Test-Path -LiteralPath {cls._ps_quote(release_path)}))"
            "{ Start-Sleep -Milliseconds 10 } "
            "} finally { $stream.Dispose() }"
        )
        return subprocess.Popen(
            [
                POWERSHELL,
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    @classmethod
    def _start_staging_file_holder(
        cls,
        staging_parent: pathlib.Path,
        staging_pattern: str,
        ready_path: pathlib.Path,
        release_path: pathlib.Path,
    ) -> subprocess.Popen[str]:
        command = (
            "$ErrorActionPreference='Stop'; "
            "$staging=$null; "
            "while($null -eq $staging){ "
            "$staging=Get-ChildItem -LiteralPath "
            f"{cls._ps_quote(staging_parent)} "
            f"-Directory -Filter {cls._ps_quote(staging_pattern)} "
            "-ErrorAction SilentlyContinue | Select-Object -First 1; "
            "if($null -eq $staging){ Start-Sleep -Milliseconds 1 } }; "
            "$path=Join-Path $staging.FullName 'contract-hold.lock'; "
            "$stream=[System.IO.File]::Open("
            "$path,"
            "[System.IO.FileMode]::Create,"
            "[System.IO.FileAccess]::ReadWrite,"
            "[System.IO.FileShare]::None); "
            "try { "
            "[System.IO.File]::WriteAllText("
            f"{cls._ps_quote(ready_path)},$staging.FullName); "
            f"while(-not (Test-Path -LiteralPath {cls._ps_quote(release_path)}))"
            "{ Start-Sleep -Milliseconds 10 } "
            "} finally { $stream.Dispose() }"
        )
        return subprocess.Popen(
            [
                POWERSHELL,
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-ExecutionPolicy",
                "Bypass",
                "-Command",
                command,
            ],
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    @staticmethod
    def _lock_residue(zip_path: pathlib.Path) -> list[pathlib.Path]:
        return list(zip_path.parent.glob(zip_path.name + ".publishing.lock*"))

    def _assert_no_private_residue(
        self,
        zip_path: pathlib.Path,
        checksum_path: pathlib.Path,
        staging_root: pathlib.Path | None = None,
        journal_path: pathlib.Path | None = None,
    ) -> None:
        self.assertEqual([], self._lock_residue(zip_path))
        self.assertEqual(
            [], list(checksum_path.parent.glob(checksum_path.name + ".recover-*"))
        )
        self.assertEqual(
            [], list(checksum_path.parent.glob(checksum_path.name + ".write-*"))
        )
        if journal_path is not None:
            self.assertEqual(
                [], list(journal_path.parent.glob(journal_path.name + ".write-*"))
            )
        self.assertEqual([], list(zip_path.parent.glob(".gateway-compress-*")))
        if staging_root is not None:
            self.assertEqual([], list(staging_root.glob("gateway-docker-deploy-*")))

    def _windows_case(
        self, root: pathlib.Path, version_id: str, payload_size: int = 0
    ) -> dict[str, object]:
        release_dir = root / "release"
        output_dir = root / "packages"
        self._create_windows_release(release_dir, payload_size)
        output_dir.mkdir()
        zip_path = output_dir / f"Gateway-{version_id}-windows-x64.zip"
        return {
            "name": "windows",
            "version": version_id,
            "script": GATEWAY_ROOT / "tools/compress-gateway-release.ps1",
            "cwd": GATEWAY_ROOT,
            "args": [
                ("-ReleaseDir", release_dir),
                ("-OutputDir", output_dir),
                ("-VersionId", version_id),
            ],
            "zip": zip_path,
            "checksum": pathlib.Path(str(zip_path) + ".sha256"),
            "journal": pathlib.Path(str(zip_path) + ".publishing.json"),
            "lock": pathlib.Path(str(zip_path) + ".publishing.lock"),
            "staging": None,
            "env": os.environ.copy(),
        }

    def _docker_case(
        self, root: pathlib.Path, version_id: str, payload_size: int = 0
    ) -> dict[str, object]:
        fixture_root = root / "Gateway"
        output_dir = root / "packages"
        staging_root = root / "temp"
        script_path = self._create_docker_fixture(fixture_root, payload_size)
        output_dir.mkdir()
        staging_root.mkdir()
        environment = os.environ.copy()
        environment["TEMP"] = str(staging_root)
        environment["TMP"] = str(staging_root)
        zip_path = output_dir / f"Gateway-{version_id}-docker-deploy.zip"
        return {
            "name": "docker",
            "version": version_id,
            "script": script_path,
            "cwd": fixture_root,
            "args": [
                ("-VersionId", version_id),
                ("-OutputDir", output_dir),
            ],
            "zip": zip_path,
            "checksum": pathlib.Path(str(zip_path) + ".sha256"),
            "journal": pathlib.Path(str(zip_path) + ".publishing.json"),
            "lock": pathlib.Path(str(zip_path) + ".publishing.lock"),
            "staging": staging_root,
            "env": environment,
        }

    @staticmethod
    def _journal_record(case: dict[str, object], digest: str) -> tuple[str, str]:
        zip_path = case["zip"]
        assert isinstance(zip_path, pathlib.Path)
        if case["name"] == "windows":
            checksum_record = f"{digest}  {zip_path.name}{os.linesep}"
            journal = {
                "schemaVersion": 1,
                "zipName": zip_path.name,
                "zipSha256": digest,
                "checksumRecord": checksum_record,
            }
        else:
            checksum_record = f"{digest} *{zip_path.name}\n"
            journal = {
                "schemaVersion": 1,
                "zipName": zip_path.name,
                "zipSha256": digest,
                "hashRecord": checksum_record,
            }
        return json.dumps(journal, separators=(",", ":")) + "\n", checksum_record

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
                fixture_root, payload_size=32 * 1024 * 1024
            )
            environment = os.environ.copy()
            environment["TEMP"] = str(staging_root)
            environment["TMP"] = str(staging_root)
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
                    environment,
                )
                processes.append(publisher_a)
                self._wait_for_path(ready_a)
                gate_a.write_text("go\n", encoding="ascii")
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
                    environment,
                )
                processes.append(publisher_b)
                self._wait_for_path(ready_b)
                gate_b.write_text("go\n", encoding="ascii")

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
