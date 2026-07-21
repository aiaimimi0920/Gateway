#!/usr/bin/env python3
"""Shared helpers for Gateway provider reference reports.

The line manifests intentionally retain their historical documentation and
credential-reference paths.  This module classifies those paths without
opening credential samples or importing material from another project.
"""

from __future__ import annotations

import datetime as dt
import hashlib
import json
import os
import pathlib
import re
import subprocess
from collections import Counter, defaultdict
from typing import Any, Iterable


GATEWAY_ROOT = pathlib.Path(__file__).resolve().parents[1]
MAX_REFERENCE_PATH_LENGTH = 4096
MANIFEST_ROOT = GATEWAY_ROOT / "manifests" / "lines"
DEFAULT_REPORT = GATEWAY_ROOT / "docs" / "provider-reference-report.json"
DEFAULT_GUIDE = GATEWAY_ROOT / "docs" / "provider-reference-guide.md"
REPORT_SCHEMA = GATEWAY_ROOT / "docs" / "provider-reference-report.schema.json"
SCHEMA_VERSION = "gateway-provider-reference-report/v1"

REFERENCE_FIELDS: tuple[tuple[str, tuple[str, ...]], ...] = (
    ("credentials.samplePath", ("credentials", "samplePath")),
    ("credentials.fieldsDocPath", ("credentials", "fieldsDocPath")),
    ("docs.overviewDocPath", ("docs", "overviewDocPath")),
    ("docs.buildDocPath", ("docs", "buildDocPath")),
)
ALLOWED_STATUSES = ("local_present", "external_legacy_reference")
STATUS_SET = set(ALLOWED_STATUSES)

# This scanner is deliberately value-oriented.  Metadata labels such as
# ``materialKinds: ["api_key"]`` are safe; credential material is not.
SECRET_VALUE_RE = re.compile(
    r"(?:Bearer\s+[A-Za-z0-9._~+/=-]{16,}|Basic\s+[A-Za-z0-9+/=]{16,}|"
    r"sk-[A-Za-z0-9._-]{16,}|sk_(?:live|test)_[A-Za-z0-9]{16,}|"
    r"(?-i:(?:AKIA|ASIA)[0-9A-Z]{16})|glpat-[A-Za-z0-9_-]{16,}|"
    r"npm_[A-Za-z0-9]{24,}|AIza[0-9A-Za-z_-]{20,}|"
    r"xox[baprs]-[A-Za-z0-9-]+|gh[pousr]_[A-Za-z0-9_]+|"
    r"eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9._-]+\.[A-Za-z0-9._-]+|"
    r"(?:cookie|set-cookie)\s*[:=]\s*[^;\r\n]+|"
    r"(?:session(?:id|_token)?|refresh_token|api[_-]?key|access[_-]?token|"
    r"authorization|password|secret|private[_-]?key|aws[_-]?(?:secret[_-]?access[_-]?key|session[_-]?token))"
    r"\s*[=:]\s*[^;\s,]+|ya29\.[A-Za-z0-9._-]+)",
    re.IGNORECASE,
)
TIMESTAMP_RE = re.compile(
    r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z$"
)


def load_json(path: pathlib.Path) -> Any:
    with path.open("r", encoding="utf-8") as handle:
        return json.load(handle)


def gateway_relative(path: pathlib.Path) -> str:
    return path.resolve().relative_to(GATEWAY_ROOT.resolve()).as_posix()


def display_path(path: pathlib.Path) -> str:
    """Render a path deterministically for reports written outside Gateway."""

    try:
        return gateway_relative(path)
    except (ValueError, OSError):
        return path.name


def schema_reference_for_output(output_path: pathlib.Path) -> str:
    return pathlib.PurePath(
        os.path.relpath(REPORT_SCHEMA.resolve(), output_path.resolve().parent)
    ).as_posix()


def _git(*arguments: str) -> str | None:
    try:
        result = subprocess.run(
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
    if result.returncode != 0:
        return None
    value = result.stdout.strip()
    return value or None


def source_revision(manifests: Iterable[pathlib.Path] | None = None) -> str:
    selected = list(manifests) if manifests is not None else discover_manifests()
    return f"manifest-source:{_source_fingerprint(selected)}"


def normalize_timestamp(value: str) -> str:
    raw = value.strip()
    if raw.endswith("Z") and TIMESTAMP_RE.match(raw):
        return raw
    try:
        parsed = dt.datetime.fromisoformat(raw.replace("Z", "+00:00"))
    except ValueError as exc:
        raise ValueError(f"invalid timestamp: {value}") from exc
    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=dt.timezone.utc)
    return parsed.astimezone(dt.timezone.utc).replace(microsecond=0).strftime(
        "%Y-%m-%dT%H:%M:%SZ"
    )


