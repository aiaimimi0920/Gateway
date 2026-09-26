import hashlib
import json
import os
import pathlib
import shutil
import subprocess
import time

try:
    from .powershell_test_utils import powershell_executable
except ImportError:
    from powershell_test_utils import powershell_executable


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
POWERSHELL_51 = shutil.which("powershell")
POWERSHELL = POWERSHELL_51 or powershell_executable()


class ReleasePublicationFixture:
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
        fixture_root: pathlib.Path, payload_size: int = 0, staging_barrier: bool = False
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
        shutil.copytree(
            GATEWAY_ROOT / "tools/release-publication",
            tools_dir / "release-publication",
        )
        if staging_barrier:
            # Instrument only the temporary fixture, never the production publisher.
            source = script_path.read_text(encoding="utf-8")
            marker = "    New-Item -ItemType Directory -Path $bundleRoot -Force | Out-Null"
            if source.count(marker) != 1:
                raise AssertionError("publisher staging barrier anchor changed")
            barrier = "\n".join((
                marker,
                "    [IO.File]::WriteAllText($env:GATEWAY_TEST_STAGE_READY, $stagingRoot)",
                "    $testDeadline = [DateTime]::UtcNow.AddSeconds(60)",
                "    while (-not (Test-Path -LiteralPath $env:GATEWAY_TEST_STAGE_GATE)) {",
                "        if ([DateTime]::UtcNow -ge $testDeadline) { throw 'Test staging gate timeout' }",
                "        Start-Sleep -Milliseconds 10",
                "    }",
            ))
            script_path.write_text(source.replace(marker, barrier), encoding="utf-8")
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
