#!/usr/bin/env python3
"""Generate Gateway's deterministic provider-reference report and guide."""

from __future__ import annotations

import argparse
import json
import pathlib
import sys

import gateway_provider_references as references


def validate_schema(payload: dict[str, object]) -> None:
    try:
        import jsonschema
    except ImportError as exc:  # pragma: no cover - environment contract
        raise RuntimeError("jsonschema is required for provider reference reports") from exc
    schema = references.load_json(references.REPORT_SCHEMA)
    errors = sorted(
        jsonschema.Draft202012Validator(schema).iter_errors(payload),
        key=lambda error: str(list(error.path)),
    )
    if errors:
        first = errors[0]
        path = "$"
        for part in first.absolute_path:
            path += f"[{part}]" if isinstance(part, int) else f".{part}"
        raise RuntimeError(f"report violates schema at {path}: {first.message}")


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--report", type=pathlib.Path, default=references.DEFAULT_REPORT)
    parser.add_argument("--guide", type=pathlib.Path, default=references.DEFAULT_GUIDE)
    parser.add_argument("--timestamp", help="Override the deterministic UTC timestamp.")
    parser.add_argument("--as-json", action="store_true")
    args = parser.parse_args(argv)

    report_path = args.report.resolve()
    guide_path = args.guide.resolve()
    try:
        timestamp = (
            references.normalize_timestamp(args.timestamp)
            if args.timestamp
            else references.deterministic_timestamp()
        )
        payload = references.build_report(
            timestamp=timestamp,
            report_path=report_path,
            guide_path=guide_path,
        )
        validate_schema(payload)
        secret_issues = references.find_secret_values(payload)
        if secret_issues:
            raise RuntimeError("report contains secret-like values")
        references.write_report(report_path, payload)
        references.write_guide(guide_path, payload)
    except (OSError, ValueError, RuntimeError, json.JSONDecodeError) as exc:
        if args.as_json:
            print(json.dumps({"status": "fail", "error": str(exc)}))
        else:
            print(f"[provider-reference-report] fail: {exc}", file=sys.stderr)
        return 1

    summary = {
        "status": "pass",
        "report": report_path.as_posix(),
        "guide": guide_path.as_posix(),
        "lineCount": payload["summary"]["lineCount"],
        "declaredReferenceCount": payload["summary"]["declaredReferenceCount"],
        "uniqueReferenceCount": payload["summary"]["uniqueReferenceCount"],
        "localPresentReferenceCount": payload["summary"]["localPresentReferenceCount"],
        "externalLegacyReferenceCount": payload["summary"]["externalLegacyReferenceCount"],
        "unclassifiedReferenceCount": payload["summary"]["unclassifiedReferenceCount"],
    }
    if args.as_json:
        print(json.dumps(summary, ensure_ascii=False, sort_keys=True))
    else:
        print(
            "[provider-reference-report] status=pass "
            f"lines={summary['lineCount']} declared={summary['declaredReferenceCount']} "
            f"unique={summary['uniqueReferenceCount']} "
            f"local={summary['localPresentReferenceCount']} "
            f"external={summary['externalLegacyReferenceCount']}"
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