def deterministic_timestamp() -> str:
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
            return normalize_timestamp(commit_time)
        except ValueError:
            pass
    return "1970-01-01T00:00:00Z"


def discover_manifests() -> list[pathlib.Path]:
    return sorted(MANIFEST_ROOT.rglob("*.json"), key=lambda path: gateway_relative(path))


def nested_value(payload: dict[str, Any], path: tuple[str, ...]) -> Any:
    current: Any = payload
    for key in path:
        if not isinstance(current, dict):
            return None
        current = current.get(key)
    return current


def normalize_reference_path(raw: Any) -> str:
    if not isinstance(raw, str):
        return ""
    return raw


def reference_path_issue(raw_path: Any) -> str | None:
    """Return a reason when a report path is not a canonical Gateway path."""

    if not isinstance(raw_path, str) or not raw_path:
        return "reference_path_missing"
    if len(raw_path) > MAX_REFERENCE_PATH_LENGTH:
        return "reference_path_too_long"
    if raw_path != raw_path.strip():
        return "reference_path_not_trimmed"
    if "\x00" in raw_path:
        return "reference_path_contains_nul"
    if "\\" in raw_path:
        return "reference_path_uses_backslash"
    if re.match(r"^[A-Za-z]:", raw_path):
        return "drive_path_not_owned_by_gateway"
    if ":" in raw_path:
        return "colon_not_allowed_in_gateway_path"

    candidate = pathlib.PurePosixPath(raw_path)
    if candidate.is_absolute():
        return "absolute_path_not_owned_by_gateway"
    if candidate.as_posix() != raw_path:
        return "non_canonical_path_not_owned_by_gateway"
    if any(part in {"", ".", ".."} for part in candidate.parts):
        return "non_canonical_path_not_owned_by_gateway"

    try:
        resolved = (GATEWAY_ROOT / pathlib.Path(*candidate.parts)).resolve()
        resolved.relative_to(GATEWAY_ROOT.resolve())
    except (OSError, RuntimeError, ValueError):
        return "path_unresolvable_or_escapes_gateway"
    try:
        if resolved.exists() and not resolved.is_file():
            return "reference_path_is_not_file"
    except OSError:
        return "path_unresolvable_or_escapes_gateway"
    return None


def resolve_local_reference(raw_path: str) -> tuple[str, str]:
    """Return (status, reason) without following paths outside Gateway."""

    path_issue = reference_path_issue(raw_path)
    if path_issue:
        raise ValueError(f"invalid Gateway reference path {raw_path!r}: {path_issue}")
    candidate = pathlib.PurePosixPath(raw_path)
    local_path = (GATEWAY_ROOT / pathlib.Path(*candidate.parts)).resolve()
    try:
        local_path.relative_to(GATEWAY_ROOT.resolve())
    except ValueError as exc:
        raise ValueError(
            f"invalid Gateway reference path {raw_path!r}: path_escapes_gateway"
        ) from exc
    if local_path.is_file():
        return "local_present", "file_present_in_gateway"
    return "external_legacy_reference", "path_not_present_in_gateway"


def line_anchor(line_id: str) -> str:
    return f"#line-{line_id}"


def _source_fingerprint(manifests: Iterable[pathlib.Path]) -> str:
    digest = hashlib.sha256()
    for path in manifests:
        digest.update(gateway_relative(path).encode("utf-8"))
        digest.update(b"\0")
        payload = (
            path.read_bytes().replace(b"\r\n", b"\n").replace(b"\r", b"\n")
        )
        digest.update(payload)
        digest.update(b"\0")
    return digest.hexdigest()


def _reference_record(
    *,
    line_id: str,
    manifest_path: str,
    field: str,
    raw_path: Any,
    guide_path: str,
) -> dict[str, Any]:
    if not isinstance(raw_path, str) or not raw_path:
        raise ValueError(f"{line_id}/{field}: reference declaration is missing")
    normalized = normalize_reference_path(raw_path)
    path_issue = reference_path_issue(normalized)
    if path_issue:
        raise ValueError(
            f"{line_id}/{field}: invalid Gateway reference path {raw_path!r}: "
            f"{path_issue}"
        )
    status, reason = resolve_local_reference(normalized)
    return {
        "lineId": line_id,
        "manifestPath": manifest_path,
        "field": field,
        "path": normalized,
        "status": status,
        "resolution": {
            "reason": reason,
            "guidePath": guide_path,
            "guideAnchor": line_anchor(line_id),
            "action": (
                "use_local_placeholder_template_and_keep_material_out_of_gateway_docs"
                if status == "external_legacy_reference"
                else "use_gateway_owned_file_after_review"
            ),
        },
    }


