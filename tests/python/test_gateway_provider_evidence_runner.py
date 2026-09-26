import json
import os
import pathlib
import tempfile
import unittest

from gateway_evidence_runner_fixture import EvidenceRunnerFixture, GATEWAY_ROOT, RUNNER


class GatewayProviderEvidenceRunnerTests(EvidenceRunnerFixture, unittest.TestCase):
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


if __name__ == "__main__":
    unittest.main()


