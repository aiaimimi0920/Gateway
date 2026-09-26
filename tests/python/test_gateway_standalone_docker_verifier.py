import json
import os
import pathlib
import shutil
import subprocess
import tempfile
import unittest


ROOT = pathlib.Path(__file__).resolve().parents[2]
POWERSHELL = shutil.which("powershell.exe") if os.name == "nt" else None


@unittest.skipUnless(POWERSHELL, "Requires Windows PowerShell native stderr semantics")
class GatewayDockerVerifierNativeTests(unittest.TestCase):
    def run_verifier(self, *, fail_operation="", source_image=False):
        with tempfile.TemporaryDirectory(prefix="gateway-docker-native-") as temp:
            root = pathlib.Path(temp)
            tools = root / "tools"
            deploy = root / "deploy"
            tools.mkdir()
            deploy.mkdir()
            verifier = tools / "verify-gateway-docker-stack.ps1"
            shutil.copyfile(ROOT / "tools" / verifier.name, verifier)
            (deploy / "docker-compose.yml").write_text("services: {}\n", encoding="utf-8")
            original = b"COMPOSE_PROJECT_NAME=preserve-existing\r\n"
            env_file = deploy / ".env"
            env_file.write_bytes(original)
            (root / "docker.cmd").write_text(
                '@echo off\n'
                'echo %*>>"%FAKE_DOCKER_LOG%"\n'
                'echo docker progress on stderr 1>&2\n'
                'if "%1"=="%FAKE_DOCKER_FAIL_OPERATION%" exit /b 23\n'
                'exit /b 0\n',
                encoding="ascii",
            )
            script = root / "run.ps1"
            script.write_text(
                r"""
$ErrorActionPreference = 'Stop'
if ((Get-Command docker -ErrorAction Stop).Source -ne (Join-Path $PSScriptRoot 'docker.cmd')) {
    throw 'Refusing to invoke a Docker executable outside the fixture.'
}
$global:fixtureEndpoints = [System.Collections.Generic.List[string]]::new()
function global:Invoke-RestMethod {
    param([string]$Uri, [int]$TimeoutSec)
    $global:fixtureEndpoints.Add($Uri)
    return @{ status = 'ok' }
}
$arguments = @{ ImageTag = 'native-stderr-test'; StartupTimeoutSeconds = 0 }
if ($env:FAKE_DOCKER_SOURCE -eq '1') {
    $arguments.SourceImage = 'fixture/source:original'
} else {
    $arguments.BuildImage = $true
}
$failed = $false
$failure = $null
try {
    & (Join-Path $PSScriptRoot 'tools/verify-gateway-docker-stack.ps1') @arguments *> (Join-Path $PSScriptRoot 'outer.log')
} catch {
    $failed = $true
    $failure = $_.Exception.Message
}
$result = @{ failed = $failed; failure = $failure; endpoints = @($global:fixtureEndpoints) }
[IO.File]::WriteAllText((Join-Path $PSScriptRoot 'result.json'), ($result | ConvertTo-Json), [Text.UTF8Encoding]::new($false))
""",
                encoding="utf-8",
            )
            environment = dict(os.environ)
            environment.update(
                PATH=str(root) + os.pathsep + environment.get("PATH", ""),
                FAKE_DOCKER_LOG=str(root / "calls.log"),
                FAKE_DOCKER_FAIL_OPERATION=fail_operation,
                FAKE_DOCKER_SOURCE="1" if source_image else "0",
            )
            completed = subprocess.run(
                [POWERSHELL, "-NoProfile", "-NonInteractive", "-ExecutionPolicy",
                 "Bypass", "-File", str(script)],
                cwd=root,
                env=environment,
                capture_output=True,
                text=True,
                timeout=30,
                check=False,
            )
            self.assertEqual(completed.returncode, 0, completed.stdout + completed.stderr)
            self.assertEqual(env_file.read_bytes(), original)
            result = json.loads((root / "result.json").read_text(encoding="utf-8"))
            calls = (root / "calls.log").read_text().splitlines()
            return result, calls

    def test_successful_build_and_compose_stderr_does_not_fail_verification(self):
        result, calls = self.run_verifier()
        self.assertFalse(result["failed"], result["failure"])
        self.assertEqual(len(result["endpoints"]), 2)
        self.assertTrue(any(call.startswith("build ") for call in calls))
        self.assertTrue(any(call.endswith("up -d") for call in calls))
        self.assertTrue(any(call.endswith("down -v") for call in calls))

    def test_successful_tag_stderr_does_not_fail_verification(self):
        result, calls = self.run_verifier(source_image=True)
        self.assertFalse(result["failed"], result["failure"])
        self.assertEqual(len(result["endpoints"]), 2)
        self.assertTrue(calls[0].startswith("tag fixture/source:original "))

    def test_nonzero_build_exit_still_fails_before_compose(self):
        result, calls = self.run_verifier(fail_operation="build")
        self.assertTrue(result["failed"])
        self.assertIn("docker build failed with exit code 23", result["failure"])
        self.assertEqual(result["endpoints"], [])
        self.assertFalse(any(call.startswith("compose ") for call in calls))

    def test_nonzero_compose_exit_still_fails_and_attempts_cleanup(self):
        result, calls = self.run_verifier(fail_operation="compose")
        self.assertTrue(result["failed"])
        self.assertIn("docker compose failed with exit code 23", result["failure"])
        self.assertEqual(result["endpoints"], [])
        self.assertTrue(any(call.endswith("logs --no-color --tail 200") for call in calls))
        self.assertTrue(any(call.endswith("down -v") for call in calls))
