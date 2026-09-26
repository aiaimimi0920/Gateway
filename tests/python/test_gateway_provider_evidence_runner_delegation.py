import json
import pathlib
import tempfile
import unittest

from gateway_evidence_runner_fixture import EvidenceRunnerFixture


class GatewayEvidenceRunnerDelegationTests(EvidenceRunnerFixture, unittest.TestCase):
    def test_live_canary_tool_failure_is_not_misclassified_as_missing_credentials(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            fake_canary = temp_root / "fake-canary.ps1"
            fake_canary.write_text(
                """param(
  [switch]$AllowLiveProviderCalls,
  [string]$GatewayBaseUrl,
  [string]$GatewayApiKey,
  [string]$TargetsPath,
  [string]$ArtifactRoot,
  [switch]$AsJson
)
[pscustomobject]@{
  status = 'fail'
  reason = 'simulated canary tool failure'
  targets = @()
  summaryPath = $null
} | ConvertTo-Json -Depth 8
exit 1
""",
                encoding="utf-8",
            )
            targets_path = temp_root / "targets.json"
            secret_body = "cookie=sessionid=must-not-persist"
            targets_path.write_text(
                json.dumps(
                    {
                        "targets": [
                            {
                                "name": "openrouter-canary",
                                "provider_line": "openrouter-openai-aggregator-api",
                                "credential_source": "Basic dXNlcjpwYXNzd29yZA==",
                                "endpoint": "/v1/chat/completions",
                                "method": "POST",
                                "model": "test-model",
                                "body": {"prompt": secret_body},
                                "cookie": "must-not-persist",
                                "token": "must-not-persist",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )
            evidence_root = temp_root / "evidence"
            artifact_root = temp_root / "artifacts"
            inventory_path = temp_root / "provider-inventory.json"

            result = self.run_runner(
                "-AllowLiveProviderCalls",
                "-SkipCargo",
                "-LineId",
                "openrouter-openai-aggregator-api",
                "-GatewayBaseUrl",
                "http://127.0.0.1:1",
                "-GatewayApiKey",
                "gateway-test-secret",
                "-TargetsPath",
                targets_path,
                "-LiveCanaryPath",
                fake_canary,
                "-EvidenceRoot",
                evidence_root,
                "-ArtifactRoot",
                artifact_root,
                "-InventoryPath",
                inventory_path,
                "-AsJson",
            )

            self.assertNotEqual(result.returncode, 0)
            summary = json.loads(result.stdout)
            evidence = json.loads(
                pathlib.Path(summary["evidencePath"]).read_text(encoding="utf-8")
            )
            record = evidence["records"][0]
            self.assertEqual(record["state"], "external_gate")
            self.assertEqual(
                record["classification"]["failureCode"],
                "live_canary_execution_failed",
            )
            self.assertNotEqual(record["state"], "credential_missing")

            persisted = "\n".join(
                path.read_text(encoding="utf-8", errors="ignore")
                for path in [*evidence_root.rglob("*"), *artifact_root.rglob("*")]
                if path.is_file()
            )
            self.assertNotIn(secret_body, persisted)
            self.assertNotIn("dXNlcjpwYXNzd29yZA", persisted)
            self.assertNotIn("must-not-persist", persisted)

    def test_overall_canary_failure_preserves_a_complete_target_pass(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            fake_canary = temp_root / "fake-canary.ps1"
            fake_canary.write_text(
                """param(
  [switch]$AllowLiveProviderCalls,
  [string]$GatewayBaseUrl,
  [string]$GatewayApiKey,
  [string]$TargetsPath,
  [string]$ArtifactRoot,
  [switch]$AsJson
)
[pscustomobject]@{
  status = 'fail'
  reason = 'another target failed'
  summaryPath = $null
  targets = @([pscustomobject]@{
    provider_line = 'openrouter-openai-aggregator-api'
    observed_provider_line = 'openrouter-openai-aggregator-api'
    route_proof = [pscustomobject]@{
      source = 'gateway_response_headers_v1'
      provider_line = 'openrouter-openai-aggregator-api'
      request_id = 'proof-request-1'
    }
    endpoint = '/v1/chat/completions'
    method = 'POST'
    http_status = 200
    request_id = 'proof-request-1'
    status = 'pass'
    failure_classification = $null
  })
} | ConvertTo-Json -Depth 8
exit 1
""",
                encoding="utf-8",
            )
            targets_path = temp_root / "targets.json"
            targets_path.write_text(
                json.dumps(
                    {
                        "targets": [
                            {
                                "name": "openrouter-canary",
                                "provider_line": "openrouter-openai-aggregator-api",
                                "expected_provider_line": "openrouter-openai-aggregator-api",
                                "credential_source": "vault:openrouter-primary",
                                "endpoint": "/v1/chat/completions",
                                "method": "POST",
                                "model": "test-model",
                                "body": {"messages": []},
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            result = self.run_runner(
                "-AllowLiveProviderCalls",
                "-SkipCargo",
                "-LineId",
                "openrouter-openai-aggregator-api",
                "-GatewayBaseUrl",
                "http://127.0.0.1:1",
                "-GatewayApiKey",
                "gateway-test-secret",
                "-TargetsPath",
                targets_path,
                "-LiveCanaryPath",
                fake_canary,
                "-EvidenceRoot",
                temp_root / "evidence",
                "-ArtifactRoot",
                temp_root / "artifacts",
                "-InventoryPath",
                temp_root / "provider-inventory.json",
                "-AsJson",
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            summary = json.loads(result.stdout)
            evidence = json.loads(
                pathlib.Path(summary["evidencePath"]).read_text(encoding="utf-8")
            )
            record = evidence["records"][0]
            self.assertEqual(record["state"], "live_passed")
            self.assertEqual(record["status"], "pass")
            self.assertEqual(
                record["classification"]["failureCode"],
                "live_canary_passed",
            )

    def test_runner_passes_gateway_key_to_canary_via_environment(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            fake_canary = temp_root / "fake-canary.ps1"
            fake_canary.write_text(
                """param(
  [switch]$AllowLiveProviderCalls,
  [string]$GatewayBaseUrl,
  [string]$GatewayApiKey,
  [string]$TargetsPath,
  [string]$ArtifactRoot,
  [switch]$AsJson
)
if (-not [string]::IsNullOrWhiteSpace($GatewayApiKey)) {
  throw 'gateway key was exposed through child process arguments'
}
if ($env:GATEWAY_CANARY_API_KEY -ne 'gateway-test-secret') {
  throw 'gateway key was not provided through the canary environment'
}
[pscustomobject]@{
  status = 'pass'
  reason = $null
  summaryPath = $null
  targets = @([pscustomobject]@{
    provider_line = 'openrouter-openai-aggregator-api'
    observed_provider_line = 'openrouter-openai-aggregator-api'
    route_proof = [pscustomobject]@{
      source = 'gateway_response_headers_v1'
      provider_line = 'openrouter-openai-aggregator-api'
      request_id = 'proof-request-1'
    }
    endpoint = '/v1/chat/completions'
    method = 'POST'
    http_status = 200
    request_id = 'proof-request-1'
    status = 'pass'
    failure_classification = $null
  })
} | ConvertTo-Json -Depth 8
exit 0
""",
                encoding="utf-8",
            )
            targets_path = temp_root / "targets.json"
            targets_path.write_text(
                json.dumps(
                    {
                        "targets": [
                            {
                                "name": "openrouter-canary",
                                "provider_line": "openrouter-openai-aggregator-api",
                                "expected_provider_line": "openrouter-openai-aggregator-api",
                                "credential_source": "vault:openrouter-primary",
                                "endpoint": "/v1/chat/completions",
                                "method": "POST",
                                "model": "test-model",
                                "body": {"messages": []},
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            result = self.run_runner(
                "-AllowLiveProviderCalls",
                "-SkipCargo",
                "-LineId",
                "openrouter-openai-aggregator-api",
                "-GatewayBaseUrl",
                "http://127.0.0.1:1",
                "-GatewayApiKey",
                "gateway-test-secret",
                "-TargetsPath",
                targets_path,
                "-LiveCanaryPath",
                fake_canary,
                "-EvidenceRoot",
                temp_root / "evidence",
                "-ArtifactRoot",
                temp_root / "artifacts",
                "-InventoryPath",
                temp_root / "provider-inventory.json",
                "-AsJson",
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )

    def test_get_live_target_is_delegated_without_rewriting_and_can_pass(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            invocation_marker = temp_root / "canary-invoked.txt"
            fake_canary = temp_root / "fake-canary.ps1"
            fake_canary.write_text(
                f"""param(
  [switch]$AllowLiveProviderCalls,
  [string]$GatewayBaseUrl,
  [string]$GatewayApiKey,
  [string]$TargetsPath,
  [string]$ArtifactRoot,
  [switch]$AsJson
)
$payload = Get-Content -LiteralPath $TargetsPath -Raw -Encoding UTF8 | ConvertFrom-Json
$target = @($payload.targets)[0]
if ([string]$target.method -ne 'GET') {{ throw 'GET target was rewritten' }}
Set-Content -LiteralPath '{invocation_marker.as_posix()}' -Value invoked
[pscustomobject]@{{
  status = 'pass'
  reason = $null
  summaryPath = $null
  targets = @([pscustomobject]@{{
    provider_line = [string]$target.provider_line
    observed_provider_line = [string]$target.provider_line
    route_proof = [pscustomobject]@{{
      source = 'gateway_response_headers_v1'
      provider_line = [string]$target.provider_line
      request_id = 'get-proof-request-1'
    }}
    endpoint = [string]$target.endpoint
    method = [string]$target.method
    http_status = 200
    request_id = 'get-proof-request-1'
    status = 'pass'
    failure_classification = $null
  }})
}} | ConvertTo-Json -Depth 8
exit 0
""",
                encoding="utf-8",
            )
            targets_path = temp_root / "targets.json"
            targets_path.write_text(
                json.dumps(
                    {
                        "targets": [
                            {
                                "name": "get-canary",
                                "provider_line": "jina-reader-official-vendor-api",
                                "credential_source": "vault:jina-reader",
                                "endpoint": "/v1/read",
                                "method": "GET",
                            }
                        ]
                    }
                ),
                encoding="utf-8",
            )

            result = self.run_runner(
                "-AllowLiveProviderCalls",
                "-SkipCargo",
                "-LineId",
                "jina-reader-official-vendor-api",
                "-GatewayBaseUrl",
                "http://127.0.0.1:1",
                "-GatewayApiKey",
                "gateway-test-secret",
                "-TargetsPath",
                targets_path,
                "-LiveCanaryPath",
                fake_canary,
                "-EvidenceRoot",
                temp_root / "evidence",
                "-ArtifactRoot",
                temp_root / "artifacts",
                "-InventoryPath",
                temp_root / "provider-inventory.json",
                "-AsJson",
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            summary = json.loads(result.stdout)
            evidence = json.loads(
                pathlib.Path(summary["evidencePath"]).read_text(encoding="utf-8")
            )
            record = evidence["records"][0]
            self.assertEqual(record["state"], "live_passed")
            self.assertEqual(
                record["classification"]["failureCode"],
                "live_canary_passed",
            )
            self.assertEqual(record["targetSummary"]["method"], "GET")
            self.assertTrue(invocation_marker.exists())


if __name__ == "__main__":
    unittest.main()
