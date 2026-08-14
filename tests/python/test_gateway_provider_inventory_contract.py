import json
import importlib.util
import os
import pathlib
import subprocess
import sys
import tempfile
import unittest


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
GENERATOR = GATEWAY_ROOT / "tools" / "generate-gateway-provider-inventory.py"
VALIDATOR = GATEWAY_ROOT / "tools" / "validate-gateway-provider-evidence.py"
SCHEMA = GATEWAY_ROOT / "docs" / "provider-inventory.schema.json"

CANONICAL_PROVIDER_LINE_IDS = {
    "accio-web-reverse-api",
    "aistudio-official",
    "aistudio-web-reverse",
    "anthropic-messages-official-model-api",
    "aws-bedrock-converse-official-model-api",
    "azure-openai-official-vendor-api",
    "chataibot-web-reverse",
    "chatgpt-codex-oauth-official",
    "chatgpt-official-api",
    "chatgpt-web-reverse",
    "cohere-chat-official-model-api",
    "deepseek-openai-official-model-api",
    "exa-search-official-vendor-api",
    "freebuff-web-reverse-api",
    "gemini-canvas-program",
    "gemini-web-reverse",
    "google-agent-platform-official",
    "grok-web-reverse-api",
    "groq-openai-official-vendor-api",
    "jina-reader-official-vendor-api",
    "jina-search-official-vendor-api",
    "kiro-official-vendor-api",
    "linkup-search-official-vendor-api",
    "longcat-openai-official-model-api",
    "lumalabs-web-reverse-api",
    "mistral-openai-official-model-api",
    "muyuan-openai-aggregator-api",
    "nvidia-openai-official-vendor-api",
    "openrouter-openai-aggregator-api",
    "perplexity-chat-official-vendor-api",
    "perplexity-search-official-vendor-api",
    "producer-web-reverse-api",
    "poe-openai-aggregator-api",
    "qwen-official-api",
    "qwen-web-reverse",
    "suno-web-reverse-api",
    "tavily-search-official-vendor-api",
    "together-openai-aggregator-api",
    "udio-web-reverse-api",
    "websearchapi-search-official-vendor-api",
    "xai-openai-official-vendor-api",
    "xfyun-native-websocket-official-vendor-api",
    "xfyun-openai-official-vendor-api",
    "you-search-official-vendor-api",
}


