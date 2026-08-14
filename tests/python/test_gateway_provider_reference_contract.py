import importlib
import json
import pathlib
import re
import subprocess
import sys
import tempfile
import unittest
from unittest import mock


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[2]
GENERATOR = GATEWAY_ROOT / "tools" / "generate-gateway-provider-reference-report.py"
VALIDATOR = GATEWAY_ROOT / "tools" / "validate-gateway-provider-reference-report.py"
CHECKED_IN_REPORT = GATEWAY_ROOT / "docs" / "provider-reference-report.json"
CHECKED_IN_GUIDE = GATEWAY_ROOT / "docs" / "provider-reference-guide.md"
sys.path.insert(0, str(GATEWAY_ROOT / "tools"))
gateway_provider_references = importlib.import_module("gateway_provider_references")

ALLOWED_STATUSES = {"local_present", "external_legacy_reference"}
REFERENCE_FIELDS = (
    "credentials.samplePath",
    "credentials.fieldsDocPath",
    "docs.overviewDocPath",
    "docs.buildDocPath",
)
SECRET_LIKE_RE = re.compile(
    r"(?:Bearer\s+[A-Za-z0-9._~+/=-]+|Basic\s+[A-Za-z0-9+/=]{8,}|"
    r"sk-[A-Za-z0-9._-]+|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{20,}|"
    r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9._-]+\.[A-Za-z0-9._-]+|"
    r"(?:api[_-]?key|access[_-]?token|authorization|cookie|session[_-]?token|"
    r"refresh[_-]?token|password|secret)\s*[=:]\s*[^;\s,]+)",
    re.IGNORECASE,
)


