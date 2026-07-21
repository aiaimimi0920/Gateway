import http.server
import json
import pathlib
import shutil
import subprocess
import threading
import time
import unittest


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
SUMMARY_TOOL = GATEWAY_ROOT / "tools" / "get-gateway-operator-summary.ps1"
SLI_SLO_DOC = GATEWAY_ROOT / "docs" / "sli-slo.md"
ALERTS_DOC = GATEWAY_ROOT / "docs" / "operations-alerts.yaml"
OPERATIONS_MANUAL = GATEWAY_ROOT / "docs" / "operations-manual.md"
ALERT_VALIDATOR = GATEWAY_ROOT / "tools" / "validate-gateway-alert-rules.ps1"


def powershell_executable() -> str:
    executable = shutil.which("pwsh") or shutil.which("powershell")
    if not executable:
        raise unittest.SkipTest("PowerShell is not available")
    return executable


class _SummaryHandler(http.server.BaseHTTPRequestHandler):
    response_status = 200
    response_delay_seconds = 0.0
    response_payload = {
        "summary": {
            "schemaVersion": 1,
            "runtime": {"role": "standalone"},
            "readiness": {"ok": True},
        }
    }
    received_headers: dict[str, str] = {}

    def do_GET(self):
        type(self).received_headers = {key.lower(): value for key, value in self.headers.items()}
        if type(self).response_delay_seconds > 0:
            time.sleep(type(self).response_delay_seconds)
        payload = json.dumps(type(self).response_payload).encode("utf-8")
        self.send_response(type(self).response_status)
        self.send_header("Content-Type", "application/json")
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        try:
            self.wfile.write(payload)
        except (BrokenPipeError, ConnectionAbortedError, ConnectionResetError):
            return

    def log_message(self, _format, *_args):
        return


