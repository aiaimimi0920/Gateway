"""Redact evidence material and normalize Gateway artifact references."""

from __future__ import annotations

import pathlib
import re
from typing import Any

from .contracts import GATEWAY_ROOT, InventoryError


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


def posix_relative(path: pathlib.Path) -> str:
    return path.resolve().relative_to(GATEWAY_ROOT.resolve()).as_posix()


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
