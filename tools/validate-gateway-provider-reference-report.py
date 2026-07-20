#!/usr/bin/env python3
"""Validate Gateway's provider-reference report and safe local guide."""

from __future__ import annotations

import argparse
import json
import pathlib
import sys
from typing import Any

import gateway_provider_references as references


def issue(code: str, path: str, message: str) -> dict[str, str]:
    return {"code": code, "path": path, "message": message}


def validate_jsonschema(payload: Any, report_path: pathlib.Path) -> list[dict[str, str]]:
    try:
        import jsonschema
    except ImportError:
        return [
            issue(
                "reference.schema.unavailable",
                str(references.REPORT_SCHEMA),
                "jsonschema package is required for strict report validation",
            )
        ]
    schema_ref = payload.get("$schema") if isinstance(payload, dict) else None
    if not isinstance(schema_ref, str) or not schema_ref:
        return [issue("reference.schema.missing", "$.$schema", "relative schema reference is required")]
    schema_path = (report_path.parent / pathlib.Path(schema_ref)).resolve()
    if schema_path != references.REPORT_SCHEMA.resolve():
        return [
            issue(
                "reference.schema.invalid",
                "$.$schema",
                "report schema must resolve to the checked-in Gateway schema",
            )
        ]
    try:
        schema = references.load_json(schema_path)
    except (OSError, json.JSONDecodeError) as exc:
        return [issue("reference.schema.read_failed", "$.$schema", str(exc))]
    errors = sorted(
        jsonschema.Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: str(list(error.path)),
    )
    return [
        issue(
            "reference.schema.invalid",
            "$" + "".join(
                f"[{part}]" if isinstance(part, int) else f".{part}"
                for part in error.absolute_path
            ),
            error.message,
        )
        for error in errors
    ]


