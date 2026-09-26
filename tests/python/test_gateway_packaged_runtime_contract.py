import pathlib
import unittest


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
SCRIPT = GATEWAY_ROOT / "tools" / "smoke-gateway-packaged-runtime.ps1"
SCRIPT_PARTS = (
    SCRIPT,
    GATEWAY_ROOT / "tools" / "smoke-gateway-packaged-runtime.integrity.ps1",
    GATEWAY_ROOT / "tools" / "smoke-gateway-packaged-runtime.runtime.ps1",
)


def read_script():
    return "\n".join(path.read_text(encoding="utf-8") for path in SCRIPT_PARTS)


class GatewayPackagedRuntimeContractTests(unittest.TestCase):
    def test_packaged_runtime_smoke_has_isolated_process_and_endpoint_contract(self):
        text = read_script()
        for required in [
            "param(",
            "$ReleaseDir",
            "$RedisUrl",
            "$Port",
            "$IntegrityOnly",
            "gateway.exe",
            "GATEWAY_RUNTIME_ROLE",
            "GATEWAY_MANAGEMENT_TOKEN",
            "/healthz",
            "/readyz",
            "/v1/models",
            "/v1/chat/completions",
            "/v1/messages",
            "/v1/responses",
            "/metrics",
            "/v1/internal/gateway/runtime/drain",
            "x-management-token",
            "x-request-id",
            "Stop-Process",
            "WaitForExit",
            "no leaked",
        ]:
            self.assertIn(required, text)

    def test_packaged_runtime_smoke_does_not_print_secret_values(self):
        text = read_script()
        self.assertIn("temporary-local", text)
        self.assertNotIn("Write-Output $GatewayApiKey", text)
        self.assertNotIn("Write-Output $ManagementToken", text)

    def test_packaged_runtime_verifies_manifest_and_exact_checksum_entries_before_start(self):
        text = read_script()
        for required in [
            "function Read-ChecksumIndex",
            "function Assert-PackagedReleaseIntegrity",
            "sha256",
            "bytes",
            "two spaces",
            "checksums.sha256",
            "manifest.json",
            "before starting",
            "checksumEntries",
            "Set-StrictMode",
            "PSObject.Properties",
        ]:
            self.assertIn(required, text)

    def test_packaged_runtime_retries_refused_connections_without_strict_mode_property_errors(self):
        text = read_script()
        self.assertIn("TransportError", text)
        self.assertIn("PSObject.Properties['Response']", text)
        self.assertIn("Start-Sleep -Milliseconds", text)

    def test_default_packaged_runtime_smoke_owns_disposable_redis_instead_of_reusing_host_default(self):
        text = read_script()
        for required in [
            "$DockerPath",
            "$RedisImage",
            "function Start-TemporaryRedis",
            "function Stop-TemporaryRedis",
            "docker run",
            "--publish",
            '"port", $containerName, "6379/tcp"',
            "redis-cli",
            '"PING"',
            "containerStarted",
            "containerRemoved",
        ]:
            self.assertIn(required, text)

        # An omitted -RedisUrl must never silently attach the smoke run to the
        # operator's shared Redis instance or the conventional host port.
        self.assertNotIn('if ([string]::IsNullOrWhiteSpace($env:GATEWAY_REDIS_URL))', text)
        self.assertNotIn('"redis://127.0.0.1:6379"', text)

    def test_packaged_runtime_smoke_captures_docker_output_via_process_wrapper(self):
        text = read_script()
        for required in [
            "function Invoke-NativeCommandCapture",
            "Start-Process",
            "RedirectStandardOutput",
            "RedirectStandardError",
            "PassThru = $true",
            'WindowStyle = "Hidden"',
            "Invoke-NativeCommandCapture -Command $DockerExecutable",
        ]:
            self.assertIn(required, text)

    def test_packaged_runtime_evidence_uses_paths_relative_to_evidence_file(self):
        text = read_script()
        for required in [
            "function Get-EvidenceRelativePath",
            'pathBase = "evidence-file-directory"',
            "releaseDir = Get-EvidenceRelativePath",
            "stdoutPath = Get-EvidenceRelativePath",
            "stderrPath = Get-EvidenceRelativePath",
            "$logRoot = Join-Path $evidenceParent",
        ]:
            self.assertIn(required, text)

        self.assertNotIn("releaseDir = $releaseDirFull", text)
        self.assertNotIn("stdoutPath = $stdoutPath", text)
        self.assertNotIn("stderrPath = $stderrPath", text)

    def test_packaged_runtime_smoke_redirects_gateway_state_outside_release_dir(self):
        text = read_script()
        for required in [
            "GATEWAY_STATE_DIR",
            '$stateRoot = Join-Path $logRoot "gateway-state"',
            '$env:GATEWAY_STATE_DIR = $stateRoot',
        ]:
            self.assertIn(required, text)

        self.assertNotIn('$env:GATEWAY_STATE_DIR = Join-Path $releaseDirFull', text)

    def test_packaged_runtime_evidence_records_the_observed_process_exit_code(self):
        text = read_script()

        for required in [
            "$process.WaitForExit() | Out-Null",
            "$process.Refresh()",
            "$processExitCode = [int]$process.ExitCode",
            "if ($processExitCode -ne 0)",
            "processExitCode = $processExitCode",
        ]:
            self.assertIn(required, text)


if __name__ == "__main__":
    unittest.main()