def load_tool(path, module_name):
    spec = importlib.util.spec_from_file_location(module_name, path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Unable to load tool module: {path}")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


class GatewayProviderInventoryContractTests(unittest.TestCase):
    def run_generator(self, output_path, *extra_args, env=None):
        return subprocess.run(
            [
                sys.executable,
                str(GENERATOR),
                "--output",
                str(output_path),
                *extra_args,
            ],
            cwd=GATEWAY_ROOT,
            env=env,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def run_validator(self, inventory_path):
        return subprocess.run(
            [
                sys.executable,
                str(VALIDATOR),
                "--inventory",
                str(inventory_path),
                "--as-json",
            ],
            cwd=GATEWAY_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_offline_generator_is_deterministic_and_covers_each_manifest_once(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            first_path = pathlib.Path(temp_dir) / "first.json"
            second_path = pathlib.Path(temp_dir) / "second.json"
            env = os.environ.copy()
            env["GATEWAY_API_KEY"] = "sk-contract-test-must-not-leak"
            env["OPENAI_API_KEY"] = "sk-openai-contract-test-must-not-leak"

            first = self.run_generator(first_path, env=env)
            second = self.run_generator(second_path, env=env)

            self.assertEqual(
                first.returncode,
                0,
                msg=f"stdout:\n{first.stdout}\nstderr:\n{first.stderr}",
            )
            self.assertEqual(
                second.returncode,
                0,
                msg=f"stdout:\n{second.stdout}\nstderr:\n{second.stderr}",
            )
            self.assertEqual(first_path.read_bytes(), second_path.read_bytes())

            payload = json.loads(first_path.read_text(encoding="utf-8"))
            self.assertEqual(payload["schemaVersion"], "gateway-product-inventory/v1")
            self.assertIsInstance(payload["sourceRevision"], str)
            self.assertTrue(payload["sourceRevision"])
            self.assertEqual(payload["evidenceSummary"]["mode"], "offline")

            lines = payload["lines"]
            ids = [line["id"] for line in lines]
            manifest_paths = [line["manifestPath"] for line in lines]
            self.assertEqual(ids, sorted(ids))
            self.assertEqual(len(ids), len(set(ids)))
            self.assertEqual(len(manifest_paths), len(set(manifest_paths)))
            self.assertEqual(len(lines), 44)
            self.assertEqual(
                payload["evidenceSummary"]["counts"]["metadata_only"], 44
            )

            serialized = first_path.read_text(encoding="utf-8")
            self.assertNotIn("sk-contract-test-must-not-leak", serialized)
            self.assertNotIn("sk-openai-contract-test-must-not-leak", serialized)
            self.assertNotIn("GATEWAY_API_KEY", serialized)
            self.assertNotIn("OPENAI_API_KEY", serialized)

    def test_line_joins_cargo_implementation_route_and_verification_metadata(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            output_path = pathlib.Path(temp_dir) / "inventory.json"
            result = self.run_generator(output_path)
            self.assertEqual(
                result.returncode,
                0,
                msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
            )
            payload = json.loads(output_path.read_text(encoding="utf-8"))

        by_id = {line["id"]: line for line in payload["lines"]}
        openrouter = by_id["openrouter-openai-aggregator-api"]
        self.assertTrue(openrouter["compilation"]["cargo"]["lineFeatureDeclared"])
        self.assertIn(
            "family-openai-compatible-official-api",
            openrouter["compilation"]["cargo"]["declaredDependencies"],
        )
        self.assertEqual(
            openrouter["implementationLineMetadata"]["canonicalProtocolProfile"],
            "openrouter",
        )
        self.assertTrue(
            openrouter["implementationLineMetadata"]["protocolProfileMapped"]
        )
        self.assertTrue(openrouter["implementationLineMetadata"]["featureMapped"])
        self.assertIn("openai_chat", openrouter["routeMetadata"]["wireProtocolFamilies"])
        self.assertEqual(
            openrouter["verification"]["fixtureSuiteId"], "openrouter_fixture"
        )
        self.assertEqual(
            openrouter["verification"]["liveSuiteId"], "openrouter_live"
        )
        self.assertEqual(
            openrouter["credentials"]["materialKinds"], ["api_key"]
        )
        self.assertNotIn("apiKey", openrouter["credentials"])
        self.assertNotIn("token", openrouter["credentials"])

    def test_explicit_evidence_replaces_synthetic_default_at_same_timestamp(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            inventory_path = temp_root / "inventory.json"
            evidence_path = temp_root / "evidence.json"
            timestamp = "2026-07-18T00:00:00Z"
            evidence_path.write_text(
                json.dumps(
                    {
                        "lineId": "openrouter-openai-aggregator-api",
                        "state": "fixture_passed",
                        "timestamp": timestamp,
                        "mode": "fixture",
                        "classification": {
                            "failureClass": "none",
                            "failureCode": "fixture_passed",
                            "message": "Offline fixture passed",
                        },
                    }
                ),
                encoding="utf-8",
            )

            result = self.run_generator(
                inventory_path,
                "--timestamp",
                timestamp,
                "--evidence",
                str(evidence_path),
            )
            self.assertEqual(result.returncode, 0, msg=result.stderr)
            payload = json.loads(inventory_path.read_text(encoding="utf-8"))

        line = next(
            item
            for item in payload["lines"]
            if item["id"] == "openrouter-openai-aggregator-api"
        )
        self.assertEqual(line["evidence"]["state"], "fixture_passed")
        self.assertEqual(
            [record["state"] for record in line["evidence"]["records"]],
            ["fixture_passed"],
        )
        self.assertEqual(payload["evidenceSummary"]["counts"]["fixture_passed"], 1)
        self.assertEqual(payload["evidenceSummary"]["counts"]["metadata_only"], 43)

    def test_validator_requires_timestamp_and_classified_failure_for_non_compiled_state(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            inventory_path = pathlib.Path(temp_dir) / "inventory.json"
            result = self.run_generator(inventory_path)
            self.assertEqual(result.returncode, 0, msg=result.stderr)
            payload = json.loads(inventory_path.read_text(encoding="utf-8"))

            line = payload["lines"][0]
            line["evidence"]["state"] = "credential_missing"
            line["evidence"]["records"] = [
                {
                    "state": "credential_missing",
                    "timestamp": "2026-07-18T00:00:00Z",
                    "classification": {
                        "failureClass": "credential",
                        "failureCode": "missing_credential",
                        "message": "Credential label is unavailable",
                    },
                }
            ]
            payload["evidenceSummary"]["counts"]["metadata_only"] -= 1
            payload["evidenceSummary"]["counts"]["credential_missing"] += 1
            inventory_path.write_text(
                json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            valid = self.run_validator(inventory_path)
            self.assertEqual(
                valid.returncode,
                0,
                msg=f"stdout:\n{valid.stdout}\nstderr:\n{valid.stderr}",
            )

            line["evidence"]["records"][0].pop("timestamp")
            inventory_path.write_text(
                json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            invalid = self.run_validator(inventory_path)
            self.assertNotEqual(invalid.returncode, 0)
            report = json.loads(invalid.stdout)
            self.assertEqual(report["status"], "fail")
            self.assertTrue(
                any(issue["code"] == "evidence.timestamp.missing" for issue in report["issues"])
            )

    def test_schema_reference_is_relative_to_output_and_validator_uses_jsonschema(self):
        with tempfile.TemporaryDirectory(dir=GATEWAY_ROOT) as temp_dir:
            inventory_path = pathlib.Path(temp_dir) / "nested" / "inventory.json"
            result = self.run_generator(inventory_path)
            self.assertEqual(result.returncode, 0, msg=result.stderr)
            payload = json.loads(inventory_path.read_text(encoding="utf-8"))

            expected_schema = os.path.relpath(
                SCHEMA, inventory_path.parent
            ).replace(os.sep, "/")
            self.assertEqual(payload["$schema"], expected_schema)

            payload["unexpectedRootProperty"] = True
            inventory_path.write_text(
                json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            invalid = self.run_validator(inventory_path)
            self.assertNotEqual(invalid.returncode, 0)
            report = json.loads(invalid.stdout)
            self.assertTrue(
                any(issue["code"] == "inventory.schema.invalid" for issue in report["issues"]),
                report,
            )

    def test_canonical_provider_line_contract_does_not_shrink_with_manifest_directory(self):
        validator = load_tool(VALIDATOR, "gateway_provider_evidence_validator_test")
        generator = load_tool(GENERATOR, "gateway_provider_inventory_generator_test")

        with tempfile.TemporaryDirectory() as temp_dir:
            validator.MANIFEST_ROOT = pathlib.Path(temp_dir)
            self.assertEqual(validator.expected_manifest_ids(), CANONICAL_PROVIDER_LINE_IDS)
            with tempfile.TemporaryDirectory() as inventory_temp:
                inventory_path = pathlib.Path(inventory_temp) / "inventory.json"
                generated = self.run_generator(inventory_path)
                self.assertEqual(generated.returncode, 0, msg=generated.stderr)
                payload = json.loads(inventory_path.read_text(encoding="utf-8"))
                issues = validator.validate_inventory(payload)
                self.assertTrue(
                    any(
                        item["code"]
                        == "inventory.manifest_source_coverage.mismatch"
                        for item in issues
                    ),
                    issues,
                )

        discovered = generator.discover_manifests()
        self.assertEqual(
            set(generator.CANONICAL_PROVIDER_LINE_IDS), CANONICAL_PROVIDER_LINE_IDS
        )
        generator.discover_manifests = lambda: discovered[1:]
        with self.assertRaisesRegex(generator.InventoryError, "missing canonical"):
            generator.build_inventory(
                timestamp="2026-07-18T00:00:00Z", explicit_evidence={}
            )

    def test_live_passed_requires_live_mode_and_observed_route_proof(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            inventory_path = temp_root / "inventory.json"
            evidence_path = temp_root / "evidence.json"
            evidence = {
                "lineId": "openrouter-openai-aggregator-api",
                "state": "live_passed",
                "timestamp": "2026-07-18T00:00:00Z",
                "mode": "offline",
                "classification": {
                    "failureClass": "none",
                    "failureCode": "live_canary_passed",
                    "message": "Canary passed",
                },
            }
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")

            wrong_mode = self.run_generator(
                inventory_path, "--evidence", str(evidence_path)
            )
            self.assertNotEqual(wrong_mode.returncode, 0)
            self.assertIn("live mode", wrong_mode.stderr)

            evidence["mode"] = "live"
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            missing_proof = self.run_generator(
                inventory_path, "--evidence", str(evidence_path)
            )
            self.assertNotEqual(missing_proof.returncode, 0)
            self.assertIn("route proof", missing_proof.stderr)

            evidence["routeProof"] = {
                "providerLine": "openrouter-openai-aggregator-api",
                "source": "gateway-response-header",
            }
            evidence["observedProvider"] = {
                "providerLine": "openrouter-openai-aggregator-api"
            }
            evidence_path.write_text(json.dumps(evidence), encoding="utf-8")
            valid = self.run_generator(
                inventory_path, "--evidence", str(evidence_path)
            )
            self.assertEqual(valid.returncode, 0, msg=valid.stderr)
            validated = self.run_validator(inventory_path)
            self.assertEqual(validated.returncode, 0, msg=validated.stdout)

    def test_artifact_paths_must_be_gateway_relative(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            inventory_path = pathlib.Path(temp_dir) / "inventory.json"
            result = self.run_generator(inventory_path)
            self.assertEqual(result.returncode, 0, msg=result.stderr)
            payload = json.loads(inventory_path.read_text(encoding="utf-8"))
            record = payload["lines"][0]["evidence"]["records"][0]
            record["artifactPaths"] = ["C:/secrets/provider-response.json"]
            inventory_path.write_text(
                json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )

            invalid = self.run_validator(inventory_path)
            self.assertNotEqual(invalid.returncode, 0)
            report = json.loads(invalid.stdout)
            self.assertTrue(
                any(
                    issue["code"] == "evidence.artifact_path.not_gateway_relative"
                    for issue in report["issues"]
                ),
                report,
            )

    def test_secret_scrubber_covers_basic_aws_google_and_cookie_material(self):
        generator = load_tool(GENERATOR, "gateway_provider_inventory_scrubber_test")
        secret_values = (
            "Basic dXNlcjpwYXNzd29yZA==",
            "AKIAIOSFODNN7EXAMPLE",
            "AIzaSyD-example-google-api-key-123456",
            "sessionid=super-secret-cookie-value",
        )
        for secret in secret_values:
            with self.subTest(secret=secret.split("=", 1)[0]):
                scrubbed = generator.sanitize(f"source:{secret}")
                self.assertNotIn(secret, scrubbed)
                self.assertIn("<redacted>", scrubbed)

    def test_explicit_evidence_scrubs_labels_messages_and_normalizes_gateway_paths(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            temp_root = pathlib.Path(temp_dir)
            inventory_path = temp_root / "inventory.json"
            evidence_path = temp_root / "evidence.json"
            evidence_path.write_text(
                json.dumps(
                    {
                        "lineId": "openrouter-openai-aggregator-api",
                        "state": "external_gate",
                        "timestamp": "2026-07-18T00:00:00Z",
                        "mode": "live",
                        "credentialSource": "Basic dXNlcjpwYXNzd29yZA==",
                        "artifactPaths": [
                            str(
                                GATEWAY_ROOT
                                / "target"
                                / "provider-evidence"
                                / "safe.log"
                            )
                        ],
                        "classification": {
                            "failureClass": "external",
                            "failureCode": "operator_classified",
                            "message": "provider returned AKIAIOSFODNN7EXAMPLE",
                        },
                    }
                ),
                encoding="utf-8",
            )

            result = self.run_generator(
                inventory_path, "--evidence", str(evidence_path)
            )
            self.assertEqual(result.returncode, 0, msg=result.stderr)
            serialized = inventory_path.read_text(encoding="utf-8")
            self.assertNotIn("dXNlcjpwYXNzd29yZA", serialized)
            self.assertNotIn("AKIAIOSFODNN7EXAMPLE", serialized)
            payload = json.loads(serialized)
            line = next(
                item
                for item in payload["lines"]
                if item["id"] == "openrouter-openai-aggregator-api"
            )
            record = next(
                item
                for item in line["evidence"]["records"]
                if item["state"] == "external_gate"
            )
            self.assertEqual(
                record["artifactPaths"],
                ["target/provider-evidence/safe.log"],
            )

    def test_schema_file_declares_inventory_and_evidence_contract(self):
        schema = json.loads(SCHEMA.read_text(encoding="utf-8"))
        self.assertEqual(schema["$id"], "https://neuroloom.local/schemas/gateway-product-inventory.schema.json")
        self.assertEqual(schema["properties"]["schemaVersion"]["const"], "gateway-product-inventory/v1")
        self.assertEqual(
            schema["$defs"]["evidenceState"]["enum"],
            [
                "compiled",
                "metadata_only",
                "fixture_passed",
                "live_passed",
                "external_gate",
                "credential_missing",
                "known_unsupported",
            ],
        )


if __name__ == "__main__":
    unittest.main()