def validate_semantics(
    payload: Any,
    report_path: pathlib.Path,
    guide_path: pathlib.Path,
) -> list[dict[str, str]]:
    issues: list[dict[str, str]] = []
    if not isinstance(payload, dict):
        return [issue("reference.root.invalid", "$", "report root must be an object")]

    try:
        expected = references.build_report(
            timestamp=str(payload.get("generatedAt") or "1970-01-01T00:00:00Z"),
            report_path=report_path,
            guide_path=guide_path,
        )
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError) as exc:
        return [
            issue(
                "reference.source.invalid",
                "manifests/lines",
                f"cannot rebuild report from provider manifests: {exc}",
            )
        ]
    for key, code in (
        ("$schema", "reference.schema_reference.mismatch"),
        ("schemaVersion", "reference.schema_version.mismatch"),
        ("sourceRevision", "reference.source_revision.mismatch"),
        ("sourceFingerprint", "reference.source_fingerprint.mismatch"),
    ):
        if payload.get(key) != expected.get(key):
            issues.append(
                issue(
                    code,
                    f"$.{key}",
                    f"{key} does not match the current generator source",
                )
            )
    expected_refs = expected["references"]
    actual_refs = payload.get("references")
    if not isinstance(actual_refs, list):
        issues.append(issue("reference.classification.missing", "$.references", "references must be an array"))
        actual_refs = []

    actual_keys = [
        (item.get("lineId"), item.get("field"))
        for item in actual_refs
        if isinstance(item, dict)
    ]
    seen: set[tuple[Any, Any]] = set()
    for index, key in enumerate(actual_keys):
        if key in seen:
            issues.append(
                issue(
                    "reference.classification.duplicate",
                    f"$.references[{index}]",
                    f"reference {key[0]!r}/{key[1]!r} is classified more than once",
                )
            )
        seen.add(key)
    expected_keys = [(item["lineId"], item["field"]) for item in expected_refs]
    if actual_keys != expected_keys:
        expected_set = set(expected_keys)
        actual_set = set(actual_keys)
        for missing in sorted(expected_set - actual_set):
            issues.append(
                issue(
                    "reference.classification.missing",
                    "$.references",
                    f"missing classification for {missing[0]}/{missing[1]}",
                )
            )
        for extra in sorted(actual_set - expected_set):
            issues.append(
                issue(
                    "reference.classification.unexpected",
                    "$.references",
                    f"unexpected classification for {extra[0]}/{extra[1]}",
                )
            )
        if not (expected_set - actual_set or actual_set - expected_set):
            issues.append(
                issue(
                    "reference.classification.order",
                    "$.references",
                    "references must use deterministic line/field order",
                )
            )

    for index, item in enumerate(actual_refs):
        if not isinstance(item, dict):
            issues.append(issue("reference.item.invalid", f"$.references[{index}]", "reference must be an object"))
            continue
        raw_path = item.get("path")
        path_issue = references.reference_path_issue(raw_path)
        if not isinstance(raw_path, str) or not raw_path:
            issues.append(
                issue(
                    "reference.declaration.missing",
                    f"$.references[{index}].path",
                    "reference declaration path is required",
                )
            )
        elif path_issue:
            issues.append(
                issue(
                    "reference.path.invalid",
                    f"$.references[{index}].path",
                    f"reference path is not a canonical Gateway-relative path: {path_issue}",
                )
            )
        if item.get("status") not in references.STATUS_SET:
            issues.append(
                issue(
                    "reference.status.invalid",
                    f"$.references[{index}].status",
                    "status must be local_present or external_legacy_reference",
                )
            )
        if index < len(expected_refs):
            expected_item = expected_refs[index]
            for key in ("lineId", "manifestPath", "field", "path", "status", "resolution"):
                if item.get(key) != expected_item.get(key):
                    issues.append(
                        issue(
                            "reference.classification.mismatch",
                            f"$.references[{index}].{key}",
                            "classification does not match the manifest and Gateway filesystem",
                        )
                    )

    expected_unique = expected["uniqueReferences"]
    actual_unique = payload.get("uniqueReferences")
    if not isinstance(actual_unique, list):
        issues.append(issue("reference.unique.missing", "$.uniqueReferences", "uniqueReferences must be an array"))
        actual_unique = []
    for index, item in enumerate(actual_unique):
        if not isinstance(item, dict):
            continue
        path_issue = references.reference_path_issue(item.get("path"))
        if path_issue:
            issues.append(
                issue(
                    "reference.path.invalid",
                    f"$.uniqueReferences[{index}].path",
                    f"reference path is not a canonical Gateway-relative path: {path_issue}",
                )
            )
    if actual_unique != expected_unique:
        issues.append(
            issue(
                "reference.unique.mismatch",
                "$.uniqueReferences",
                "unique reference structures do not match declarations",
            )
        )

    expected_lines = expected["lines"]
    actual_lines = payload.get("lines")
    if not isinstance(actual_lines, list):
        issues.append(issue("reference.lines.missing", "$.lines", "lines must be an array"))
        actual_lines = []
    if [item.get("id") for item in actual_lines if isinstance(item, dict)] != [
        item["id"] for item in expected_lines
    ]:
        issues.append(issue("reference.lines.mismatch", "$.lines", "line catalog does not match manifests"))
    for index, expected_line in enumerate(expected_lines):
        if index >= len(actual_lines) or not isinstance(actual_lines[index], dict):
            continue
        actual_line = actual_lines[index]
        if actual_line.get("legacyReferences") != expected_line["legacyReferences"]:
            issues.append(
                issue(
                    "reference.line.references.mismatch",
                    f"$.lines[{index}].legacyReferences",
                    "line references do not match global classifications",
                )
            )
        legacy_references = actual_line.get("legacyReferences")
        if isinstance(legacy_references, list):
            for reference_index, reference in enumerate(legacy_references):
                if not isinstance(reference, dict):
                    continue
                path_issue = references.reference_path_issue(reference.get("path"))
                if path_issue:
                    issues.append(
                        issue(
                            "reference.path.invalid",
                            f"$.lines[{index}].legacyReferences[{reference_index}].path",
                            f"reference path is not a canonical Gateway-relative path: {path_issue}",
                        )
                    )
        for key in ("identity", "capabilities", "materialKinds", "manifestPath", "resolution"):
            if actual_line.get(key) != expected_line.get(key):
                issues.append(
                    issue(
                        "reference.line.metadata.mismatch",
                        f"$.lines[{index}].{key}",
                        "line metadata does not match its manifest",
                    )
                )

    summary = payload.get("summary")
    if summary != expected["summary"]:
        issues.append(issue("reference.summary.mismatch", "$.summary", "summary counts do not match classifications"))
    if payload.get("guidePath") != references.display_path(guide_path):
        issues.append(issue("reference.guide_path.mismatch", "$.guidePath", "guidePath does not match the supplied guide"))

    try:
        guide_text = guide_path.read_text(encoding="utf-8")
    except OSError as exc:
        issues.append(issue("reference.guide.read_failed", str(guide_path), str(exc)))
        guide_text = ""
    if guide_text != references.render_guide(expected):
        issues.append(issue("reference.guide.stale", str(guide_path), "guide is not the deterministic rendering of the report"))
    if references.SECRET_VALUE_RE.search(guide_text):
        issues.append(issue("reference.guide.secret_value.present", str(guide_path), "guide contains secret-like material"))
    for secret in references.find_secret_values(payload):
        issues.append(issue("reference.report.secret_value.present", secret["path"], "report contains secret-like material"))
    return issues


def validate_report(report_path: pathlib.Path, guide_path: pathlib.Path) -> list[dict[str, str]]:
    try:
        payload = references.load_json(report_path)
    except (OSError, json.JSONDecodeError) as exc:
        return [issue("reference.read.failed", str(report_path), str(exc))]
    issues = validate_jsonschema(payload, report_path)
    issues.extend(validate_semantics(payload, report_path, guide_path))
    return issues


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=pathlib.Path, default=references.DEFAULT_REPORT)
    parser.add_argument("--guide", type=pathlib.Path, default=references.DEFAULT_GUIDE)
    parser.add_argument("--as-json", action="store_true")
    args = parser.parse_args(argv)
    report_path = args.report.resolve()
    guide_path = args.guide.resolve()
    issues = validate_report(report_path, guide_path)
    result = {
        "status": "pass" if not issues else "fail",
        "report": report_path.as_posix(),
        "guide": guide_path.as_posix(),
        "issueCount": len(issues),
        "issues": issues,
    }
    if not issues:
        try:
            summary = references.load_json(report_path).get("summary", {})
            for key in (
                "lineCount",
                "declaredReferenceCount",
                "uniqueReferenceCount",
                "localPresentReferenceCount",
                "externalLegacyReferenceCount",
                "unclassifiedReferenceCount",
            ):
                result[key] = summary.get(key)
        except (OSError, json.JSONDecodeError):
            pass
    if args.as_json:
        print(json.dumps(result, ensure_ascii=False, sort_keys=True))
    else:
        if issues:
            for item in issues:
                print(f"[provider-reference-report] {item['code']}: {item['message']}", file=sys.stderr)
        else:
            print(f"[provider-reference-report] status=pass report={report_path.as_posix()}")
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
