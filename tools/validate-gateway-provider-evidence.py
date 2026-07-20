#!/usr/bin/env python3
"""Validate a generated Gateway provider inventory and its evidence records."""

from __future__ import annotations

import argparse
import datetime as dt
import json
import pathlib
import re
import sys
from typing import Any


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[1]
MANIFEST_ROOT = GATEWAY_ROOT / "manifests" / "lines"
DEFAULT_INVENTORY = GATEWAY_ROOT / "docs" / "provider-inventory.json"
SCHEMA_PATH = GATEWAY_ROOT / "docs" / "provider-inventory.schema.json"
SCHEMA_VERSION = "gateway-product-inventory/v1"
EVIDENCE_STATES = {
    "compiled",
    "metadata_only",
    "fixture_passed",
    "live_passed",
    "external_gate",
    "credential_missing",
    "known_unsupported",
}
TIMESTAMP_RE = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$"
)
SECRET_FIELD_RE = re.compile(
    r"(?:api[_-]?key|access[_-]?token|authorization|cookie|jwt|password|"
    r"private[_-]?key|refresh[_-]?token|secret|session[_-]?token)$",
    re.IGNORECASE,
)
SECRET_VALUE_RE = re.compile(
    r"(?:Bearer\s+[A-Za-z0-9._~+/=-]+|Basic\s+[A-Za-z0-9+/=]{8,}|"
    r"sk-[A-Za-z0-9._-]+|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{20,}|"
    r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9._-]+\.[A-Za-z0-9._-]+|"
    r"(?:cookie|set-cookie)\s*[:=]\s*[^;\r\n]+|"
    r"(?:session(?:id|_token)?|refresh_token|api[_-]?key|access[_-]?token|"
    r"authorization|password|secret|aws[_-]?(?:secret[_-]?access[_-]?key|session[_-]?token))\s*[=:]\s*[^;\s,]+|"
    r"ya29\.[A-Za-z0-9._-]+)",
    re.IGNORECASE,
)

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
    "lumalabs-web-reverse-api",
    "mistral-openai-official-model-api",
    "nvidia-openai-official-vendor-api",
    "openrouter-openai-aggregator-api",
    "perplexity-chat-official-vendor-api",
    "perplexity-search-official-vendor-api",
    "producer-web-reverse-api",
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


def issue(code: str, path: str, message: str) -> dict[str, str]:
    return {"code": code, "path": path, "message": message}


