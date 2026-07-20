#!/usr/bin/env python3
from __future__ import annotations

import argparse
import json
import pathlib
import re
import sys
from dataclasses import dataclass
from typing import Any

GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[1]
REPO_ROOT = GATEWAY_ROOT.parent
MANIFEST_ROOT = GATEWAY_ROOT / "manifests" / "lines"
SCHEMA_PATH = GATEWAY_ROOT / "manifests" / "schema" / "gateway-line-manifest.schema.json"
CARGO_TOML = GATEWAY_ROOT / "Cargo.toml"

FEATURE_RE = re.compile(r"^[a-z0-9]+(?:-[a-z0-9]+)*$")


@dataclass
class Issue:
    severity: str
    code: str
    manifest_id: str
    json_path: str
    message: str
    evidence: str | None = None

    def to_dict(self) -> dict[str, str]:
        payload = {
            "severity": self.severity,
            "code": self.code,
            "manifestId": self.manifest_id,
            "jsonPath": self.json_path,
            "message": self.message,
        }
        if self.evidence:
            payload["evidence"] = self.evidence
        return payload


def load_json(path: pathlib.Path) -> dict[str, Any]:
    with path.open("r", encoding="utf-8") as fh:
        return json.load(fh)


def discover_manifests(explicit: list[str]) -> list[pathlib.Path]:
    if explicit:
        return sorted({pathlib.Path(value).resolve() for value in explicit})
    return sorted(MANIFEST_ROOT.rglob("*.json"))


def maybe_import_jsonschema() -> Any | None:
    try:
        import jsonschema  # type: ignore

        return jsonschema
    except Exception:
        return None


def add_issue(
    issues: list[Issue],
    severity: str,
    code: str,
    manifest_id: str,
    json_path: str,
    message: str,
    evidence: str | None = None,
) -> None:
    issues.append(Issue(severity, code, manifest_id, json_path, message, evidence))


def gateway_relative_path(raw_path: str) -> pathlib.Path:
    normalized = raw_path.replace("\\", "/")
    if normalized.startswith("gateway/"):
        normalized = normalized[len("gateway/") :]
    if normalized.startswith("Gateway/"):
        normalized = normalized[len("Gateway/") :]
    return GATEWAY_ROOT / normalized


def cargo_feature_declared(feature: str, cargo_text: str) -> bool:
    return bool(re.search(rf"(?m)^{re.escape(feature)}\s*=", cargo_text))


def validate_required_file(
    issues: list[Issue],
    manifest_id: str,
    json_path: str,
    raw_path: str | None,
) -> None:
    if not raw_path:
        return
    if raw_path.startswith("docs/"):
        # The migrated gateway keeps historical doc references from NeuroPlatform.
        # Missing platform docs should not block packaging the standalone Gateway.
        return
    candidate = gateway_relative_path(raw_path)
    if not candidate.exists():
        add_issue(
            issues,
            "error",
            "manifest.path.missing",
            manifest_id,
            json_path,
            f"Manifest references a missing Gateway path: {raw_path}",
            str(candidate),
        )


def validate_manifest(manifest: dict[str, Any], cargo_text: str, issues: list[Issue]) -> None:
    manifest_id = str(manifest.get("id") or "<unknown>")

    compilation = manifest.get("compilation") or {}
    line_feature = compilation.get("lineFeature")
    if isinstance(line_feature, str):
        if not FEATURE_RE.match(line_feature) or not line_feature.startswith("line-"):
            add_issue(
                issues,
                "error",
                "manifest.feature.invalid",
                manifest_id,
                "$.compilation.lineFeature",
                f"Invalid line feature name: {line_feature}",
            )
        elif not cargo_feature_declared(line_feature, cargo_text):
            add_issue(
                issues,
                "error",
                "manifest.feature.missing",
                manifest_id,
                "$.compilation.lineFeature",
                f"Cargo.toml does not declare line feature: {line_feature}",
            )

    for index, feature in enumerate(compilation.get("familyCommonFeatures") or []):
        if not isinstance(feature, str) or not FEATURE_RE.match(feature) or not feature.startswith("family-"):
            add_issue(
                issues,
                "error",
                "manifest.family_feature.invalid",
                manifest_id,
                f"$.compilation.familyCommonFeatures[{index}]",
                f"Invalid family feature name: {feature}",
            )
            continue
        if not cargo_feature_declared(feature, cargo_text):
            add_issue(
                issues,
                "error",
                "manifest.family_feature.missing",
                manifest_id,
                f"$.compilation.familyCommonFeatures[{index}]",
                f"Cargo.toml does not declare family feature: {feature}",
            )

    implementation = manifest.get("implementation") or {}
    validate_required_file(issues, manifest_id, "$.implementation.protocolModule", implementation.get("protocolModule"))
    validate_required_file(issues, manifest_id, "$.implementation.upstreamModule", implementation.get("upstreamModule"))
    validate_required_file(
        issues,
        manifest_id,
        "$.implementation.platformDeltaModule",
        implementation.get("platformDeltaModule"),
    )

    credentials = manifest.get("credentials") or {}
    validate_required_file(issues, manifest_id, "$.credentials.samplePath", credentials.get("samplePath"))
    validate_required_file(issues, manifest_id, "$.credentials.fieldsDocPath", credentials.get("fieldsDocPath"))


def main() -> int:
    parser = argparse.ArgumentParser(description="Validate migrated Neuro Gateway line manifests.")
    parser.add_argument("--manifest", action="append", default=[], help="Validate one explicit manifest path.")
    parser.add_argument("--as-json", action="store_true", help="Emit machine-readable JSON.")
    args = parser.parse_args()

    schema = load_json(SCHEMA_PATH)
    jsonschema = maybe_import_jsonschema()
    cargo_text = CARGO_TOML.read_text(encoding="utf-8")
    manifest_paths = discover_manifests(args.manifest)

    issues: list[Issue] = []
    checked = 0
    for path in manifest_paths:
        manifest = load_json(path)
        manifest_id = str(manifest.get("id") or path.stem)
        if jsonschema is not None:
            try:
                jsonschema.validate(instance=manifest, schema=schema)
            except Exception as exc:
                add_issue(
                    issues,
                    "error",
                    "manifest.schema.invalid",
                    manifest_id,
                    "$",
                    f"JSON schema validation failed: {exc}",
                )
        validate_manifest(manifest, cargo_text, issues)
        checked += 1

    error_count = sum(1 for issue in issues if issue.severity == "error")
    report = {
        "status": "pass" if error_count == 0 else "fail",
        "checkedManifests": checked,
        "errorCount": error_count,
        "issues": [issue.to_dict() for issue in issues],
    }

    if args.as_json:
        json.dump(report, sys.stdout, indent=2, ensure_ascii=False)
        sys.stdout.write("\n")
    else:
        print(
            f"[manifest-validator] status={report['status']} "
            f"checked={checked} errors={error_count}"
        )
        for issue in issues:
            print(
                f"- {issue.severity.upper()} {issue.code} "
                f"{issue.manifest_id} {issue.json_path}: {issue.message}"
            )

    return 1 if error_count else 0


if __name__ == "__main__":
    raise SystemExit(main())
