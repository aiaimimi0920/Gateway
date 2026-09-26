#!/usr/bin/env python3
"""Generate the deterministic, secret-free Gateway provider inventory.

The generator intentionally uses only Python's standard library.  It reads
line manifests and source metadata, but it never loads credential samples or
environment credential values.  Live/fixture evidence can be merged only when
an operator explicitly supplies an evidence file or directory.
"""

from __future__ import annotations

import argparse
import json
import os
import pathlib
import sys
from collections import defaultdict
from typing import Any, Iterable

from provider_inventory.contracts import (
    CANONICAL_PROVIDER_LINE_IDS,
    CARGO_PATH,
    DEFAULT_OUTPUT,
    EVIDENCE_STATES,
    IMPLEMENTATION_LINES_PATH,
    MANIFEST_ROOT,
    PROTOCOL_REGISTRY_PATH,
    PROTOCOL_RESOLUTION_PATH,
    SCHEMA_PATH,
    InventoryError,
)
from provider_inventory.metadata import (
    _normalize_timestamp,
    _wire_families_for_line,
    deterministic_timestamp,
    parse_cargo_features,
    parse_implementation_metadata,
    parse_route_metadata,
    read_implementation_source,
    read_protocol_resolution_source,
    source_revision,
)
from provider_inventory.redaction import (
    _safe_proof_object,
    normalize_gateway_artifact_paths,
    posix_relative,
    sanitize,
)


