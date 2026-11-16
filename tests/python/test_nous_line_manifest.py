"""Nous product metadata must resolve to its existing owner without fake accounts or evidence."""
import json
import pathlib
import subprocess
import sys
import unittest

from powershell_test_utils import powershell_executable

ROOT = pathlib.Path(__file__).resolve().parents[2]
MANIFEST = ROOT / "manifests/lines/nous/portal-oauth.json"


class NousLineManifestTests(unittest.TestCase):
    def test_identity_feature_and_source_owners(self):
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        self.assertEqual(manifest["id"], "nous-portal-oauth")
        self.assertEqual(manifest["identity"]["protocolProfile"], "nous_portal_oauth")
        self.assertEqual(manifest["identity"]["adapter"], "openai_compatible")
        self.assertEqual(manifest["identity"]["executionMode"], "direct_http")
        self.assertEqual(manifest["compilation"]["lineFeature"], "line-nous-portal-oauth")
        self.assertEqual(manifest["verification"]["focusedCargoFilters"], ["nous_"])
        self.assertEqual(set(manifest["capabilities"]["families"]), {"conversation", "embeddings", "models"})
        for source in manifest["implementation"].values():
            self.assertTrue((ROOT / source.removeprefix("gateway/")).exists(), source)
        result = subprocess.run(
            [sys.executable, str(ROOT / "tools/validate-gateway-line-manifests.py"),
             "--manifest", str(MANIFEST), "--as-json"],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8",
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)

    def test_sample_has_two_fixed_empty_pools_and_no_credentials(self):
        manifest = json.loads(MANIFEST.read_text(encoding="utf-8"))
        sample = json.loads((ROOT / manifest["credentials"]["samplePath"]).read_text(encoding="utf-8"))
        providers = sample["providers"]
        self.assertEqual(len(providers), 2)
        self.assertEqual({p["base_url"] for p in providers}, {
            "https://inference-api.nousresearch.com/v1", "https://welcome-api.nousresearch.com/v1",
        })
        for provider in providers:
            self.assertEqual(set(provider), {"id", "preset", "auth_mode", "base_url", "explicit_credentials_only", "credentials"})
            self.assertEqual(provider["preset"], "nous-portal-oauth")
            self.assertEqual(provider["auth_mode"], "nous_portal_oauth")
            self.assertIs(provider["explicit_credentials_only"], True)
            self.assertEqual(provider["credentials"], [])
        self.assertEqual(sample["model_routes"], [])

    def test_line_verification_resolves_nous_without_claiming_cargo_execution(self):
        result = subprocess.run(
            [powershell_executable(), "-NoProfile", "-File", str(ROOT / "tools/verify-gateway-line.ps1"),
             "-LineId", "nous-portal-oauth", "-SkipCargo", "-AsJson"],
            cwd=ROOT, capture_output=True, text=True, encoding="utf-8",
        )
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        payload = json.loads(result.stdout[result.stdout.index("{"):])
        self.assertEqual(payload["lineId"], "nous-portal-oauth")
        self.assertEqual(payload["status"], "pass")
        self.assertIs(payload["cargoSkipped"], True)


if __name__ == "__main__":
    unittest.main()
