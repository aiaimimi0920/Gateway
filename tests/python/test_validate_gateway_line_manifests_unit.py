import importlib.util
import pathlib
import sys
import unittest


def load_validator_module():
    module_path = (
        pathlib.Path(__file__).resolve().parents[2]
        / "tools"
        / "validate-gateway-line-manifests.py"
    )
    spec = importlib.util.spec_from_file_location("validate_gateway_line_manifests", module_path)
    if spec is None or spec.loader is None:
        raise RuntimeError(f"Unable to load validator module from {module_path}")
    module = importlib.util.module_from_spec(spec)
    sys.modules[spec.name] = module
    spec.loader.exec_module(module)
    return module


class GatewayLineManifestValidatorTests(unittest.TestCase):
    @classmethod
    def setUpClass(cls):
        cls.validator = load_validator_module()

    def test_gateway_relative_path_strips_legacy_gateway_prefix(self):
        resolved = self.validator.gateway_relative_path(
            "gateway/src/protocol/chatgpt/official_api.rs"
        )

        self.assertEqual(
            resolved,
            self.validator.GATEWAY_ROOT / "src/protocol/chatgpt/official_api.rs",
        )

    def test_gateway_relative_path_accepts_already_relative_paths(self):
        resolved = self.validator.gateway_relative_path(
            "src/protocol/chatgpt/official_api.rs"
        )

        self.assertEqual(
            resolved,
            self.validator.GATEWAY_ROOT / "src/protocol/chatgpt/official_api.rs",
        )

    def test_cargo_feature_declared_detects_line_features(self):
        cargo_text = "line-chatgpt-official-api = []\nfamily-openai-compatible-official-api = []\n"

        self.assertTrue(
            self.validator.cargo_feature_declared("line-chatgpt-official-api", cargo_text)
        )
        self.assertFalse(
            self.validator.cargo_feature_declared("line-missing-provider", cargo_text)
        )

    def test_validate_required_file_ignores_historical_docs_references(self):
        issues = []

        self.validator.validate_required_file(
            issues,
            "chatgpt-official-api",
            "$.docs.overviewDocPath",
            "docs/20-ai-gateway/legacy.md",
        )

        self.assertEqual(issues, [])

    def test_validate_required_file_reports_missing_gateway_file(self):
        issues = []

        self.validator.validate_required_file(
            issues,
            "chatgpt-official-api",
            "$.implementation.protocolModule",
            "gateway/src/protocol/does_not_exist.rs",
        )

        self.assertEqual(len(issues), 1)
        self.assertEqual(issues[0].code, "manifest.path.missing")


if __name__ == "__main__":
    unittest.main()
