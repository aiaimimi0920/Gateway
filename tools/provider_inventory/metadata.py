"""Read source provenance and parse Cargo and Rust inventory metadata."""

from __future__ import annotations

import datetime as dt
import os
import pathlib
import re
import subprocess
from typing import Any

from .contracts import (
    GATEWAY_ROOT,
    IMPLEMENTATION_LINES_PATH,
    PROTOCOL_RESOLUTION_PATH,
    TIMESTAMP_RE,
    InventoryError,
)


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


def read_declared_module_source(facade_path: pathlib.Path) -> str:
    """Read a Rust facade together with its declared file-module owners."""

    facade = facade_path.read_text(encoding="utf-8")
    module_root = facade_path.with_suffix("")
    module_names = re.findall(r"^\s*mod\s+([A-Za-z0-9_]+)\s*;", facade, re.MULTILINE)
    sources = [facade]
    for module_name in module_names:
        if module_name == "tests":
            continue
        module_path = module_root / f"{module_name}.rs"
        if module_path.is_file():
            sources.append(module_path.read_text(encoding="utf-8"))
    return "\n\n".join(sources)


def read_implementation_source() -> str:
    return read_declared_module_source(IMPLEMENTATION_LINES_PATH)


def read_protocol_resolution_source() -> str:
    return read_declared_module_source(PROTOCOL_RESOLUTION_PATH)


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