def load_json(path: pathlib.Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def schema_reference_for_output(output_path: pathlib.Path) -> str:
    return pathlib.PurePath(
        os.path.relpath(SCHEMA_PATH.resolve(), output_path.resolve().parent)
    ).as_posix()


def discover_manifests() -> list[pathlib.Path]:
    return sorted(MANIFEST_ROOT.rglob("*.json"), key=lambda path: path.as_posix())


def _normalise_explicit_record(record: dict[str, Any], default_timestamp: str) -> dict[str, Any]:
    line_id = record.get("lineId") or record.get("providerLine") or record.get("provider_line")
    if not isinstance(line_id, str) or not line_id.strip():
        raise InventoryError("Evidence record is missing lineId/providerLine")
    state = record.get("state")
    if not isinstance(state, str) or state not in EVIDENCE_STATES:
        raise InventoryError(f"Evidence record for {line_id} has invalid state: {state}")
    timestamp = record.get("timestamp") or record.get("observedAt") or default_timestamp
    if not isinstance(timestamp, str):
        raise InventoryError(f"Evidence record for {line_id} has invalid timestamp")
    timestamp = _normalize_timestamp(timestamp)
    mode = record.get("mode")
    if not isinstance(mode, str) or not mode.strip():
        mode = "live" if state == "live_passed" else "offline"
    mode = mode.strip().lower()
    allowed_modes = {"offline", "fixture", "live", "external"}
    if mode not in allowed_modes:
        raise InventoryError(f"Evidence record for {line_id} has invalid mode: {mode}")
    if state == "live_passed" and mode != "live":
        raise InventoryError(
            f"Evidence record for {line_id} live_passed requires live mode"
        )
    if state == "metadata_only" and mode != "offline":
        raise InventoryError(
            f"Evidence record for {line_id} metadata_only requires offline mode"
        )
    if state == "fixture_passed" and mode not in {"offline", "fixture"}:
        raise InventoryError(
            f"Evidence record for {line_id} fixture_passed requires offline or fixture mode"
        )
    classification = record.get("classification")
    if not isinstance(classification, dict):
        classification = {
            "failureClass": record.get("failureClass") or record.get("failure_classification"),
            "failureCode": record.get("failureCode") or "external_evidence",
            "message": record.get("message") or "Evidence supplied by an operator",
        }
    classification = sanitize(classification)
    failure_class = classification.get("failureClass")
    if not isinstance(failure_class, str) or not failure_class:
        raise InventoryError(
            f"Evidence record for {line_id} requires classified failure data"
        )
    result: dict[str, Any] = {
        "state": state,
        "timestamp": timestamp,
        "observedAt": timestamp,
        "mode": mode,
        "classification": classification,
    }
    for source_key, target_key in (
        ("credentialSource", "credentialSource"),
        ("credential_source", "credentialSource"),
        ("endpointFamily", "endpointFamily"),
        ("endpoint_family", "endpointFamily"),
        ("artifactPaths", "artifactPaths"),
        ("artifact_paths", "artifactPaths"),
    ):
        if source_key in record:
            if target_key == "artifactPaths":
                result[target_key] = normalize_gateway_artifact_paths(
                    record[source_key], line_id
                )
            else:
                result[target_key] = sanitize(record[source_key], target_key)
    for source_key, target_key in (
        ("routeProof", "routeProof"),
        ("route_proof", "routeProof"),
        ("observedProvider", "observedProvider"),
        ("observed_provider", "observedProvider"),
    ):
        if source_key in record:
            proof = _safe_proof_object(
                record[source_key], line_id, target_key
            )
            if "provider_line" in proof and "providerLine" not in proof:
                proof["providerLine"] = proof.pop("provider_line")
            if "http_status" in proof and "httpStatus" not in proof:
                proof["httpStatus"] = proof.pop("http_status")
            if "request_id" in proof and "requestId" not in proof:
                proof["requestId"] = proof.pop("request_id")
            if "artifact_path" in proof and "artifactPath" not in proof:
                proof["artifactPath"] = proof.pop("artifact_path")
            if "artifactPath" in proof:
                proof["artifactPath"] = normalize_gateway_artifact_paths(
                    [proof["artifactPath"]], line_id
                )[0]
            result[target_key] = {
                key: proof[key]
                for key in (
                    "providerLine",
                    "source",
                    "endpoint",
                    "method",
                    "httpStatus",
                    "requestId",
                    "artifactPath",
                )
                if key in proof
            }
    if state == "live_passed":
        if "routeProof" not in result or "observedProvider" not in result:
            raise InventoryError(
                f"Evidence record for {line_id} live_passed requires route proof and observed provider proof"
            )
    if "failure" in record:
        result["failure"] = sanitize(record["failure"])
    return line_id.strip(), result


def load_explicit_evidence(
    evidence_files: Iterable[pathlib.Path], default_timestamp: str
) -> dict[str, list[dict[str, Any]]]:
    records_by_line: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for path in sorted(evidence_files, key=lambda item: item.as_posix()):
        payload = load_json(path)
        if isinstance(payload, dict) and isinstance(payload.get("records"), list):
            records = payload["records"]
        elif isinstance(payload, list):
            records = payload
        else:
            records = [payload]
        for raw_record in records:
            if not isinstance(raw_record, dict):
                raise InventoryError(f"Evidence file contains a non-object record: {path}")
            line_id, record = _normalise_explicit_record(raw_record, default_timestamp)
            records_by_line[line_id].append(record)
    return records_by_line


def discover_evidence_files(paths: list[str], directory: str | None) -> list[pathlib.Path]:
    files = [pathlib.Path(value).resolve() for value in paths]
    if directory:
        root = pathlib.Path(directory).resolve()
        if not root.exists():
            raise InventoryError(f"Evidence directory does not exist: {root}")
        files.extend(sorted(root.rglob("*.json"), key=lambda item: item.as_posix()))
    return sorted(set(files), key=lambda item: item.as_posix())


def make_default_evidence(timestamp: str) -> dict[str, Any]:
    return {
        "state": "metadata_only",
        "timestamp": timestamp,
        "observedAt": timestamp,
        "mode": "offline",
        "classification": {
            "failureClass": "none",
            "failureCode": "offline_metadata_only",
            "message": "Manifest, Cargo feature, implementation metadata, and source paths are present; Cargo execution was not observed by the inventory generator.",
        },
    }


def build_inventory(
    *,
    timestamp: str,
    explicit_evidence: dict[str, list[dict[str, Any]]],
    schema_reference: str | None = None,
) -> dict[str, Any]:
    cargo_text = CARGO_PATH.read_text(encoding="utf-8")
    cargo_features = parse_cargo_features(cargo_text)
    implementation_text = read_implementation_source()
    implementation = parse_implementation_metadata(implementation_text)
    route = parse_route_metadata(
        read_protocol_resolution_source(),
        PROTOCOL_REGISTRY_PATH.read_text(encoding="utf-8"),
    )

    manifests = []
    seen_ids: set[str] = set()
    for manifest_path in discover_manifests():
        manifest = load_json(manifest_path)
        line_id = str(manifest.get("id") or "")
        if not line_id:
            raise InventoryError(f"Manifest has no id: {manifest_path}")
        if line_id in seen_ids:
            raise InventoryError(f"Duplicate manifest id: {line_id}")
        seen_ids.add(line_id)
        manifests.append((manifest_path, manifest))

    expected_ids = set(CANONICAL_PROVIDER_LINE_IDS)
    missing_ids = sorted(expected_ids - seen_ids)
    unexpected_ids = sorted(seen_ids - expected_ids)
    if missing_ids:
        raise InventoryError(
            "Manifest discovery is missing canonical provider lines: "
            + ", ".join(missing_ids)
        )
    if unexpected_ids:
        raise InventoryError(
            "Manifest discovery found provider lines outside the canonical contract: "
            + ", ".join(unexpected_ids)
        )

    lines: list[dict[str, Any]] = []
    for manifest_path, manifest in manifests:
        identity = manifest.get("identity") or {}
        compilation = dict(manifest.get("compilation") or {})
        line_feature = compilation.get("lineFeature")
        family_features = [
            str(value)
            for value in (compilation.get("familyCommonFeatures") or [])
            if isinstance(value, str)
        ]
        declared_dependencies = list(cargo_features.get(str(line_feature), []))
        family_declared = {
            family: family in cargo_features for family in family_features
        }
        default_features = set(cargo_features.get("default", []))

        profile = str(identity.get("protocolProfile") or "")
        adapter = str(identity.get("adapter") or "")
        profile_variant = implementation["profileAliases"].get(profile)
        adapter_variant = implementation["adapterAliases"].get(adapter)
        variant = profile_variant or adapter_variant
        canonical_profile = (
            implementation["canonicalByVariant"].get(variant)
            if variant
            else None
        ) or profile
        feature_name = implementation["featureByVariant"].get(variant) if variant else None
        feature_mapped = bool(
            feature_name == line_feature
            or (isinstance(line_feature, str) and line_feature in implementation_text)
        )
        profile_mapped = bool(
            profile in implementation["profileAliases"]
            or canonical_profile == profile
            or (isinstance(profile, str) and f'"{profile}"' in implementation_text)
        )
        wire_families = _wire_families_for_line(manifest, route)

        explicit_records = explicit_evidence.get(str(manifest.get("id")), [])
        records = list(explicit_records) if explicit_records else [make_default_evidence(timestamp)]
        records.sort(key=lambda record: (record["timestamp"], record["state"], json.dumps(record, sort_keys=True)))
        current_state = records[-1]["state"]

        generated_line = {
            "id": manifest["id"],
            "manifestPath": posix_relative(manifest_path),
            "manifest": sanitize(manifest),
            "identity": sanitize(identity),
            "capabilities": sanitize(manifest.get("capabilities") or {}),
            "credentials": sanitize(manifest.get("credentials") or {}),
            "docs": sanitize(manifest.get("docs") or {}),
            "verification": sanitize(manifest.get("verification") or {}),
            "compilation": {
                **sanitize(compilation),
                "cargo": {
                    "lineFeatureDeclared": isinstance(line_feature, str)
                    and line_feature in cargo_features,
                    "declaredDependencies": sorted(dict.fromkeys(declared_dependencies)),
                    "familyFeaturesDeclared": {
                        key: family_declared[key] for key in sorted(family_declared)
                    },
                    "defaultEnabled": isinstance(line_feature, str)
                    and line_feature in default_features,
                },
            },
            "implementation": sanitize(manifest.get("implementation") or {}),
            "implementationLineMetadata": {
                "enumVariant": variant,
                "canonicalProtocolProfile": canonical_profile,
                "featureName": feature_name,
                "protocolProfileMapped": profile_mapped,
                "adapterMapped": adapter in implementation["adapterAliases"],
                "featureMapped": feature_mapped,
                "sourcePath": posix_relative(IMPLEMENTATION_LINES_PATH),
            },
            "routeMetadata": {
                "wireProtocolFamilies": sorted(dict.fromkeys(wire_families)),
                "sourcePaths": [
                    posix_relative(PROTOCOL_RESOLUTION_PATH),
                    posix_relative(PROTOCOL_REGISTRY_PATH),
                ],
            },
            "evidence": {"state": current_state, "records": records},
        }
        lines.append(generated_line)

    lines.sort(key=lambda line: line["id"])
    counts = {state: 0 for state in EVIDENCE_STATES}
    for line in lines:
        counts[line["evidence"]["state"]] += 1
    explicit_states = {
        record["state"]
        for records in explicit_evidence.values()
        for record in records
    }
    mode = "offline"
    if explicit_states:
        mode = "live" if "live_passed" in explicit_states else "mixed"

    return {
        "$schema": schema_reference or "provider-inventory.schema.json",
        "schemaVersion": "gateway-product-inventory/v1",
        "generatedAt": timestamp,
        "sourceRevision": source_revision(),
        "sources": {
            "manifestRoot": "manifests/lines",
            "cargoManifest": "Cargo.toml",
            "implementationLines": posix_relative(IMPLEMENTATION_LINES_PATH),
            "protocolResolution": posix_relative(PROTOCOL_RESOLUTION_PATH),
            "protocolRegistry": posix_relative(PROTOCOL_REGISTRY_PATH),
        },
        "evidenceSummary": {
            "mode": mode,
            "counts": counts,
            "states": list(EVIDENCE_STATES),
        },
        "lines": lines,
    }


def write_inventory(path: pathlib.Path, payload: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    serialized = json.dumps(payload, indent=2, ensure_ascii=False) + "\n"
    path.write_text(serialized, encoding="utf-8", newline="\n")


def validate_schema(payload: dict[str, Any]) -> None:
    """Apply the checked-in JSON Schema before publishing generated output."""

    try:
        import jsonschema
    except ImportError as exc:  # pragma: no cover - environment contract
        raise InventoryError(
            "jsonschema is required to validate Gateway provider inventory"
        ) from exc
    schema = load_json(SCHEMA_PATH)
    validator = jsonschema.Draft202012Validator(schema)
    errors = sorted(validator.iter_errors(payload), key=lambda error: str(list(error.path)))
    if errors:
        first = errors[0]
        path = "$"
        for part in first.absolute_path:
            path += f"[{part}]" if isinstance(part, int) else f".{part}"
        raise InventoryError(f"Generated inventory violates schema at {path}: {first.message}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--output", type=pathlib.Path, default=DEFAULT_OUTPUT)
    parser.add_argument("--timestamp", help="Override the deterministic evidence timestamp.")
    parser.add_argument("--evidence", action="append", default=[], help="Explicit evidence JSON file.")
    parser.add_argument("--evidence-dir", help="Explicit directory of evidence JSON files.")
    parser.add_argument("--as-json", action="store_true", help="Print a redacted generation summary as JSON.")
    args = parser.parse_args(argv)

    try:
        timestamp = _normalize_timestamp(args.timestamp) if args.timestamp else deterministic_timestamp()
        evidence_files = discover_evidence_files(args.evidence, args.evidence_dir)
        explicit_evidence = load_explicit_evidence(evidence_files, timestamp)
        output_path = args.output.resolve()
        payload = build_inventory(
            timestamp=timestamp,
            explicit_evidence=explicit_evidence,
            schema_reference=schema_reference_for_output(output_path),
        )
        known_ids = {line["id"] for line in payload["lines"]}
        unknown_ids = sorted(set(explicit_evidence) - known_ids)
        if unknown_ids:
            raise InventoryError(
                "Evidence references unknown provider lines: " + ", ".join(unknown_ids)
            )
        validate_schema(payload)
        write_inventory(output_path, payload)
    except (OSError, ValueError, InventoryError) as exc:
        if args.as_json:
            print(json.dumps({"status": "fail", "error": str(exc)}, ensure_ascii=False))
        else:
            print(f"[provider-inventory] fail: {exc}", file=sys.stderr)
        return 1

    summary = {
        "status": "pass",
        "schemaVersion": payload["schemaVersion"],
        "output": args.output.resolve().as_posix(),
        "sourceRevision": payload["sourceRevision"],
        "lineCount": len(payload["lines"]),
        "mode": payload["evidenceSummary"]["mode"],
    }
    if args.as_json:
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    else:
        print(
            f"[provider-inventory] status=pass lines={summary['lineCount']} "
            f"mode={summary['mode']} output={summary['output']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
