"""Independent, read-only protocol receipt/account/config reconciliation; never send a model request."""
import argparse
import hashlib
import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path


def load(path):
    return json.loads(Path(path).read_text("utf-8"))


def exact_metered_usage(case, rows):
    phases = case.get("phases", [])
    sent = [row for row in rows if row.get("status") == "completed"]
    if len(phases) != 1 or len(sent) != 1:
        return False
    usage = phases[0].get("usage")
    if not isinstance(usage, dict) or set(usage) != {"input", "output", "total"}:
        return False
    if not all(type(n) is int and 0 < n <= 2**53 - 1 for n in usage.values()):
        return False
    fields = (("promptTokens", "input"), ("completionTokens", "output"), ("totalTokens", "total"))
    return usage["total"] == usage["input"] + usage["output"] and all(
        type(sent[0].get(field)) is int and sent[0][field] == usage[key] for field, key in fields
    )


def requires_metered_usage(name):
    return name in {
        "bedrock-json-text", "bedrock-stream-text", "bedrock-json-image", "bedrock-stream-image",
        "dashscope-json-text", "dashscope-stream-text", "dashscope-json-message",
        "dashscope-multimodal-json-text", "dashscope-multimodal-stream-text",
        "dashscope-multimodal-json-image", "dashscope-multimodal-stream-image",
    }


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--receipt", required=True)
    parser.add_argument("--database", required=True)
    parser.add_argument("--before", required=True)
    parser.add_argument("--after", required=True)
    args = parser.parse_args()
    source = Path(args.receipt).resolve(strict=True)
    database = Path(args.database).resolve(strict=True)
    if source.drive.upper() != "C:" or database.drive.upper() != "C:":
        raise RuntimeError("Local evidence and database required")
    receipt, before, after = load(source), load(args.before), load(args.after)
    unchanged = {name: before[name] == after[name] for name in (
        "revision", "documentSha256", "routingShape", "criticalFileHashes",
        "credentialsOnDisk", "accountSummary", "cursorProjection",
    )}
    cases = []
    with sqlite3.connect(database.as_uri() + "?mode=ro", uri=True, timeout=10) as db:
        db.execute("PRAGMA query_only=ON")
        for case in receipt["cases"]:
            key_id = case.get("accessKeyId")
            rows = [json.loads(row[0]) for row in db.execute(
                "SELECT payload FROM request_audits WHERE json_extract(payload, '$.accessKeyId')=?",
                (key_id,),
            )]
            key = db.execute("SELECT payload FROM local_access_keys WHERE id=?", (key_id,)).fetchone()
            route = case.get("before", {})
            positive, negative, exact = 0, 0, True
            for row in rows:
                trace = row.get("routeTrace") or {}
                shared = bool(row.get("completedAt")) and row.get("accessKeyId") == key_id \
                    and row.get("endpointKind") == case["endpoint"]
                if case.get("protocolFamily"):
                    shared = shared and trace.get("requestedProtocolFamily") == case["protocolFamily"]
                if row.get("status") == "completed":
                    positive += 1
                    matched = shared and (
                        row.get("upstreamStatus") == 200
                        and row.get("providerAccountId") == route.get("provider") == receipt["provider"]
                        and trace.get("routeRevision") == route.get("revision") == before["revision"]
                        and trace.get("realCredentialRef") == route.get("credential") == receipt["credential"]
                        and trace.get("selectedPipelineMode") == "cursor_remote"
                        and trace.get("routeAttemptCount") == 1
                        and trace.get("cursorRemoteDeliveryUnknown") is False
                        and trace.get("errorCode") is None and trace.get("fallbackEligible") in (None, False)
                    )
                else:
                    negative += 1
                    matched = shared and (
                        row.get("upstreamStatus") is None and trace.get("routeAttemptCount") == 0
                        and "cursorRemoteDeliveryUnknown" in trace
                        and trace["cursorRemoteDeliveryUnknown"] is None
                        and trace.get("errorCode") in {r["code"] for r in case.get("rejections", [])}
                    )
                exact = exact and matched
            checks = {
                "originalCasePassed": case.get("passed") is True,
                "wirePassed": case.get("wirePassed") is True,
                "exactAudits": bool(rows) and exact,
                "stageCount": positive == len(case.get("phases", [])),
                "rejectionCount": negative == len(case.get("rejections", [])),
                "rejectionCodes": sorted((r.get("routeTrace") or {}).get("errorCode", "") for r in rows
                                         if r.get("status") != "completed")
                == sorted(r["code"] for r in case.get("rejections", [])),
                "exactAuditSet": {r["id"] for r in rows} == {r["id"] for r in case.get("audits", [])},
                "keyRevoked": key is not None and json.loads(key[0])["status"] == "revoked",
            }
            if requires_metered_usage(case["name"]):
                checks["exactUsage"] = exact_metered_usage(case, rows)
            cases.append({"name": case["name"], "checks": checks, "passed": all(checks.values()),
                          "positiveAudits": positive, "negativeAudits": negative})
    result = {
        "observedAt": datetime.now(timezone.utc).isoformat(), "sourceReceipt": str(source),
        "sourceReceiptSha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "configurationUnchanged": unchanged, "cases": cases, "modelRequestsReplayed": 0,
        "passed": receipt.get("passed") is True and len(cases) == len(receipt["selectedCases"])
        and all(c["passed"] for c in cases) and all(unchanged.values()),
    }
    destination = source.with_name(source.stem + "-reconciled.safe.json")
    with destination.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(result, output, ensure_ascii=False, indent=2)
        output.write("\n")
    print(json.dumps({"evidence": str(destination), "passed": result["passed"], "cases": cases}))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
