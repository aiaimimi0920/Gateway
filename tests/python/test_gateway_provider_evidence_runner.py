import json
import os
import pathlib
import subprocess
import tempfile
import unittest

from powershell_test_utils import powershell_executable


REPO_ROOT = pathlib.Path(__file__).resolve().parents[2]
GATEWAY_ROOT = REPO_ROOT
RUNNER = GATEWAY_ROOT / "tools" / "run-gateway-line-evidence.ps1"


class GatewayProviderEvidenceRunnerTests(unittest.TestCase):
    def run_runner(self, *arguments, env=None):
        return subprocess.run(
            [
                powershell_executable(),
                "-NoProfile",
                "-ExecutionPolicy",
                "Bypass",
                "-File",
                str(RUNNER),
                *map(str, arguments),
            ],
            cwd=REPO_ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_default_offline_run_persists_representative_secret_free_evidence(self):
        expected_lines = {
            "anthropic-messages-official-model-api": "official_http",
            "chatgpt-web-reverse": "browser_backed",
            "freebuff-web-reverse-api": "direct_http_replay",
            "jina-search-official-vendor-api": "search",
            "suno-web-reverse-api": "media",
            "xfyun-native-websocket-official-vendor-api": "websocket",
        }

        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            evidence_root = temp_root / "evidence"
            artifact_root = temp_root / "artifacts"
            inventory_path = temp_root / "provider-inventory.json"
            secret = "sk-runner-contract-must-not-leak"
            env = os.environ.copy()
            env["OPENAI_API_KEY"] = secret
            env["GATEWAY_API_KEY"] = secret

            result = self.run_runner(
                "-SkipCargo",
                "-EvidenceRoot",
                evidence_root,
                "-ArtifactRoot",
                artifact_root,
                "-InventoryPath",
                inventory_path,
                "-GatewayBaseUrl",
                "http://127.0.0.1:1",
                "-GatewayApiKey",
                secret,
                "-TargetsPath",
                temp_root / "missing-live-targets.json",
                "-AsJson",
                env=env,
            )

            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            summary = json.loads(result.stdout)
            evidence_path = pathlib.Path(summary["evidencePath"])
            evidence = json.loads(evidence_path.read_text(encoding="utf-8"))
            inventory_text = inventory_path.read_text(encoding="utf-8")

        self.assertEqual(summary["status"], "pass")
        self.assertEqual(summary["mode"], "offline")
        self.assertFalse(summary["allowLiveProviderCalls"])
        self.assertIsNone(summary["liveCanary"])
        self.assertEqual(evidence["schemaVersion"], "gateway-line-evidence/v1")
        self.assertEqual(evidence["mode"], "offline")
        self.assertFalse(evidence["allowLiveProviderCalls"])

        records = {record["lineId"]: record for record in evidence["records"]}
        self.assertEqual(set(records), set(expected_lines))
        self.assertEqual(
            {record["executionMode"] for record in records.values()},
            {"direct_http", "direct_http_replay", "browser_backed"},
        )
        for line_id, category in expected_lines.items():
            record = records[line_id]
            self.assertEqual(record["status"], "pass")
            self.assertEqual(record["state"], "metadata_only")
            self.assertEqual(record["verificationCategory"], category)
            self.assertTrue(record["executionMode"])
            self.assertTrue(record["endpointFamily"])
            self.assertTrue(record["credentialSource"].startswith("manifest:"))
            self.assertFalse(record["fallback"]["used"])
            self.assertEqual(
                set(record["readiness"]),
                {
                    "remoteExecutor",
                    "localBrowserFallback",
                    "sessionMaterial",
                    "credentialRefresh",
                    "leaseCooling",
                    "modelHealth",
                },
            )
            for artifact_path in record["artifactPaths"]:
                self.assertFalse(pathlib.PurePosixPath(artifact_path).is_absolute())
                self.assertNotIn("..", pathlib.PurePosixPath(artifact_path).parts)
                self.assertNotIn(":", artifact_path)

        for artifact_path in evidence["artifacts"].values():
            if artifact_path is None:
                continue
            self.assertFalse(pathlib.PurePosixPath(artifact_path).is_absolute())
            self.assertNotIn("..", pathlib.PurePosixPath(artifact_path).parts)
            self.assertNotIn(":", artifact_path)

        browser = records["chatgpt-web-reverse"]
        self.assertTrue(browser["readiness"]["remoteExecutor"]["required"])
        self.assertTrue(browser["readiness"]["localBrowserFallback"]["required"])
        self.assertTrue(browser["readiness"]["sessionMaterial"]["required"])
        self.assertEqual(browser["fallback"]["declaredMode"], "browser_backed")
        self.assertEqual(browser["fallback"]["observedMode"], "browser_backed")

        serialized = json.dumps(evidence, ensure_ascii=False)
        self.assertNotIn(secret, serialized)
        self.assertNotIn(secret, inventory_text)
        self.assertNotIn("OPENAI_API_KEY", serialized)
        self.assertNotIn("GATEWAY_API_KEY", serialized)

    def test_live_mode_is_explicit_and_delegates_to_the_existing_canary(self):
        script = RUNNER.read_text(encoding="utf-8")
        provider_doc = (GATEWAY_ROOT / "docs" / "provider-evidence.md").read_text(
            encoding="utf-8"
        )
        evidence_readme = (GATEWAY_ROOT / "docs" / "evidence" / "README.md").read_text(
            encoding="utf-8"
        )

        self.assertIn("AllowLiveProviderCalls", script)
        self.assertIn("invoke-gateway-live-provider-canary.ps1", script)
        self.assertIn(
            'Join-Path $GatewayRoot "scripts\\invoke-gateway-live-provider-canary.ps1"',
            script,
        )
        self.assertIn("validate-gateway-line-manifests.py", script)
        self.assertIn("generate-gateway-provider-inventory.py", script)
        self.assertIn("validate-gateway-provider-evidence.py", script)
        self.assertIn("verify-gateway-line.ps1", script)
        self.assertNotIn("Invoke-WebRequest", script)
        self.assertNotIn("curl.exe", script)

        self.assertIn("external_gate", provider_doc)
        self.assertIn("credential_missing", provider_doc)
        self.assertIn("remote executor", provider_doc.lower())
        self.assertIn("local browser", provider_doc.lower())
        self.assertIn("session material", provider_doc.lower())
        self.assertIn("quota", provider_doc.lower())
        self.assertIn("challenge", provider_doc.lower())
        self.assertIn("region", provider_doc.lower())
        self.assertIn("GATEWAY_LIVE_ROUTE_PROOF_ENABLED", provider_doc)
        self.assertIn("X-Neuro-Gateway-Route-Proof-Request", provider_doc)
        self.assertIn("X-Neuro-Gateway-Provider-Line", provider_doc)
        self.assertIn("route_proof_missing_or_mismatch", provider_doc)
        self.assertIn("WebSocket", provider_doc)
        self.assertIn("gateway-line-evidence/v1", evidence_readme)

        with tempfile.TemporaryDirectory() as temp_dir:
            result = self.run_runner(
                "-AllowLiveProviderCalls",
                "-SkipCargo",
                "-EvidenceRoot",
                pathlib.Path(temp_dir) / "evidence",
                "-ArtifactRoot",
                pathlib.Path(temp_dir) / "artifacts",
                "-InventoryPath",
                pathlib.Path(temp_dir) / "provider-inventory.json",
                "-AsJson",
            )

        self.assertNotEqual(result.returncode, 0)
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "fail")
        self.assertEqual(payload["mode"], "live")
        self.assertTrue(payload["allowLiveProviderCalls"])
        self.assertIn("GatewayBaseUrl", payload["reason"])

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