class GatewayOperatorSummaryContractTests(unittest.TestCase):
    def setUp(self):
        _SummaryHandler.response_status = 200
        _SummaryHandler.response_delay_seconds = 0.0
        _SummaryHandler.response_payload = {
            "summary": {
                "schemaVersion": 1,
                "generatedAt": "2026-07-18T12:00:00Z",
                "build": {
                    "name": "neuro-gateway",
                    "version": "0.1.0",
                    "target": {"os": "windows", "arch": "x86_64"},
                    "debugAssertions": False,
                },
                "runtime": {"role": "standalone", "processId": 42, "port": 4200},
                "lifecycle": {
                    "state": "serving",
                    "draining": False,
                    "activeRequests": 0,
                    "drainStartedAt": None,
                    "drainReason": None,
                    "shutdownRequested": False,
                    "shutdownRequestedAt": None,
                    "shutdownReason": None,
                },
                "readiness": {
                    "ok": True,
                    "dependencies": {
                        "redis": {"configured": True, "required": True, "ready": True, "timedOut": False},
                        "postgresql": {"configured": False, "required": False, "ready": True, "timedOut": False},
                        "objectStorage": {
                            "configured": True,
                            "required": True,
                            "ready": True,
                            "timedOut": False,
                            "driver": "local",
                        },
                    },
                    "configuration": {"apiKeySecret": True, "publicBaseUrl": True},
                },
                "routing": {
                    "configured": True,
                    "providerCount": 1,
                    "routeCount": 1,
                    "publishedModelCount": 1,
                },
                "credentialCache": {"entryCount": 0},
                "requestMetrics": {
                    "requestsTotal": 1,
                    "requestErrorsTotal": 0,
                    "requestDrainRejectionsTotal": 0,
                    "requestInFlight": 0,
                    "requestDurationMsCount": 1,
                    "requestDurationMsSum": 5,
                    "rateLimitChecksTotal": 0,
                    "rateLimitRejectionsTotal": 0,
                    "rateLimitStoreFailuresTotal": 0,
                },
                "providerStats": {
                    "activeProviders": 1,
                    "coolingProviders": 0,
                    "disabledProviders": 0,
                },
            }
        }
        _SummaryHandler.received_headers = {}
        self.server = http.server.ThreadingHTTPServer(("127.0.0.1", 0), _SummaryHandler)
        self.thread = threading.Thread(target=self.server.serve_forever, daemon=True)
        self.thread.start()

    def tearDown(self):
        self.server.shutdown()
        self.server.server_close()
        self.thread.join(timeout=5)

    @property
    def base_url(self) -> str:
        return f"http://127.0.0.1:{self.server.server_port}"

    def run_tool(self, *arguments: str) -> subprocess.CompletedProcess[str]:
        return subprocess.run(
            [
                powershell_executable(),
                "-NoLogo",
                "-NoProfile",
                "-NonInteractive",
                "-File",
                str(SUMMARY_TOOL),
                *arguments,
            ],
            cwd=GATEWAY_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )

    @staticmethod
    def normalized_output(result: subprocess.CompletedProcess[str]) -> str:
        return (result.stdout + result.stderr).replace("\r", "").replace("\n", "")

    def test_client_sends_management_header_and_emits_json_without_token(self):
        token = "operator-client-contract-secret"
        result = self.run_tool(
            "-BaseUrl",
            self.base_url,
            "-ManagementToken",
            token,
            "-AsJson",
        )

        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["summary"]["runtime"]["role"], "standalone")
        self.assertEqual(_SummaryHandler.received_headers["x-management-token"], token)
        self.assertNotIn(token, result.stdout + result.stderr)

    def test_client_supports_bearer_and_fails_stably_on_non_success(self):
        token = "operator-bearer-contract-secret"
        _SummaryHandler.response_status = 503
        result = self.run_tool(
            "-BaseUrl",
            self.base_url,
            "-ManagementToken",
            token,
            "-AuthenticationMode",
            "Bearer",
            "-AsJson",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertEqual(_SummaryHandler.received_headers["authorization"], f"Bearer {token}")
        combined = self.normalized_output(result)
        self.assertIn("Gateway operator summary request failed with HTTP 503", combined)
        self.assertNotIn(token, combined)

    def test_client_fails_with_stable_transport_timeout(self):
        token = "operator-timeout-contract-secret"
        _SummaryHandler.response_delay_seconds = 2.0
        started = time.monotonic()
        result = self.run_tool(
            "-BaseUrl",
            self.base_url,
            "-ManagementToken",
            token,
            "-TimeoutSec",
            "1",
            "-AsJson",
        )

        self.assertNotEqual(result.returncode, 0)
        self.assertLess(time.monotonic() - started, 5.0)
        combined = self.normalized_output(result)
        self.assertIn("Gateway operator summary transport-timeout after 1 seconds", combined)
        self.assertNotIn(token, combined)

    def test_client_rejects_missing_required_schema_in_both_output_modes(self):
        token = "operator-schema-contract-secret"
        del _SummaryHandler.response_payload["summary"]["build"]

        for output_arguments in (("-AsJson",), ()): 
            with self.subTest(output_arguments=output_arguments):
                result = self.run_tool(
                    "-BaseUrl",
                    self.base_url,
                    "-ManagementToken",
                    token,
                    *output_arguments,
                )
                self.assertNotEqual(result.returncode, 0)
                combined = self.normalized_output(result)
                self.assertIn(
                    "Gateway operator summary schema validation failed: missing summary.build",
                    combined,
                )
                self.assertNotIn("PropertyNotFound", combined)
                self.assertNotIn(token, combined)

    def test_powershell_client_parses_and_targets_the_owned_endpoint(self):
        self.assertTrue(SUMMARY_TOOL.is_file())
        command = (
            "$errors=$null; "
            f"[System.Management.Automation.Language.Parser]::ParseFile('{SUMMARY_TOOL}', "
            "[ref]$null, [ref]$errors) | Out-Null; "
            "if($errors.Count -gt 0){$errors | ForEach-Object {$_.Message}; exit 1}"
        )
        result = subprocess.run(
            [powershell_executable(), "-NoLogo", "-NoProfile", "-Command", command],
            cwd=GATEWAY_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            timeout=30,
            check=False,
        )
        self.assertEqual(result.returncode, 0, msg=result.stdout + result.stderr)
        script_text = SUMMARY_TOOL.read_text(encoding="utf-8")
        self.assertIn("/v1/internal/gateway/operations/summary", script_text)
        self.assertIn("x-management-token", script_text)
        self.assertIn("Bearer", script_text)
        self.assertIn("ConvertFrom-Json", script_text)
        self.assertIn("DateKind", script_text)
        self.assertIn('"String"', script_text)

    def test_sli_slo_alerts_and_runbook_reference_real_surfaces(self):
        self.assertTrue(SLI_SLO_DOC.is_file())
        sli_slo = SLI_SLO_DOC.read_text(encoding="utf-8")
        alerts = ALERTS_DOC.read_text(encoding="utf-8")
        manual = OPERATIONS_MANUAL.read_text(encoding="utf-8")
        for surface in (
            "/healthz",
            "/readyz",
            "/metrics",
            "/v1/internal/gateway/operations/summary",
        ):
            self.assertIn(surface, sli_slo)
        for metric in (
            "gateway_requests_total",
            "gateway_request_errors_total",
            "gateway_request_duration_ms_sum",
            "gateway_request_duration_ms_count",
            "gateway_provider_errors_total",
            "gateway_provider_requests_total",
        ):
            self.assertIn(metric, sli_slo)
            self.assertIn(metric, alerts)
        self.assertIn("increase(gateway_request_errors_total[30d])", sli_slo)
        self.assertIn("increase(gateway_provider_requests_total[30d])", sli_slo)
        self.assertNotIn("rate(gateway_request_errors_total[5m])", sli_slo)
        self.assertIn("rate(gateway_request_errors_total[5m])", alerts)
        self.assertIn("rate(gateway_provider_requests_total[10m])", alerts)
        self.assertIn("groups:", alerts)
        self.assertIn("rules:", alerts)
        self.assertIn('probe_success{job="gateway-operator-summary"}', alerts)
        self.assertNotIn("kind: gateway-operations-alerts", alerts)
        self.assertNotIn("clamp_min(rate(gateway_requests_total[5m]), 1)", alerts)
        self.assertNotIn("clamp_min(rate(gateway_request_duration_ms_count[5m]), 1)", alerts)
        self.assertIn('gateway_provider_errors_total{failure_class=""}', alerts)
        self.assertIn('gateway_provider_errors_total{failure_class=""}', sli_slo)
        for dependency in (
            "Redis",
            "PostgreSQL",
            "browser executor",
            "upstream timeout",
            "retry exhaustion",
        ):
            self.assertIn(dependency.lower(), sli_slo.lower())
        self.assertIn("sli-slo.md", manual)
        self.assertIn("operations/summary", manual)

    def test_prometheus_alert_validation_entrypoint_uses_promtool(self):
        self.assertTrue(ALERT_VALIDATOR.is_file())
        script_text = ALERT_VALIDATOR.read_text(encoding="utf-8")
        self.assertIn("promtool", script_text)
        self.assertIn("check", script_text)
        self.assertIn("rules", script_text)
        self.assertIn("operations-alerts.yaml", script_text)


if __name__ == "__main__":
    unittest.main()
