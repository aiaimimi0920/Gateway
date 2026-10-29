"""Retired DashScope product has no built-in wiring; other Qwen lines remain."""
import json
import pathlib
import re
import unittest

ROOT = pathlib.Path(__file__).resolve().parents[2]


class QwenDashscopeRetirementTests(unittest.TestCase):
    def test_default_routes_do_not_restore_retired_provider(self):
        for name in ("routes.yaml", "routes.example.yaml"):
            source = (ROOT / name).read_text(encoding="utf-8")
            self.assertIsNone(re.search(r"(?m)^\s*(?:- id|preset):\s*(?:qwen|qwen-dashscope-openai)\s*$", source), name)
            self.assertNotIn("QWEN_DASHSCOPE_API_KEY", source)
            for preset in ("qwen-coding-plan-openai", "qwen-coding-plan-anthropic", "qwen-web-chat"):
                self.assertIn(f"preset: {preset}", source)

    def test_production_sources_have_no_retired_product_wiring(self):
        paths = [
            "src/preset.rs", "src/preset/qwen.rs", "src/preset/registry.rs",
            "src/protocol/qwen/official_api.rs", "src/protocol/qwen/official_api_disabled.rs",
            "src/protocol/registry/profiles.rs", "src/protocol/registry/profile_inference.rs",
            "src/implementation_lines/profiles.rs", "src/implementation_lines/payload.rs",
            "src/provider_credential_folder_sync/classification.rs",
            "src/provider_credential_folder_sync/canonicalization.rs",
            "src/provider_credential_folder_sync/payload_metadata.rs",
            "apps/desktop/src/features/console/providerBrandLabel.ts",
            "apps/desktop/src/features/console/providerCatalogWorkflow.ts",
            "apps/desktop/src/features/console/providerVerificationRegistry.ts",
        ]
        for relative in paths:
            source = (ROOT / relative).read_text(encoding="utf-8")
            # Inline unit tests may retain names only to reject retired inputs.
            source = source.split("#[cfg(test)]", 1)[0]
            with self.subTest(path=relative):
                self.assertIsNone(re.search(r"(?i)qwen[-_]dashscope|qwen_preset", source), relative)

    def test_shared_qwen_manifest_describes_retained_coding_plan(self):
        manifest = json.loads((ROOT / "manifests/lines/qwen/official-api.json").read_text(encoding="utf-8"))
        self.assertEqual(manifest["identity"]["providerSurfaceKey"], "qwen-coding-plan-openai")
        self.assertEqual(manifest["compilation"]["lineFeature"], "line-qwen-official-api")
        self.assertNotRegex(json.dumps(manifest), r"(?i)qwen[-_]dashscope")


if __name__ == "__main__":
    unittest.main()
