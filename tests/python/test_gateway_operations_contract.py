import hashlib
import json
import os
import pathlib
import secrets
import subprocess
import tempfile
import time
import unittest

from powershell_test_utils import powershell_executable


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
GATEWAY_ROOT = REPO_ROOT
BACKUP_SCRIPT = GATEWAY_ROOT / "tools" / "backup-gateway-state.ps1"
RESTORE_SCRIPT = GATEWAY_ROOT / "tools" / "restore-gateway-state.ps1"
VERIFY_SCRIPT = GATEWAY_ROOT / "tools" / "verify-gateway-recovery.ps1"
OPERATIONS_MANUAL = GATEWAY_ROOT / "docs" / "operations-manual.md"
OPERATIONS_ALERTS = GATEWAY_ROOT / "docs" / "operations-alerts.yaml"


class GatewayOperationsContractTests(unittest.TestCase):
    def _run_script(
        self, script: pathlib.Path, *arguments: str
    ) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                powershell_executable(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(script),
                *arguments,
            ],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def _run_docker(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            ["docker", *arguments],
            cwd=REPO_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_backup_and_restore_local_object_storage_with_checksums(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            source = root / "source-objects"
            backup = root / "backup" / "gateway-state-v1"
            restored = root / "restored-objects"
            (source / "profiles" / "tenant-a").mkdir(parents=True)
            (source / "profiles" / "tenant-a" / "state.json").write_text(
                '{"state":"ready"}\n', encoding="utf-8"
            )
            (source / "artifacts").mkdir()
            (source / "artifacts" / "preview.bin").write_bytes(b"gateway-object\x00v1")

            backup_result = self._run_script(
                BACKUP_SCRIPT,
                "-DestinationPath",
                str(backup),
                "-ObjectStoragePath",
                str(source),
            )
            self.assertEqual(
                backup_result.returncode,
                0,
                msg=f"stdout:\n{backup_result.stdout}\nstderr:\n{backup_result.stderr}",
            )

            manifest_path = backup / "manifest.json"
            checksum_path = backup / "checksums.sha256"
            self.assertTrue(manifest_path.is_file())
            self.assertTrue(checksum_path.is_file())
            manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
            self.assertEqual(manifest["schemaVersion"], 1)
            self.assertEqual(manifest["kind"], "gateway-state-backup")
            self.assertFalse(manifest["components"]["redis"]["included"])
            self.assertFalse(manifest["components"]["postgresql"]["included"])
            self.assertTrue(manifest["components"]["objectStorage"]["included"])
            self.assertEqual(manifest["components"]["objectStorage"]["fileCount"], 2)

            checksum_entries = {}
            for line in checksum_path.read_text(encoding="ascii").splitlines():
                digest, relative = line.split("  ", 1)
                checksum_entries[relative] = digest
            self.assertIn("manifest.json", checksum_entries)
            self.assertIn(
                "object-storage/profiles/tenant-a/state.json", checksum_entries
            )
            for relative, digest in checksum_entries.items():
                payload = backup / pathlib.PurePath(
                    *pathlib.PurePosixPath(relative).parts
                )
                self.assertEqual(hashlib.sha256(payload.read_bytes()).hexdigest(), digest)

            restore_result = self._run_script(
                RESTORE_SCRIPT,
                "-BackupPath",
                str(backup),
                "-ObjectStorageDestinationPath",
                str(restored),
            )
            self.assertEqual(
                restore_result.returncode,
                0,
                msg=f"stdout:\n{restore_result.stdout}\nstderr:\n{restore_result.stderr}",
            )
            self.assertEqual(
                (restored / "profiles" / "tenant-a" / "state.json").read_text(
                    encoding="utf-8"
                ),
                '{"state":"ready"}\n',
            )
            self.assertEqual(
                (restored / "artifacts" / "preview.bin").read_bytes(),
                b"gateway-object\x00v1",
            )

    def test_restore_rejects_checksum_tampering_before_writing(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            source = root / "source"
            backup = root / "backup"
            restored = root / "restored"
            source.mkdir()
            (source / "state.json").write_text("original\n", encoding="utf-8")

            backup_result = self._run_script(
                BACKUP_SCRIPT,
                "-DestinationPath",
                str(backup),
                "-ObjectStoragePath",
                str(source),
            )
            self.assertEqual(backup_result.returncode, 0, msg=backup_result.stderr)
            (backup / "object-storage" / "state.json").write_text(
                "tampered\n", encoding="utf-8"
            )

            restore_result = self._run_script(
                RESTORE_SCRIPT,
                "-BackupPath",
                str(backup),
                "-ObjectStorageDestinationPath",
                str(restored),
            )
            self.assertNotEqual(restore_result.returncode, 0)
            self.assertIn("checksum", (restore_result.stdout + restore_result.stderr).lower())
            self.assertFalse(restored.exists())

    def test_dry_run_is_side_effect_free_and_redacts_connection_secrets(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            root = pathlib.Path(temporary_directory)
            destination = root / "must-not-exist"
            redis_url = "redis://user:contract-secret@127.0.0.1:6399/15"
            postgres_url = "postgresql://user:pg-secret@127.0.0.1:5499/gateway"

            result = self._run_script(
                BACKUP_SCRIPT,
                "-DestinationPath",
                str(destination),
                "-RedisUrl",
                redis_url,
                "-RedisNamespace",
                "gateway-contract:",
                "-PostgresConnection",
                postgres_url,
                "-DryRun",
            )
            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            combined = result.stdout + result.stderr
            self.assertFalse(destination.exists())
            self.assertNotIn("contract-secret", combined)
            self.assertNotIn("pg-secret", combined)
            self.assertNotIn(redis_url, combined)
            self.assertNotIn(postgres_url, combined)

    def test_redis_contract_is_namespace_scoped_and_never_flushes_a_database(self):
        for script in (BACKUP_SCRIPT, RESTORE_SCRIPT, VERIFY_SCRIPT):
            self.assertTrue(script.is_file(), msg=f"missing operations script: {script}")
        backup_text = BACKUP_SCRIPT.read_text(encoding="utf-8")
        restore_text = RESTORE_SCRIPT.read_text(encoding="utf-8")
        verify_text = VERIFY_SCRIPT.read_text(encoding="utf-8")
        combined = "\n".join((backup_text, restore_text, verify_text))

        for required in (
            "RedisUrl",
            "RedisContainer",
            "RedisCliPath",
            "RedisNamespace",
            "RedisDestinationNamespace",
            "SCAN",
            "DUMP",
            "PTTL",
            "RESTORE",
            "DryRun",
        ):
            self.assertIn(required, combined)
        self.assertNotRegex(combined.upper(), r"\bFLUSH(?:ALL|DB)\b")
        self.assertIn("AllowSameRedisNamespace", restore_text)
        self.assertIn("OverwriteRedisKeys", restore_text)
        self.assertIn("PostgresContainer", backup_text)
        self.assertIn("PostgresContainer", restore_text)
        self.assertIn("docker run", verify_text.lower())
        self.assertIn("--network none", verify_text.lower())
        self.assertIn("gateway-recovery-", verify_text)
        self.assertIn("finally", verify_text.lower())
        self.assertNotRegex(verify_text, r'"--save",\s*""')

    def test_operations_manual_and_alert_config_match_runtime_surfaces(self):
        self.assertTrue(OPERATIONS_MANUAL.is_file())
        self.assertTrue(OPERATIONS_ALERTS.is_file())
        manual = OPERATIONS_MANUAL.read_text(encoding="utf-8")
        alerts = OPERATIONS_ALERTS.read_text(encoding="utf-8")
        for endpoint in (
            "/healthz",
            "/readyz",
            "/metrics",
            "/v1/internal/gateway/readiness",
            "/v1/internal/gateway/runtime/drain",
            "/v1/internal/gateway/splitter/status",
            "/v1/internal/gateway/splitter/reload",
        ):
            self.assertIn(endpoint, manual)
        for command in (
            "backup-gateway-state.ps1",
            "restore-gateway-state.ps1",
            "verify-gateway-recovery.ps1",
        ):
            self.assertIn(command, manual)
        for operational_topic in (
            "Redis",
            "PostgreSQL",
            "object storage",
            "rollback",
            "checksum",
            "incident",
        ):
            self.assertIn(operational_topic.lower(), manual.lower())
        for metric in (
            "gateway_request_errors_total",
            "gateway_request_in_flight",
            "gateway_request_duration_ms_sum",
            "gateway_provider_errors_total",
        ):
            self.assertIn(metric, alerts)
        self.assertNotRegex(alerts, r"(?i)(password|secret|token)\s*:\s*[^\s#]+")

    @unittest.skipUnless(
        os.environ.get("GATEWAY_RUN_RECOVERY_DOCKER_TESTS") == "1",
        "set GATEWAY_RUN_RECOVERY_DOCKER_TESTS=1 for the isolated Docker recovery test",
    )
    def test_isolated_docker_recovery_verifier(self):
        with tempfile.TemporaryDirectory() as temporary_directory:
            evidence = pathlib.Path(temporary_directory) / "recovery-evidence.json"
            result = self._run_script(
                VERIFY_SCRIPT,
                "-EvidencePath",
                str(evidence),
            )
            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            payload = json.loads(evidence.read_text(encoding="utf-8"))
            self.assertEqual(payload["kind"], "gateway-recovery-verification")
            self.assertEqual(payload["status"], "pass")
            self.assertEqual(payload["redis"]["network"], "none")
            self.assertEqual(payload["redis"]["publishedPorts"], 0)
            self.assertEqual(payload["redis"]["restoredKeyCount"], 3)
            self.assertEqual(payload["objectStorage"]["restoredFileCount"], 2)

    @unittest.skipUnless(
        os.environ.get("GATEWAY_RUN_RECOVERY_DOCKER_TESTS") == "1",
        "set GATEWAY_RUN_RECOVERY_DOCKER_TESTS=1 for PostgreSQL recovery",
    )
    def test_isolated_docker_postgresql_backup_restore(self):
        run_id = secrets.token_hex(6)
        container_name = f"gateway-recovery-postgres-{run_id}"
        postgres_user = f"gateway_{run_id}"
        postgres_password = secrets.token_hex(18)
        source_database = f"source_{run_id}"
        restored_database = f"restored_{run_id}"
        container_started = False

        try:
            run_result = self._run_docker(
                "run",
                "--detach",
                "--rm",
                "--name",
                container_name,
                "--network",
                "none",
                "--env",
                f"POSTGRES_USER={postgres_user}",
                "--env",
                f"POSTGRES_PASSWORD={postgres_password}",
                "--env",
                "POSTGRES_DB=postgres",
                "postgres:16-alpine",
            )
            self.assertEqual(
                run_result.returncode,
                0,
                msg=f"stdout:\n{run_result.stdout}\nstderr:\n{run_result.stderr}",
            )
            container_started = True

            deadline = time.monotonic() + 60
            while time.monotonic() < deadline:
                readiness = self._run_docker(
                    "exec",
                    container_name,
                    "sh",
                    "-ec",
                    (
                        'test "$(cat /proc/1/comm)" = postgres; '
                        'exec pg_isready --username "$1" --dbname postgres'
                    ),
                    "gateway-readiness",
                    postgres_user,
                )
                if readiness.returncode == 0:
                    break
                state = self._run_docker(
                    "inspect",
                    "--format={{.State.Status}}",
                    container_name,
                )
                if state.returncode != 0 or state.stdout.strip() != "running":
                    logs = self._run_docker(
                        "logs", "--tail", "100", container_name
                    )
                    self.fail(
                        "disposable PostgreSQL exited before final readiness\n"
                        f"state: {state.stdout}{state.stderr}\n"
                        f"logs:\n{logs.stdout}{logs.stderr}"
                    )
                time.sleep(0.2)
            else:
                logs = self._run_docker("logs", "--tail", "100", container_name)
                self.fail(
                    "disposable PostgreSQL did not reach final readiness\n"
                    f"logs:\n{logs.stdout}{logs.stderr}"
                )

            for database in (source_database, restored_database):
                create_result = self._run_docker(
                    "exec",
                    container_name,
                    "createdb",
                    "--username",
                    postgres_user,
                    database,
                )
                self.assertEqual(
                    create_result.returncode,
                    0,
                    msg=create_result.stderr,
                )

            seed_sql = """
                CREATE TYPE public.gateway_recovery_state AS ENUM ('ready', 'draining');
                CREATE TABLE public.gateway_recovery_items (
                    id bigint GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
                    tenant_id text NOT NULL,
                    state public.gateway_recovery_state NOT NULL,
                    quota integer NOT NULL CHECK (quota >= 0)
                );
                CREATE INDEX gateway_recovery_items_tenant_idx
                    ON public.gateway_recovery_items (tenant_id);
                INSERT INTO public.gateway_recovery_items (tenant_id, state, quota)
                VALUES ('tenant-a', 'ready', 7), ('tenant-b', 'draining', 0);
            """
            seed_result = self._run_docker(
                "exec",
                container_name,
                "psql",
                "--username",
                postgres_user,
                "--dbname",
                source_database,
                "--set=ON_ERROR_STOP=on",
                "--command",
                seed_sql,
            )
            self.assertEqual(seed_result.returncode, 0, msg=seed_result.stderr)

            source_connection = (
                f"postgresql://{postgres_user}:{postgres_password}"
                f"@127.0.0.1/{source_database}"
            )
            restored_connection = (
                f"postgresql://{postgres_user}:{postgres_password}"
                f"@127.0.0.1/{restored_database}"
            )
            with tempfile.TemporaryDirectory() as temporary_directory:
                backup = pathlib.Path(temporary_directory) / "postgresql-backup"
                backup_result = self._run_script(
                    BACKUP_SCRIPT,
                    "-DestinationPath",
                    str(backup),
                    "-PostgresConnection",
                    source_connection,
                    "-PostgresContainer",
                    container_name,
                    "-DockerPath",
                    "docker",
                )
                self.assertEqual(
                    backup_result.returncode,
                    0,
                    msg=(
                        f"stdout:\n{backup_result.stdout}\n"
                        f"stderr:\n{backup_result.stderr}"
                    ),
                )
                manifest = json.loads(
                    (backup / "manifest.json").read_text(encoding="utf-8")
                )
                self.assertTrue(manifest["components"]["postgresql"]["included"])
                self.assertEqual(
                    manifest["components"]["postgresql"]["format"], "plain-sql"
                )

                restore_result = self._run_script(
                    RESTORE_SCRIPT,
                    "-BackupPath",
                    str(backup),
                    "-PostgresConnection",
                    restored_connection,
                    "-PostgresContainer",
                    container_name,
                    "-DockerPath",
                    "docker",
                    "-ConfirmPostgresRestore",
                )
                self.assertEqual(
                    restore_result.returncode,
                    0,
                    msg=(
                        f"stdout:\n{restore_result.stdout}\n"
                        f"stderr:\n{restore_result.stderr}"
                    ),
                )

            restrict_key = "0123456789abcdef" * 4
            for dump_mode in ("--schema-only", "--data-only"):
                source_dump = self._run_docker(
                    "exec",
                    container_name,
                    "pg_dump",
                    f"--dbname={source_connection}",
                    dump_mode,
                    "--no-owner",
                    "--no-privileges",
                    f"--restrict-key={restrict_key}",
                )
                restored_dump = self._run_docker(
                    "exec",
                    container_name,
                    "pg_dump",
                    f"--dbname={restored_connection}",
                    dump_mode,
                    "--no-owner",
                    "--no-privileges",
                    f"--restrict-key={restrict_key}",
                )
                self.assertEqual(source_dump.returncode, 0, msg=source_dump.stderr)
                self.assertEqual(restored_dump.returncode, 0, msg=restored_dump.stderr)
                self.assertEqual(source_dump.stdout, restored_dump.stdout)

            temporary_payloads = self._run_docker(
                "exec",
                container_name,
                "sh",
                "-c",
                "find /tmp -maxdepth 1 -name 'gateway-restore-*.sql' -print -quit",
            )
            self.assertEqual(
                temporary_payloads.returncode,
                0,
                msg=temporary_payloads.stderr,
            )
            self.assertEqual(temporary_payloads.stdout.strip(), "")
        finally:
            if container_started:
                cleanup = self._run_docker("rm", "--force", container_name)
                self.assertEqual(cleanup.returncode, 0, msg=cleanup.stderr)


if __name__ == "__main__":
    unittest.main()



