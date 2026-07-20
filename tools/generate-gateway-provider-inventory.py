#!/usr/bin/env python3
"""Generate the deterministic, secret-free Gateway provider inventory.

The generator intentionally uses only Python's standard library.  It reads
line manifests and source metadata, but it never loads credential samples or
environment credential values.  Live/fixture evidence can be merged only when
an operator explicitly supplies an evidence file or directory.
"""

from __future__ import annotations

import argparse
import datetime as dt
import json
import os
import pathlib
import re
import subprocess
import sys
from collections import defaultdict
from typing import Any, Iterable


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[1]
MANIFEST_ROOT = GATEWAY_ROOT / "manifests" / "lines"
CARGO_PATH = GATEWAY_ROOT / "Cargo.toml"
IMPLEMENTATION_LINES_PATH = GATEWAY_ROOT / "src" / "implementation_lines.rs"
PROTOCOL_RESOLUTION_PATH = GATEWAY_ROOT / "src" / "routing" / "protocol_resolution.rs"
PROTOCOL_REGISTRY_PATH = GATEWAY_ROOT / "src" / "protocol" / "registry.rs"
DEFAULT_OUTPUT = GATEWAY_ROOT / "docs" / "provider-inventory.json"
SCHEMA_PATH = GATEWAY_ROOT / "docs" / "provider-inventory.schema.json"

# This is an intentionally checked-in contract.  Manifest discovery is used
# for metadata, but it must never silently redefine the product surface.
CANONICAL_PROVIDER_LINE_IDS = (
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
)

EVIDENCE_STATES = (
    "compiled",
    "metadata_only",
    "fixture_passed",
    "live_passed",
    "external_gate",
    "credential_missing",
    "known_unsupported",
)

