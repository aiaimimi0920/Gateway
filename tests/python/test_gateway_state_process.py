import base64
import json
import pathlib
import shutil
import subprocess
import unittest

from powershell_test_utils import powershell_executable


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
POWERSHELL = shutil.which("powershell") or powershell_executable()
NODE = shutil.which("node.exe") or shutil.which("node")
PROCESS_OWNER = GATEWAY_ROOT / "tools/state-recovery/native-process.ps1"


def ps_quote(value: object) -> str:
    return "'" + str(value).replace("'", "''") + "'"


@unittest.skipUnless(NODE, "Node.js is required for native process transport checks")
class GatewayStateProcessTests(unittest.TestCase):
    def run_probe(self, body: str) -> str:
        script = (
            "$ErrorActionPreference='Stop'; "
            f". {ps_quote(PROCESS_OWNER)}; "
            f"$nodeExe={ps_quote(NODE)}; "
            + body
        )
        encoded = base64.b64encode(script.encode("utf-16-le")).decode("ascii")
        result = subprocess.run(
            [POWERSHELL, "-NoProfile", "-NonInteractive", "-EncodedCommand", encoded],
            cwd=GATEWAY_ROOT,
            text=True,
            capture_output=True,
            timeout=30,
            check=False,
        )
        self.assertEqual(0, result.returncode, msg=result.stdout + result.stderr)
        return result.stdout.strip()

    def test_binary_stdin_has_no_encoding_preamble(self):
        encoded = self.run_probe(
            "[Console]::InputEncoding=[Text.UTF8Encoding]::new($true); "
            "$payload=[byte[]]@(0,127,128,255,0,65); "
            "$result=Invoke-NativeCapture -FilePath $nodeExe "
            "-Arguments @('-e','process.stdin.pipe(process.stdout)') "
            "-Operation 'Binary transport' -InputBytes $payload; "
            "[Convert]::ToBase64String($result.Bytes)"
        )
        self.assertEqual(bytes([0, 127, 128, 255, 0, 65]), base64.b64decode(encoded))

    def test_native_arguments_preserve_spaces_quotes_and_trailing_slashes(self):
        expected = ["two words", 'quote"inside', "C:\\space dir\\"]
        arguments = ",".join(ps_quote(value) for value in expected)
        output = self.run_probe(
            "$result=Invoke-NativeCapture -FilePath $nodeExe "
            "-Arguments @('-e','process.stdout.write(JSON.stringify(process.argv.slice(1)))',"
            f"'--',{arguments}) -Operation 'Argument transport'; $result.Text"
        )
        self.assertEqual(expected, json.loads(output))

    def test_native_failure_does_not_expose_stderr(self):
        message = self.run_probe(
            "try { Invoke-NativeCapture -FilePath $nodeExe "
            "-Arguments @('-e','process.stderr.write(\"private-marker\");process.exit(23)') "
            "-Operation 'Failure transport' | Out-Null; throw 'Expected failure' } "
            "catch { $_.Exception.Message }"
        )
        self.assertIn("exit code 23", message)
        self.assertNotIn("private-marker", message)

    def test_console_input_encoding_is_restored_on_success_and_start_failure(self):
        result = self.run_probe(
            "[Console]::InputEncoding=[Text.UTF8Encoding]::new($true); "
            "$before=[Convert]::ToBase64String([Console]::InputEncoding.GetPreamble()); "
            "$null=Invoke-NativeCapture -FilePath $nodeExe "
            "-Arguments @('-e','process.stdin.resume()') "
            "-Operation 'Encoding success' -InputBytes ([byte[]]@(65)); "
            "$afterSuccess=[Convert]::ToBase64String([Console]::InputEncoding.GetPreamble()); "
            "$missing=Join-Path $env:TEMP ([guid]::NewGuid().ToString('N')+'.exe'); "
            "try { Invoke-NativeCapture -FilePath $missing -Arguments @('unused') "
            "-Operation 'Encoding failure' -InputBytes ([byte[]]@(65)) | Out-Null } catch {} "
            "$afterFailure=[Convert]::ToBase64String([Console]::InputEncoding.GetPreamble()); "
            "@($before,$afterSuccess,$afterFailure)|ConvertTo-Json -Compress"
        )
        before, after_success, after_failure = json.loads(result)
        self.assertEqual(before, after_success)
        self.assertEqual(before, after_failure)


if __name__ == "__main__":
    unittest.main()