def load_json(path: pathlib.Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def valid_timestamp(value: Any) -> bool:
    if not isinstance(value, str) or not TIMESTAMP_RE.match(value):
        return False
    try:
        dt.datetime.fromisoformat(value.replace("Z", "+00:00"))
    except ValueError:
        return False
    return True


def expected_manifest_ids() -> set[str]:
    return set(CANONICAL_PROVIDER_LINE_IDS)


def discovered_manifest_ids() -> set[str]:
    ids: set[str] = set()
    for path in sorted(MANIFEST_ROOT.rglob("*.json")):
        payload = load_json(path)
        line_id = payload.get("id") if isinstance(payload, dict) else None
        if isinstance(line_id, str) and line_id:
            ids.add(line_id)
    return ids


def find_secret_fields(value: Any, path: str = "$") -> list[dict[str, str]]:
    issues: list[dict[str, str]] = []
    if isinstance(value, dict):
        for key, item in value.items():
            child_path = f"{path}.{key}"
            if SECRET_FIELD_RE.search(str(key)) and item != "<redacted>":
                issues.append(
                    issue(
                        "inventory.secret_field.present",
                        child_path,
                        "secret-like fields must not contain material in provider inventory",
                    )
                )
            issues.extend(find_secret_fields(item, child_path))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            issues.extend(find_secret_fields(item, f"{path}[{index}]"))
    elif isinstance(value, str) and SECRET_VALUE_RE.search(value):
        issues.append(
            issue(
                "inventory.secret_value.present",
                path,
                "secret-like material must not be present in provider inventory",
            )
        )
    return issues


def validate_jsonschema(payload: Any, inventory_path: pathlib.Path) -> list[dict[str, str]]:
    try:
        import jsonschema
    except ImportError:
        return [
            issue(
                "inventory.schema.unavailable",
                str(SCHEMA_PATH),
                "jsonschema package is required for strict inventory validation",
            )
        ]

    schema_ref = payload.get("$schema") if isinstance(payload, dict) else None
    if not isinstance(schema_ref, str) or not schema_ref:
        return [
            issue(
                "inventory.schema.reference.missing",
                ".$schema",
                "inventory must declare a relative $schema reference",
            )
        ]
    if re.match(r"^[A-Za-z][A-Za-z0-9+.-]*:", schema_ref) or pathlib.PurePosixPath(
        schema_ref.replace("\\", "/")
    ).is_absolute():
        return [
            issue(
                "inventory.schema.reference.invalid",
                ".$schema",
                "$schema must be a relative path",
            )
        ]
    resolved_schema = (inventory_path.parent / pathlib.Path(schema_ref)).resolve()
    if resolved_schema != SCHEMA_PATH.resolve():
        return [
            issue(
                "inventory.schema.reference.invalid",
                ".$schema",
                "$schema must resolve to Gateway/docs/provider-inventory.schema.json",
            )
        ]
    schema = load_json(resolved_schema)
    validator = jsonschema.Draft202012Validator(schema)
    issues: list[dict[str, str]] = []
    for error in sorted(validator.iter_errors(payload), key=lambda item: str(list(item.path))):
        path = "$"
        for part in error.absolute_path:
            path += f"[{part}]" if isinstance(part, int) else f".{part}"
        issues.append(issue("inventory.schema.invalid", path, error.message))
    return issues


def validate_artifact_path(value: Any, path: str) -> list[dict[str, str]]:
    if not isinstance(value, str) or not value.strip():
        return [issue("evidence.artifact_path.invalid", path, "artifact path must be a non-empty string")]
    candidate = value.replace("\\", "/")
    pure = pathlib.PurePosixPath(candidate)
    if pure.is_absolute() or re.match(r"^[A-Za-z]:", candidate) or ".." in pure.parts:
        return [
            issue(
                "evidence.artifact_path.not_gateway_relative",
                path,
                "artifact path must be relative to Gateway and may not escape it",
            )
        ]
    return []


def validate_inventory(payload: Any) -> list[dict[str, str]]:
    issues: list[dict[str, str]] = []
    if not isinstance(payload, dict):
        return [issue("inventory.root.invalid", "$", "inventory root must be an object")]

    # JSON Schema provides the strict shape contract; the checks below cover
    # cross-record semantics that JSON Schema cannot express conveniently.
    # Callers that need the path-aware $schema resolution use the CLI wrapper.

    if payload.get("schemaVersion") != SCHEMA_VERSION:
        issues.append(
            issue(
                "inventory.schema_version.invalid",
                "$.schemaVersion",
                f"schemaVersion must be {SCHEMA_VERSION}",
            )
        )

    lines = payload.get("lines")
    if not isinstance(lines, list) or not lines:
        issues.append(issue("inventory.lines.invalid", "$.lines", "lines must be a non-empty array"))
        return issues

    ids: list[str] = []
    manifest_paths: list[str] = []
    calculated_counts = {state: 0 for state in sorted(EVIDENCE_STATES)}
    for index, line in enumerate(lines):
        line_path = f"$.lines[{index}]"
        if not isinstance(line, dict):
            issues.append(issue("inventory.line.invalid", line_path, "line must be an object"))
            continue

        line_id = line.get("id")
        manifest_path = line.get("manifestPath")
        if not isinstance(line_id, str) or not line_id:
            issues.append(issue("inventory.line.id.missing", f"{line_path}.id", "line id is required"))
        else:
            ids.append(line_id)
        if not isinstance(manifest_path, str) or not manifest_path:
            issues.append(
                issue(
                    "inventory.line.manifest_path.missing",
                    f"{line_path}.manifestPath",
                    "manifestPath is required",
                )
            )
        else:
            manifest_paths.append(manifest_path)

        evidence = line.get("evidence")
        if not isinstance(evidence, dict):
            issues.append(issue("evidence.invalid", f"{line_path}.evidence", "evidence must be an object"))
            continue
        state = evidence.get("state")
        if state not in EVIDENCE_STATES:
            issues.append(
                issue(
                    "evidence.state.invalid",
                    f"{line_path}.evidence.state",
                    f"evidence state must be one of {sorted(EVIDENCE_STATES)}",
                )
            )
        else:
            calculated_counts[state] += 1

        records = evidence.get("records")
        if not isinstance(records, list) or not records:
            issues.append(
                issue(
                    "evidence.records.missing",
                    f"{line_path}.evidence.records",
                    "at least one evidence record is required",
                )
            )
            continue

        latest_record: dict[str, Any] | None = None
        valid_records: list[dict[str, Any]] = []
        for record_index, record in enumerate(records):
            record_path = f"{line_path}.evidence.records[{record_index}]"
            if not isinstance(record, dict):
                issues.append(issue("evidence.record.invalid", record_path, "evidence record must be an object"))
                continue
            record_state = record.get("state")
            if record_state not in EVIDENCE_STATES:
                issues.append(issue("evidence.record.state.invalid", f"{record_path}.state", "invalid evidence state"))
            timestamp = record.get("timestamp")
            if timestamp is None:
                issues.append(
                    issue(
                        "evidence.timestamp.missing",
                        f"{record_path}.timestamp",
                        "evidence timestamp is required",
                    )
                )
            elif not valid_timestamp(timestamp):
                issues.append(
                    issue(
                        "evidence.timestamp.invalid",
                        f"{record_path}.timestamp",
                        "timestamp must be a valid UTC RFC3339 value ending in Z",
                    )
                )

            classification = record.get("classification")
            if record_state != "compiled" and not isinstance(classification, dict):
                issues.append(
                    issue(
                        "evidence.classification.missing",
                        f"{record_path}.classification",
                        "non-compiled evidence requires a classification",
                    )
                )
            if isinstance(classification, dict):
                for field in ("failureClass", "failureCode", "message"):
                    if not isinstance(classification.get(field), str) or not classification[field].strip():
                        issues.append(
                            issue(
                                "evidence.classification.invalid",
                                f"{record_path}.classification.{field}",
                                f"classification {field} is required",
                            )
                            )
            mode = record.get("mode")
            if record_state == "live_passed" and mode != "live":
                issues.append(
                    issue(
                        "evidence.mode.invalid",
                        f"{record_path}.mode",
                        "live_passed evidence requires live mode",
                    )
                )
            if record_state == "metadata_only" and mode != "offline":
                issues.append(
                    issue(
                        "evidence.mode.invalid",
                        f"{record_path}.mode",
                        "metadata_only evidence requires offline mode",
                    )
                )
            if record_state == "fixture_passed" and mode not in {"offline", "fixture"}:
                issues.append(
                    issue(
                        "evidence.mode.invalid",
                        f"{record_path}.mode",
                        "fixture_passed evidence requires offline or fixture mode",
                    )
                )
            if record_state == "live_passed":
                route_proof = record.get("routeProof")
                observed_provider = record.get("observedProvider")
                if not isinstance(route_proof, dict) or not route_proof:
                    issues.append(
                        issue(
                            "evidence.route_proof.missing",
                            f"{record_path}.routeProof",
                            "live_passed evidence requires route proof",
                        )
                    )
                if not isinstance(observed_provider, dict) or not observed_provider:
                    issues.append(
                        issue(
                            "evidence.observed_provider_proof.missing",
                            f"{record_path}.observedProvider",
                            "live_passed evidence requires observed provider proof",
                        )
                    )
                for proof_name, proof in (
                    ("routeProof", route_proof),
                    ("observedProvider", observed_provider),
                ):
                    if isinstance(proof, dict):
                        proof_line = proof.get("providerLine") or proof.get("provider_line")
                        if proof_line != line_id:
                            issues.append(
                                issue(
                                    "evidence.route_proof.mismatch",
                                    f"{record_path}.{proof_name}",
                                    "provider proof must identify the containing provider line",
                                )
                            )
            artifact_paths = record.get("artifactPaths")
            if artifact_paths is not None:
                if not isinstance(artifact_paths, list):
                    issues.append(
                        issue(
                            "evidence.artifact_path.invalid",
                            f"{record_path}.artifactPaths",
                            "artifactPaths must be an array",
                        )
                    )
                else:
                    for artifact_index, artifact_path in enumerate(artifact_paths):
                        issues.extend(
                            validate_artifact_path(
                                artifact_path,
                                f"{record_path}.artifactPaths[{artifact_index}]",
                            )
                        )
            for proof_name in ("routeProof", "observedProvider"):
                proof = record.get(proof_name)
                if isinstance(proof, dict) and "artifactPath" in proof:
                    issues.extend(
                        validate_artifact_path(
                            proof["artifactPath"],
                            f"{record_path}.{proof_name}.artifactPath",
                        )
                    )
            if valid_timestamp(timestamp) and record_state in EVIDENCE_STATES:
                valid_records.append(record)

        if valid_records:
            latest_record = max(
                valid_records,
                key=lambda record: (record["timestamp"], record["state"]),
            )
        if latest_record is not None and latest_record.get("state") != state:
            issues.append(
                issue(
                    "evidence.state.not_latest",
                    f"{line_path}.evidence.state",
                    "evidence state must match the latest record",
                )
            )

    if ids != sorted(ids):
        issues.append(issue("inventory.lines.not_sorted", "$.lines", "line ids must be sorted"))
    if len(ids) != len(set(ids)):
        issues.append(issue("inventory.line.id.duplicate", "$.lines", "line ids must be unique"))
    if len(manifest_paths) != len(set(manifest_paths)):
        issues.append(
            issue(
                "inventory.manifest_path.duplicate",
                "$.lines",
                "manifest paths must be unique",
            )
        )

    expected_ids = expected_manifest_ids()
    actual_ids = set(ids)
    if actual_ids != expected_ids:
        missing = sorted(expected_ids - actual_ids)
        extra = sorted(actual_ids - expected_ids)
        issues.append(
            issue(
                "inventory.manifest_coverage.mismatch",
                "$.lines",
                f"manifest coverage mismatch; missing={missing}, extra={extra}",
            )
        )

    source_manifest_ids = discovered_manifest_ids()
    if source_manifest_ids != expected_ids:
        missing = sorted(expected_ids - source_manifest_ids)
        extra = sorted(source_manifest_ids - expected_ids)
        issues.append(
            issue(
                "inventory.manifest_source_coverage.mismatch",
                "$.sources.manifestRoot",
                f"canonical manifest source coverage mismatch; missing={missing}, extra={extra}",
            )
        )

    summary_counts = payload.get("evidenceSummary", {}).get("counts")
    if not isinstance(summary_counts, dict) or any(
        summary_counts.get(state) != calculated_counts[state]
        for state in calculated_counts
    ):
        issues.append(
            issue(
                "inventory.evidence_counts.mismatch",
                "$.evidenceSummary.counts",
                "evidence summary counts must match line states",
            )
        )

    issues.extend(find_secret_fields(payload))
    return issues


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--inventory", type=pathlib.Path, default=DEFAULT_INVENTORY)
    parser.add_argument("--as-json", action="store_true")
    args = parser.parse_args(argv)

    try:
        resolved_inventory = args.inventory.resolve()
        payload = load_json(resolved_inventory)
        issues = validate_jsonschema(payload, resolved_inventory)
        issues.extend(validate_inventory(payload))
    except (OSError, json.JSONDecodeError) as exc:
        issues = [issue("inventory.read.failed", str(args.inventory), str(exc))]

    report = {
        "status": "pass" if not issues else "fail",
        "inventory": args.inventory.resolve().as_posix(),
        "issueCount": len(issues),
        "issues": issues,
    }
    if args.as_json:
        print(json.dumps(report, ensure_ascii=False, sort_keys=True))
    elif issues:
        for entry in issues:
            print(
                f"[provider-evidence] {entry['code']} {entry['path']}: {entry['message']}",
                file=sys.stderr,
            )
    else:
        print(f"[provider-evidence] status=pass inventory={report['inventory']}")
    return 0 if not issues else 1


if __name__ == "__main__":
    raise SystemExit(main())