class GatewayProviderReferenceContractTests(unittest.TestCase):
    def run_generator(self, report_path: pathlib.Path, guide_path: pathlib.Path):
        return subprocess.run(
            [
                sys.executable,
                str(GENERATOR),
                "--report",
                str(report_path),
                "--guide",
                str(guide_path),
                "--timestamp",
                "2026-07-19T00:00:00Z",
            ],
            cwd=GATEWAY_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def run_validator(self, report_path: pathlib.Path, guide_path: pathlib.Path):
        return subprocess.run(
            [
                sys.executable,
                str(VALIDATOR),
                "--report",
                str(report_path),
                "--guide",
                str(guide_path),
                "--as-json",
            ],
            cwd=GATEWAY_ROOT,
            text=True,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
        )

    def test_report_classifies_every_manifest_reference_exactly_once(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            report_path = root / "provider-reference-report.json"
            guide_path = root / "provider-reference-guide.md"
            result = self.run_generator(report_path, guide_path)
            self.assertEqual(result.returncode, 0, result.stderr)

            payload = json.loads(report_path.read_text(encoding="utf-8"))
            self.assertEqual(
                payload["schemaVersion"], "gateway-provider-reference-report/v1"
            )
            self.assertEqual(payload["summary"]["lineCount"], 44)
            self.assertEqual(payload["summary"]["declaredReferenceCount"], 176)
            self.assertEqual(payload["summary"]["uniqueReferenceCount"], 166)
            self.assertEqual(payload["summary"]["unclassifiedReferenceCount"], 0)

            references = payload["references"]
            keys = [(item["lineId"], item["field"]) for item in references]
            self.assertEqual(len(keys), 176)
            self.assertEqual(len(set(keys)), 176)
            self.assertEqual(
                {item["field"] for item in references}, set(REFERENCE_FIELDS)
            )
            self.assertTrue(
                all(item["status"] in ALLOWED_STATUSES for item in references)
            )

            unique = payload["uniqueReferences"]
            self.assertEqual(len(unique), 166)
            self.assertEqual(
                len({item["path"] for item in unique}), 166
            )
            self.assertEqual(len(payload["lines"]), 44)
            for line in payload["lines"]:
                self.assertEqual(len(line["legacyReferences"]), 4)
                self.assertIn("identity", line)
                self.assertIn("capabilities", line)
                self.assertIn("materialKinds", line)
                self.assertIn("manifestPath", line)
                self.assertIn("resolution", line)

            validated = self.run_validator(report_path, guide_path)
            self.assertEqual(validated.returncode, 0, validated.stderr)
            self.assertEqual(json.loads(validated.stdout)["status"], "pass")

    def test_source_revision_is_content_addressed_and_ignores_git_revision_changes(self):
        manifests = gateway_provider_references.discover_manifests()
        with mock.patch.object(
            gateway_provider_references,
            "_git",
            side_effect=["commit-before", "commit-after"],
        ):
            before = gateway_provider_references.source_revision(manifests)
            after = gateway_provider_references.source_revision(manifests)
        self.assertEqual(before, after)
        self.assertTrue(before.startswith("manifest-source:"))
        self.assertEqual(
            before.split(":", 1)[1],
            gateway_provider_references._source_fingerprint(manifests),
        )

    def test_source_fingerprint_normalizes_manifest_line_endings(self):
        manifest = gateway_provider_references.discover_manifests()[0]
        canonical = (
            manifest.read_bytes().replace(b"\r\n", b"\n").replace(b"\r", b"\n")
        )
        with mock.patch.object(
            pathlib.Path,
            "read_bytes",
            return_value=canonical,
        ):
            lf_fingerprint = gateway_provider_references._source_fingerprint([manifest])
        with mock.patch.object(
            pathlib.Path,
            "read_bytes",
            return_value=canonical.replace(b"\n", b"\r\n"),
        ):
            crlf_fingerprint = gateway_provider_references._source_fingerprint([manifest])
        self.assertEqual(lf_fingerprint, crlf_fingerprint)

    def test_legacy_paths_are_explicitly_classified_and_point_to_local_guide(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            report_path = root / "report.json"
            guide_path = root / "guide.md"
            result = self.run_generator(report_path, guide_path)
            self.assertEqual(result.returncode, 0, result.stderr)
            payload = json.loads(report_path.read_text(encoding="utf-8"))

            self.assertEqual(
                payload["summary"]["externalLegacyReferenceCount"], 176
            )
            self.assertEqual(payload["summary"]["localPresentReferenceCount"], 0)
            for item in payload["references"]:
                self.assertEqual(item["status"], "external_legacy_reference")
                self.assertEqual(item["resolution"]["guidePath"], guide_path.name)
                self.assertTrue(item["resolution"]["guideAnchor"].startswith("#"))

    def test_guide_uses_only_obvious_placeholders(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            report_path = root / "report.json"
            guide_path = root / "guide.md"
            result = self.run_generator(report_path, guide_path)
            self.assertEqual(result.returncode, 0, result.stderr)
            guide = guide_path.read_text(encoding="utf-8")
            self.assertIn("INSERT_PROVIDER_VALUE_HERE", guide)
            self.assertIn("PATH_TO_LOCAL_STATE_FILE", guide)
            self.assertIn(
                "python tools/generate-gateway-provider-reference-report.py",
                guide,
            )
            self.assertIn(
                "python tools/validate-gateway-provider-reference-report.py",
                guide,
            )
            self.assertNotRegex(guide, SECRET_LIKE_RE)

    def test_validator_rejects_duplicate_reference_classification(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            report_path = root / "report.json"
            guide_path = root / "guide.md"
            result = self.run_generator(report_path, guide_path)
            self.assertEqual(result.returncode, 0, result.stderr)
            payload = json.loads(report_path.read_text(encoding="utf-8"))
            payload["references"].append(dict(payload["references"][0]))
            report_path.write_text(
                json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            invalid = self.run_validator(report_path, guide_path)
            self.assertNotEqual(invalid.returncode, 0)
            parsed = json.loads(invalid.stdout)
            self.assertTrue(
                any(
                    issue["code"] == "reference.classification.duplicate"
                    for issue in parsed["issues"]
                ),
                parsed,
            )

    def test_validator_rejects_stale_source_metadata_but_allows_legal_timestamp_change(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            report_path = root / "report.json"
            guide_path = root / "guide.md"
            result = self.run_generator(report_path, guide_path)
            self.assertEqual(result.returncode, 0, result.stderr)

            payload = json.loads(report_path.read_text(encoding="utf-8"))
            payload["generatedAt"] = "2030-01-02T03:04:05Z"
            report_path.write_text(
                json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            self.assertEqual(self.run_validator(report_path, guide_path).returncode, 0)

            for key, expected_code, replacement in (
                ("$schema", "reference.schema_reference.mismatch", "./" + payload["$schema"]),
                ("schemaVersion", "reference.schema_version.mismatch", "gateway-provider-reference-report/v999"),
                ("sourceRevision", "reference.source_revision.mismatch", "0" * 40),
                ("sourceFingerprint", "reference.source_fingerprint.mismatch", "0" * 64),
            ):
                tampered = json.loads(json.dumps(payload))
                tampered[key] = replacement
                report_path.write_text(
                    json.dumps(tampered, indent=2, ensure_ascii=False) + "\n",
                    encoding="utf-8",
                )
                invalid = self.run_validator(report_path, guide_path)
                self.assertNotEqual(invalid.returncode, 0, key)
                parsed = json.loads(invalid.stdout)
                self.assertTrue(
                    any(issue["code"] == expected_code for issue in parsed["issues"]),
                    (key, parsed),
                )

    def test_validator_rejects_tampered_unique_reference_details(self):
        mutations = (
            ("status", lambda item: item.update(status="local_present")),
            ("declarationCount", lambda item: item.update(declarationCount=item["declarationCount"] + 1)),
            (
                "declarations",
                lambda item: item["declarations"][0].update(lineId="tampered-line"),
            ),
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            report_path = root / "report.json"
            guide_path = root / "guide.md"
            result = self.run_generator(report_path, guide_path)
            self.assertEqual(result.returncode, 0, result.stderr)
            payload = json.loads(report_path.read_text(encoding="utf-8"))
            for field, mutate in mutations:
                with self.subTest(field=field):
                    tampered = json.loads(json.dumps(payload))
                    mutate(tampered["uniqueReferences"][0])
                    report_path.write_text(
                        json.dumps(tampered, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8",
                    )
                    invalid = self.run_validator(report_path, guide_path)
                    self.assertNotEqual(invalid.returncode, 0, field)
                    parsed = json.loads(invalid.stdout)
                    self.assertTrue(
                        any(
                            issue["code"] == "reference.unique.mismatch"
                            for issue in parsed["issues"]
                        ),
                        (field, parsed),
                    )

    def test_validator_rejects_non_gateway_reference_paths_with_explicit_issue(self):
        unsafe_paths = (
            "../missing/FIELDS.md",
            "/tmp/provider/FIELDS.md",
            "C:/provider/FIELDS.md",
            "\\\\server\\share\\FIELDS.md",
            "docs/../outside/FIELDS.md",
            "docs",
            "docs/provider-reference-guide.md::$DATA",
            "docs/" + ("a" * 5000),
        )
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            report_path = root / "report.json"
            guide_path = root / "guide.md"
            result = self.run_generator(report_path, guide_path)
            self.assertEqual(result.returncode, 0, result.stderr)
            payload = json.loads(report_path.read_text(encoding="utf-8"))
            for unsafe_path in unsafe_paths:
                with self.subTest(path=unsafe_path):
                    tampered = json.loads(json.dumps(payload))
                    tampered["references"][0]["path"] = unsafe_path
                    report_path.write_text(
                        json.dumps(tampered, indent=2, ensure_ascii=False) + "\n",
                        encoding="utf-8",
                    )
                    invalid = self.run_validator(report_path, guide_path)
                    self.assertNotEqual(invalid.returncode, 0, unsafe_path)
                    parsed = json.loads(invalid.stdout)
                    self.assertTrue(
                        any(
                            issue["code"] == "reference.path.invalid"
                            for issue in parsed["issues"]
                        ),
                        (unsafe_path, parsed),
                    )

    def test_validator_rejects_missing_reference_declaration(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            report_path = root / "report.json"
            guide_path = root / "guide.md"
            result = self.run_generator(report_path, guide_path)
            self.assertEqual(result.returncode, 0, result.stderr)
            payload = json.loads(report_path.read_text(encoding="utf-8"))
            payload["references"][0]["path"] = ""
            report_path.write_text(
                json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
                encoding="utf-8",
            )
            invalid = self.run_validator(report_path, guide_path)
            self.assertNotEqual(invalid.returncode, 0)
            parsed = json.loads(invalid.stdout)
            self.assertTrue(
                any(
                    issue["code"] == "reference.declaration.missing"
                    for issue in parsed["issues"]
                ),
                parsed,
            )

    def test_build_report_rejects_missing_or_noncanonical_manifest_declarations(self):
        invalid_values = (
            ("missing", None),
            ("backslash", r"docs\provider\FIELDS.md"),
            ("leading whitespace", " docs/provider/FIELDS.md"),
            ("trailing whitespace", "docs/provider/FIELDS.md "),
            ("directory path", "docs/provider/"),
            ("existing directory", "docs"),
            ("ntfs alternate stream", "docs/provider/FIELDS.md::$DATA"),
        )
        for name, invalid_value in invalid_values:
            with self.subTest(name=name), tempfile.TemporaryDirectory(
                dir=GATEWAY_ROOT
            ) as temp_dir:
                manifest_root = pathlib.Path(temp_dir)
                manifest = {
                    "id": "isolated-provider",
                    "identity": {},
                    "capabilities": {},
                    "credentials": {
                        "materialKinds": [],
                        "samplePath": "docs/provider/sample.json",
                        "fieldsDocPath": "docs/provider/FIELDS.md",
                    },
                    "docs": {
                        "overviewDocPath": "docs/provider/overview.md",
                        "buildDocPath": "docs/provider/BUILD.md",
                    },
                }
                if invalid_value is None:
                    del manifest["credentials"]["samplePath"]
                else:
                    manifest["credentials"]["samplePath"] = invalid_value
                (manifest_root / "isolated-provider.json").write_text(
                    json.dumps(manifest, ensure_ascii=False),
                    encoding="utf-8",
                )
                with mock.patch.object(
                    gateway_provider_references,
                    "MANIFEST_ROOT",
                    manifest_root,
                ):
                    with self.assertRaisesRegex(
                        ValueError,
                        "isolated-provider/credentials.samplePath",
                    ):
                        gateway_provider_references.build_report(
                            timestamp="2026-07-19T00:00:00Z",
                            report_path=manifest_root / "report.json",
                            guide_path=manifest_root / "guide.md",
                        )

    def test_secret_detector_rejects_common_provider_key_shapes(self):
        values = (
            "ASIA1234567890ABCDEF",
            "sk_live_1234567890abcdef12345678",
            "sk_test_1234567890abcdef12345678",
            "glpat-1234567890abcdefghij",
            "npm_1234567890abcdef1234567890abcdef1234",
        )
        for value in values:
            with self.subTest(value=value):
                issues = gateway_provider_references.find_secret_values({"value": value})
                self.assertEqual(len(issues), 1, value)

        safe_values = (
            "Bearer token",
            "Basic authentication",
            "sk-search",
            "sk-live-provider",
            "npm_provider",
            "glpat-provider",
        )
        for value in safe_values:
            with self.subTest(safe_value=value):
                issues = gateway_provider_references.find_secret_values({"value": value})
                self.assertEqual(issues, [], value)

    def test_generation_is_byte_for_byte_deterministic(self):
        with tempfile.TemporaryDirectory() as temp_dir:
            root = pathlib.Path(temp_dir)
            first_report = root / "first" / "report.json"
            first_guide = root / "first" / "guide.md"
            second_report = root / "second" / "report.json"
            second_guide = root / "second" / "guide.md"
            first = self.run_generator(first_report, first_guide)
            second = self.run_generator(second_report, second_guide)
            self.assertEqual(first.returncode, 0, first.stderr)
            self.assertEqual(second.returncode, 0, second.stderr)
            self.assertEqual(first_report.read_bytes(), second_report.read_bytes())
            self.assertEqual(first_guide.read_bytes(), second_guide.read_bytes())

    def test_checked_in_report_and_guide_are_current_and_valid(self):
        result = self.run_validator(CHECKED_IN_REPORT, CHECKED_IN_GUIDE)
        self.assertEqual(
            result.returncode,
            0,
            msg=f"stdout:\n{result.stdout}\nstderr:\n{result.stderr}",
        )
        payload = json.loads(result.stdout)
        self.assertEqual(payload["status"], "pass")
        self.assertEqual(payload["declaredReferenceCount"], 176)
        self.assertEqual(payload["uniqueReferenceCount"], 166)


if __name__ == "__main__":
    unittest.main()
