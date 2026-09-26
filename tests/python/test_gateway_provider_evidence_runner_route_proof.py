import json
import pathlib
import tempfile
import unittest

from gateway_evidence_runner_fixture import EvidenceRunnerFixture


class GatewayEvidenceRunnerRouteProofTests(EvidenceRunnerFixture, unittest.TestCase):
    def test_live_passed_requires_matching_versioned_request_bound_route_proof(self):
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
            self.assertEqual(record["mode"], "live")
            self.assertEqual(
                record["routeProof"]["providerLine"],
                "openrouter-openai-aggregator-api",
            )
            self.assertEqual(
                record["observedProvider"]["providerLine"],
                "openrouter-openai-aggregator-api",
            )

    def test_runner_rejects_untrusted_route_proof_shapes_and_statuses(self):
        cases = (
            ("wrong-source", "legacy_route_proof", "proof-request-1", "proof-request-1", 200),
            ("missing-request-id", "gateway_response_headers_v1", "", "", 200),
            ("non-2xx", "gateway_response_headers_v1", "proof-request-1", "proof-request-1", 500),
        )
        for case_name, source, proof_request_id, result_request_id, http_status in cases:
            with self.subTest(case=case_name):
                with tempfile.TemporaryDirectory() as temp_dir:
                    temp_root = pathlib.Path(temp_dir)
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
[pscustomobject]@{{
  status = 'pass'
  reason = $null
  summaryPath = $null
  targets = @([pscustomobject]@{{
    provider_line = 'openrouter-openai-aggregator-api'
    observed_provider_line = 'openrouter-openai-aggregator-api'
    route_proof = [pscustomobject]@{{
      source = '{source}'
      provider_line = 'openrouter-openai-aggregator-api'
      request_id = '{proof_request_id}'
    }}
    endpoint = '/v1/chat/completions'
    method = 'POST'
    http_status = {http_status}
    request_id = '{result_request_id}'
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
                                        "name": "openrouter-canary",
                                        "provider_line": "openrouter-openai-aggregator-api",
                                        "credential_source": "vault:openrouter-primary",
                                        "endpoint": "/v1/chat/completions",
                                        "method": "POST",
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

                    self.assertNotEqual(result.returncode, 0)
                    summary = json.loads(result.stdout)
                    evidence = json.loads(
                        pathlib.Path(summary["evidencePath"]).read_text(encoding="utf-8")
                    )
                    record = evidence["records"][0]
                    self.assertEqual(record["state"], "external_gate")
                    self.assertEqual(record["classification"]["failureClass"], "route_proof")

    def test_route_proof_mismatch_is_classified_as_route_proof_external_gate(self):
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
  reason = $null
  summaryPath = $null
  targets = @([pscustomobject]@{
    provider_line = 'openrouter-openai-aggregator-api'
    observed_provider_line = 'together-openai-aggregator-api'
    route_proof = [pscustomobject]@{
      source = 'gateway_response_headers_v1'
      provider_line = 'together-openai-aggregator-api'
      request_id = 'proof-request-1'
    }
    endpoint = '/v1/chat/completions'
    method = 'POST'
    http_status = 200
    status = 'fail'
    failure_classification = 'route_proof_missing_or_mismatch'
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
                                "credential_source": "env:OPENROUTER_API_KEY",
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

            self.assertNotEqual(result.returncode, 0)
            summary = json.loads(result.stdout)
            evidence = json.loads(
                pathlib.Path(summary["evidencePath"]).read_text(encoding="utf-8")
            )
            record = evidence["records"][0]
            self.assertEqual(record["state"], "external_gate")
            self.assertEqual(record["classification"]["failureClass"], "route_proof")
            self.assertEqual(
                record["classification"]["failureCode"],
                "live_canary_route_proof_missing_or_mismatch",
            )


if __name__ == "__main__":
    unittest.main()