def _line_identity(manifest: dict[str, Any]) -> dict[str, Any]:
    identity = manifest.get("identity")
    return dict(identity) if isinstance(identity, dict) else {}


def _line_capabilities(manifest: dict[str, Any]) -> dict[str, Any]:
    capabilities = manifest.get("capabilities")
    return dict(capabilities) if isinstance(capabilities, dict) else {}


def build_report(
    *,
    timestamp: str,
    report_path: pathlib.Path,
    guide_path: pathlib.Path,
) -> dict[str, Any]:
    manifests = discover_manifests()
    guide_display = display_path(guide_path)
    references: list[dict[str, Any]] = []
    lines: list[dict[str, Any]] = []
    for manifest_path in manifests:
        manifest = load_json(manifest_path)
        line_id = str(manifest.get("id") or manifest_path.stem)
        manifest_display = gateway_relative(manifest_path)
        line_refs = [
            _reference_record(
                line_id=line_id,
                manifest_path=manifest_display,
                field=field,
                raw_path=nested_value(manifest, path),
                guide_path=guide_display,
            )
            for field, path in REFERENCE_FIELDS
        ]
        references.extend(line_refs)
        status_counts = Counter(item["status"] for item in line_refs)
        lines.append(
            {
                "id": line_id,
                "identity": _line_identity(manifest),
                "capabilities": _line_capabilities(manifest),
                "materialKinds": list(
                    (manifest.get("credentials") or {}).get("materialKinds") or []
                ),
                "manifestPath": manifest_display,
                "legacyReferences": line_refs,
                "resolution": {
                    "localPresentReferenceCount": status_counts["local_present"],
                    "externalLegacyReferenceCount": status_counts[
                        "external_legacy_reference"
                    ],
                    "unclassifiedReferenceCount": 4
                    - sum(status_counts.values()),
                    "guidePath": guide_display,
                    "guideAnchor": line_anchor(line_id),
                },
            }
        )

    references.sort(key=lambda item: (item["lineId"], item["field"]))
    unique_by_path: dict[str, list[dict[str, Any]]] = defaultdict(list)
    for item in references:
        unique_by_path[item["path"]].append(item)
    unique_references = []
    for path in sorted(unique_by_path):
        declarations = unique_by_path[path]
        statuses = {item["status"] for item in declarations}
        # A path has one filesystem status; retain a deterministic failure mode
        # if a future malformed manifest produces inconsistent declarations.
        status = next(iter(statuses)) if len(statuses) == 1 else "external_legacy_reference"
        unique_references.append(
            {
                "path": path,
                "status": status,
                "declarationCount": len(declarations),
                "declarations": [
                    {"lineId": item["lineId"], "field": item["field"]}
                    for item in declarations
                ],
            }
        )

    status_counts = Counter(item["status"] for item in references)
    field_counts = Counter(item["field"] for item in references)
    return {
        "$schema": schema_reference_for_output(report_path),
        "schemaVersion": SCHEMA_VERSION,
        "generatedAt": timestamp,
        "sourceRevision": source_revision(manifests),
        "sourceFingerprint": _source_fingerprint(manifests),
        "guidePath": guide_display,
        "summary": {
            "lineCount": len(lines),
            "declaredReferenceCount": len(references),
            "uniqueReferenceCount": len(unique_references),
            "localPresentReferenceCount": status_counts["local_present"],
            "externalLegacyReferenceCount": status_counts[
                "external_legacy_reference"
            ],
            "unclassifiedReferenceCount": len(references)
            - sum(status_counts.values()),
            "statusCounts": {
                "local_present": status_counts["local_present"],
                "external_legacy_reference": status_counts[
                    "external_legacy_reference"
                ],
            },
            "fieldCounts": {
                field: field_counts[field] for field, _ in REFERENCE_FIELDS
            },
        },
        "lines": sorted(lines, key=lambda item: item["id"]),
        "references": references,
        "uniqueReferences": unique_references,
    }