SECRET_KEY_RE = re.compile(
    r"(?:api[_-]?key|access[_-]?token|auth(?:orization)?|cookie|credential|"
    r"jwt|password|private[_-]?key|refresh[_-]?token|secret|session[_-]?token)",
    re.IGNORECASE,
)
SECRET_VALUE_RE = re.compile(
    r"(?:Bearer\s+[A-Za-z0-9._~+/=-]+|Basic\s+[A-Za-z0-9+/=]{8,}|"
    r"sk-[A-Za-z0-9._-]+|AKIA[0-9A-Z]{16}|AIza[0-9A-Za-z_-]{20,}|"
    r"(?:xox[baprs]-|gh[pousr]_[A-Za-z0-9_]+)|"
    r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9._-]+\.[A-Za-z0-9._-]+|"
    r"(?:cookie|set-cookie)\s*[:=]\s*[^;\r\n]+|"
    r"(?:session(?:id|_token)?|refresh_token|api[_-]?key|access[_-]?token|"
    r"authorization|password|secret|aws[_-]?(?:secret[_-]?access[_-]?key|session[_-]?token))\s*[=:]\s*[^;\s,]+|"
    r"ya29\.[A-Za-z0-9._-]+)",
    re.IGNORECASE,
)
TIMESTAMP_RE = re.compile(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$")


class InventoryError(RuntimeError):
    """Raised for malformed source metadata or explicit evidence."""


def load_json(path: pathlib.Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def posix_relative(path: pathlib.Path) -> str:
    return path.resolve().relative_to(GATEWAY_ROOT.resolve()).as_posix()


def schema_reference_for_output(output_path: pathlib.Path) -> str:
    return pathlib.PurePath(
        os.path.relpath(SCHEMA_PATH.resolve(), output_path.resolve().parent)
    ).as_posix()


def normalize_gateway_artifact_paths(value: Any, line_id: str) -> list[str]:
    if not isinstance(value, list):
        raise InventoryError(f"Evidence record for {line_id} has invalid artifactPaths")
    normalized: list[str] = []
    for raw_path in value:
        if not isinstance(raw_path, str) or not raw_path.strip():
            raise InventoryError(
                f"Evidence record for {line_id} has an invalid artifact path"
            )
        candidate = raw_path.strip().replace("\\", "/")
        pure = pathlib.PurePosixPath(candidate)
        if pure.is_absolute() or re.match(r"^[A-Za-z]:", candidate):
            try:
                absolute = pathlib.Path(raw_path).resolve()
                normalized.append(posix_relative(absolute))
                continue
            except (OSError, ValueError):
                pass
        if ".." in pure.parts:
            try:
                absolute = (GATEWAY_ROOT / pathlib.Path(candidate)).resolve()
                normalized.append(posix_relative(absolute))
                continue
            except (OSError, ValueError):
                pass
        if pure.is_absolute() or re.match(r"^[A-Za-z]:", candidate) or ".." in pure.parts:
            raise InventoryError(
                f"Evidence record for {line_id} artifact paths must be Gateway-relative"
            )
        normalized.append(pure.as_posix())
    return sorted(dict.fromkeys(normalized))


def sanitize(value: Any, key: str | None = None) -> Any:
    """Return JSON-safe metadata with secret-like fields redacted.

    Provider credentials are represented by material kinds and documentation
    paths only.  This defensive pass also protects explicitly supplied
    evidence files from accidentally copying a token into the inventory.
    """

    if key and SECRET_KEY_RE.search(key) and key not in {
        "credentialSource",
        "credential_source",
    }:
        if isinstance(value, (dict, list)):
            return "<redacted>"
        return "<redacted>"
    if isinstance(value, dict):
        return {str(k): sanitize(v, str(k)) for k, v in value.items()}
    if isinstance(value, list):
        return [sanitize(item) for item in value]
    if isinstance(value, str):
        return SECRET_VALUE_RE.sub("<redacted>", value)
    return value


def _safe_proof_object(value: Any, line_id: str, field_name: str) -> dict[str, Any]:
    if not isinstance(value, dict) or not value:
        raise InventoryError(
            f"Evidence record for {line_id} requires non-empty {field_name} route proof"
        )
    sanitized = sanitize(value)
    provider_line = sanitized.get("providerLine") or sanitized.get("provider_line")
    if provider_line != line_id:
        raise InventoryError(
            f"Evidence record for {line_id} {field_name} must identify the same provider line"
        )
    return sanitized


def _git(*arguments: str) -> str | None:
    try:
        completed = subprocess.run(
            ["git", *arguments],
            cwd=GATEWAY_ROOT,
            check=False,
            stdout=subprocess.PIPE,
            stderr=subprocess.DEVNULL,
            text=True,
            encoding="utf-8",
        )
    except OSError:
        return None
    if completed.returncode != 0:
        return None
    value = completed.stdout.strip()
    return value or None


def source_revision() -> str:
    return _git("rev-parse", "HEAD") or "unknown"


def _normalize_timestamp(value: str) -> str:
    raw = value.strip()
    if raw.endswith("Z") and TIMESTAMP_RE.match(raw):
        return raw
    try:
        parsed = dt.datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError as exc:
        raise InventoryError(f"Invalid evidence timestamp: {value}") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    parsed = parsed.astimezone(dt.timezone.utc).replace(microsecond=0)
    return parsed.strftime("%Y-%m-%dT%H:%M:%SZ")


def deterministic_timestamp() -> str:
    """Use an explicit build epoch or HEAD commit time for repeatable output."""

    epoch = os.environ.get("SOURCE_DATE_EPOCH")
    if epoch:
        try:
            return dt.datetime.fromtimestamp(
                int(epoch), tz=dt.timezone.utc
            ).strftime("%Y-%m-%dT%H:%M:%SZ")
        except (TypeError, ValueError, OverflowError):
            pass
    commit_time = _git("show", "-s", "--format=%cI", "HEAD")
    if commit_time:
        try:
            return _normalize_timestamp(commit_time)
        except InventoryError:
            pass
    return "1970-01-01T00:00:00Z"


def _strip_toml_comment(line: str) -> str:
    quoted = False
    escaped = False
    for index, char in enumerate(line):
        if char == "\\" and quoted and not escaped:
            escaped = True
            continue
        if char == '"' and not escaped:
            quoted = not quoted
        if char == "#" and not quoted:
            return line[:index]
        escaped = False
    return line


def _bracket_delta(text: str) -> int:
    return text.count("[") - text.count("]")


def parse_cargo_features(text: str) -> dict[str, list[str]]:
    """Parse the small `[features]` table without requiring a TOML package."""

    features: dict[str, list[str]] = {}
    in_features = False
    pending_name: str | None = None
    pending_value = ""
    depth = 0

    for raw_line in text.splitlines():
        line = _strip_toml_comment(raw_line).strip()
        if not line:
            continue
        if line.startswith("[") and line.endswith("]") and depth == 0:
            in_features = line == "[features]"
            continue
        if not in_features:
            continue
        if pending_name is None:
            match = re.match(r"^([A-Za-z0-9_-]+)\s*=\s*(.*)$", line)
            if not match:
                continue
            pending_name = match.group(1)
            pending_value = match.group(2)
            depth = _bracket_delta(pending_value)
            if depth <= 0:
                features[pending_name] = re.findall(r'"([^"]+)"', pending_value)
                pending_name = None
                pending_value = ""
                depth = 0
        else:
            pending_value += " " + line
            depth += _bracket_delta(line)
            if depth <= 0:
                features[pending_name] = re.findall(r'"([^"]+)"', pending_value)
                pending_name = None
                pending_value = ""
                depth = 0

    return features


def extract_function_block(text: str, function_name: str) -> str:
    marker = f"pub fn {function_name}"
    start = text.find(marker)
    if start < 0:
        return ""
    brace = text.find("{", start)
    if brace < 0:
        return ""
    depth = 0
    for index in range(brace, len(text)):
        char = text[index]
        if char == "{":
            depth += 1
        elif char == "}":
            depth -= 1
            if depth == 0:
                return text[start : index + 1]
    return text[start:]


def parse_constants(text: str) -> dict[str, str]:
    return dict(re.findall(r'pub const ([A-Z0-9_]+):\s*&str\s*=\s*"([^"]+)"', text))


def parse_variant_string_map(block: str) -> dict[str, str]:
    return dict(re.findall(r"Self::([A-Za-z0-9_]+)\s*=>\s*\"([^\"]+)\"", block))


def parse_variant_constant_map(block: str, constants: dict[str, str]) -> dict[str, str]:
    result: dict[str, str] = {}
    for variant, constant in re.findall(
        r"Self::([A-Za-z0-9_]+)\s*=>\s*(LINE_[A-Z0-9_]+)", block
    ):
        result[variant] = constants.get(constant, constant)
    return result


def parse_alias_variant_map(block: str) -> dict[str, str]:
    """Parse match arms whose body may be wrapped in braces or span lines."""

    result: dict[str, str] = {}
    pending: list[str] = []
    armed = False
    for line in block.splitlines():
        if "=>" in line:
            before, after = line.split("=>", 1)
            pending.extend(re.findall(r'"([^"]+)"', before))
            armed = True
            variant_match = re.search(
                r"RefactoredImplementationLine::([A-Za-z0-9_]+)", after
            )
            if variant_match:
                for alias in pending:
                    result[alias] = variant_match.group(1)
                pending = []
                armed = False
        elif armed:
            variant_match = re.search(
                r"RefactoredImplementationLine::([A-Za-z0-9_]+)", line
            )
            if variant_match:
                for alias in pending:
                    result[alias] = variant_match.group(1)
                pending = []
                armed = False
        elif '"' in line and ("|" in line or line.strip().startswith('"')):
            pending.extend(re.findall(r'"([^"]+)"', line))
    return result


def parse_implementation_metadata(text: str) -> dict[str, Any]:
    constants = parse_constants(text)
    canonical = parse_variant_string_map(extract_function_block(text, "canonical_protocol_profile"))
    feature_by_variant = parse_variant_constant_map(
        extract_function_block(text, "feature_name"), constants
    )
    profile_aliases = parse_alias_variant_map(
        extract_function_block(text, "line_for_protocol_profile")
    )
    adapter_aliases = parse_alias_variant_map(extract_function_block(text, "line_for_adapter"))
    return {
        "canonicalByVariant": canonical,
        "featureByVariant": feature_by_variant,
        "profileAliases": profile_aliases,
        "adapterAliases": adapter_aliases,
        "sourceText": text,
    }


def parse_route_metadata(resolution_text: str, registry_text: str) -> dict[str, Any]:
    constants = parse_constants(registry_text)
    block = extract_function_block(
        resolution_text, "surface_supported_wire_protocol_families"
    )
    adapter_map: dict[str, list[str]] = {}
    arm_pattern = re.compile(
        r"(?P<patterns>(?:\s*\"[^\"]+\"\s*(?:\|\s*)?)+)=>\s*vec!\[(?P<body>.*?)\]",
        re.DOTALL,
    )
    for match in arm_pattern.finditer(block):
        aliases = re.findall(r'"([^"]+)"', match.group("patterns"))
        values: list[str] = []
        body = match.group("body")
        for constant in re.findall(r"\b([A-Z][A-Z0-9_]+_FAMILY)\b", body):
            values.append(constants.get(constant, constant))
        if not values:
            values.extend(re.findall(r'"([^"]+)"', body))
        for alias in aliases:
            adapter_map[alias] = sorted(dict.fromkeys(values))
    return {"adapterFamilies": adapter_map, "sourceText": resolution_text}


def _wire_families_for_line(
    manifest: dict[str, Any], route_metadata: dict[str, Any]
) -> list[str]:
    adapter = str(manifest.get("identity", {}).get("adapter", ""))
    profile = str(manifest.get("identity", {}).get("protocolProfile", ""))
    values = list(route_metadata["adapterFamilies"].get(adapter, []))
    if values:
        return values
    # Search adapters intentionally resolve from the fallback protocol profile.
    search_profiles = {
        "perplexity_search": "perplexity_search",
        "tavily": "tavily_search",
        "exa": "exa_search",
        "jina_search": "jina_search",
        "jina_reader": "jina_reader",
        "linkup": "linkup_search",
        "you_search": "you_search",
        "websearchapi": "websearchapi_search",
    }
    if profile in search_profiles:
        return [search_profiles[profile]]
    return sorted(
        {
            str(family)
            for family in (manifest.get("capabilities", {}).get("families") or [])
            if isinstance(family, str)
        }
    )


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
    implementation_text = IMPLEMENTATION_LINES_PATH.read_text(encoding="utf-8")
    implementation = parse_implementation_metadata(implementation_text)
    route = parse_route_metadata(
        PROTOCOL_RESOLUTION_PATH.read_text(encoding="utf-8"),
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
