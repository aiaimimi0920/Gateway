"""Exercise real Windows child exits through the production release runner."""
import json
import pathlib
import shutil
import subprocess
import tempfile
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]
POWERSHELL = shutil.which("powershell.exe")


@unittest.skipUnless(POWERSHELL, "Requires Windows PowerShell process semantics")
class GatewayReleaseNativeExitTests(unittest.TestCase):
    def test_native_exit_codes_and_streams_are_preserved(self):
        with tempfile.TemporaryDirectory(prefix="gateway-release-native-") as directory:
            fixture = pathlib.Path(directory)
            child = fixture / "child.ps1"
            child.write_text(
                "param([int]$Code, [int]$DelayMs)\n"
                "[Console]::Out.WriteLine('fixture stdout')\n"
                "[Console]::Error.WriteLine('fixture stderr')\n"
                "Start-Sleep -Milliseconds $DelayMs\nexit $Code\n",
                encoding="utf-8",
            )
            source = str(ROOT / "tools/build-gateway-release.ps1").replace("'", "''")
            script = fixture / "probe.ps1"
            script.write_text(
                "param([int]$Code, [int]$DelayMs)\n"
                "$ErrorActionPreference = 'Stop'\n"
                "$tokens = $null; $parseErrors = $null\n"
                f"$ast = [System.Management.Automation.Language.Parser]::ParseFile('{source}', [ref]$tokens, [ref]$parseErrors)\n"
                "if ($parseErrors.Count) { throw 'Release source parse failure' }\n"
                "foreach ($name in @('Resolve-NativeExecutable', 'Write-NewCaptureLines', 'Invoke-NativeCommandCapture')) {\n"
                "  $definition = $ast.Find({ param($node) $node -is [System.Management.Automation.Language.FunctionDefinitionAst] -and $node.Name -eq $name }, $true)\n"
                "  if ($null -eq $definition) { throw \"Missing function $name\" }\n"
                "  Invoke-Expression $definition.Extent.Text\n"
                "}\n"
                "$child = Join-Path $PSScriptRoot 'child.ps1'\n"
                "$result = Invoke-NativeCommandCapture -Command 'powershell.exe' -Arguments @('-NoProfile', '-File', ('\"' + $child + '\"'), [string]$Code, [string]$DelayMs) -WorkingDirectory $PSScriptRoot\n"
                "[pscustomobject]@{ ExitCode = $result.ExitCode; Output = @($result.Output | ForEach-Object { [string]$_ }) } | ConvertTo-Json -Depth 3 -Compress\n",
                encoding="utf-8",
            )
            for code, delay in [(0, 0), (7, 0), (19, 450)]:
                with self.subTest(code=code, delay=delay):
                    result = subprocess.run(
                        [POWERSHELL, "-NoProfile", "-ExecutionPolicy", "Bypass",
                         "-File", str(script), str(code), str(delay)],
                        capture_output=True, text=True, encoding="utf-8", timeout=30,
                    )
                    self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                    payload = json.loads(result.stdout.strip().splitlines()[-1])
                    self.assertEqual(payload["ExitCode"], code)
                    self.assertIn("fixture stdout", payload["Output"])
                    self.assertIn("fixture stderr", payload["Output"])


if __name__ == "__main__":
    unittest.main()