def render_guide(report: dict[str, Any]) -> str:
    summary = report["summary"]
    lines: list[str] = [
        "# Gateway Provider Reference Guide",
        "",
        "This guide closes the documentation and credential-reference gap for the",
        "Gateway provider-line manifests without copying legacy provider material.",
        "Manifest paths are retained as provenance. Each path is classified in the",
        "deterministic report and can be resolved locally only after an operator",
        "reviews a Gateway-owned, sanitized file.",
        "",
        "## Resolution model",
        "",
        "| Status | Meaning | Operator action |",
        "| --- | --- | --- |",
        "| `local_present` | The referenced file exists under Gateway. | Review its contents and keep material outside checked-in docs. |",
        "| `external_legacy_reference` | The historical path is absent or not owned by Gateway. | Use the safe placeholder below; do not copy a sibling project tree. |",
        "",
        "Current deterministic baseline: **{declared} declared references**, "
        "**{unique} unique paths**, **{external} external legacy references**, "
        "and **{unclassified} unclassified references**.".format(
            declared=summary["declaredReferenceCount"],
            unique=summary["uniqueReferenceCount"],
            external=summary["externalLegacyReferenceCount"],
            unclassified=summary["unclassifiedReferenceCount"],
        ),
        "",
        "`sourceRevision` is the content-addressed identity",
        "`manifest-source:<sha256>` and does not depend on Git commit history.",
        "`sourceFingerprint` is the SHA-256 of the same sorted manifest paths",
        "and bytes after normalizing text line endings to LF, including uncommitted",
        "source changes.",
        "",
        "## Safe local template",
        "",
        "The following values are intentionally non-credentials. Replace them only",
        "in an ignored local file or an approved secret store; never put real values",
        "in this guide, the report, or a manifest.",
        "",
        "```json",
        "{",
        '  "value_slot": "INSERT_PROVIDER_VALUE_HERE",',
        '  "state_file": "PATH_TO_LOCAL_STATE_FILE"',
        "}",
        "```",
        "",
        "To promote a legacy reference to `local_present`, create the reviewed file",
        "inside Gateway, rerun the generator, and inspect the report diff. The",
        "generator never reads the file contents; the validator only checks that the",
        "path is Gateway-owned and present.",
        "",
        "## Reproducible commands",
        "",
        "Run both commands from the Gateway repository root after changing a manifest or",
        "intentionally adding a Gateway-owned reference:",
        "",
        "```powershell",
        "python tools/generate-gateway-provider-reference-report.py",
        "python tools/validate-gateway-provider-reference-report.py --as-json",
        "```",
        "",
        "## Provider line catalog",
        "",
    ]
    for line in report["lines"]:
        line_id = line["id"]
        identity = line["identity"]
        capabilities = line["capabilities"]
        capability_values = capabilities.get("families") or []
        material_kinds = line["materialKinds"] or []
        resolution = line["resolution"]
        lines.extend(
            [
                f'<a id="line-{line_id}"></a>',
                f"### {line_id}",
                "",
                f"- Identity: `{identity.get('serviceProviderKey', '')}` / `{identity.get('providerSurfaceKey', '')}`",
                f"- Capabilities: {', '.join(f'`{value}`' for value in capability_values) or '`none declared`'}",
                f"- Material kinds: {', '.join(f'`{value}`' for value in material_kinds) or '`none declared`'}",
                f"- Manifest: `{line['manifestPath']}`",
                f"- Resolution: `{resolution['localPresentReferenceCount']}` local present, `{resolution['externalLegacyReferenceCount']}` external legacy, `{resolution['unclassifiedReferenceCount']}` unclassified",
                "- Legacy references:",
            ]
        )
        for reference in line["legacyReferences"]:
            lines.append(
                f"  - `{reference['field']}` -> `{reference['path']}` (`{reference['status']}`; `{reference['resolution']['reason']}`)"
            )
        lines.append("")
    return "\n".join(lines).rstrip() + "\n"


def write_report(path: pathlib.Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(
        json.dumps(report, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
        newline="\n",
    )


def write_guide(path: pathlib.Path, report: dict[str, Any]) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(render_guide(report), encoding="utf-8", newline="\n")


def find_secret_values(value: Any, path: str = "$") -> list[dict[str, str]]:
    issues: list[dict[str, str]] = []
    if isinstance(value, dict):
        for key, item in value.items():
            issues.extend(find_secret_values(item, f"{path}.{key}"))
    elif isinstance(value, list):
        for index, item in enumerate(value):
            issues.extend(find_secret_values(item, f"{path}[{index}]"))
    elif isinstance(value, str) and SECRET_VALUE_RE.search(value):
        issues.append({"path": path, "value": "<redacted>"})
    return issues
