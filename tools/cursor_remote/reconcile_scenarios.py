"""Read-only exact-key reconciliation of a real Cursor scenario matrix; never replay."""
import argparse
import hashlib
import json
import sqlite3
from datetime import datetime, timezone
from pathlib import Path


def read(path):
    return json.loads(Path(path).read_text("utf-8"))


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
    receipt, before, after = read(source), read(args.before), read(args.after)
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
            revoked = key is not None and json.loads(key[0])["status"] == "revoked"
            exact = True
            audits = []
            route = case.get("preflight", {})
            for row in sorted(rows, key=lambda r: r["createdAt"]):
                trace = row.get("routeTrace") or {}
                matched = (
                    row.get("status") == "completed" and bool(row.get("completedAt"))
                    and row.get("endpointKind") == "responses" and row.get("upstreamStatus") == 200
                    and row.get("accessKeyId") == key_id and row.get("providerAccountId") == route.get("provider")
                    and trace.get("routeRevision") == route.get("revision") == before["revision"]
                    and trace.get("realCredentialRef") == route.get("credential")
                    and trace.get("selectedPipelineMode") == "cursor_remote"
                    and trace.get("routeAttemptCount") == 1
                    and trace.get("cursorRemoteDeliveryUnknown") is False
                    and trace.get("errorCode") is None and trace.get("fallbackEligible") in (None, False)
                )
                exact = exact and matched
                audits.append({
                    "id": row["id"], "status": row["status"], "upstreamStatus": row.get("upstreamStatus"),
                    "createdAt": row["createdAt"], "completedAt": row.get("completedAt"),
                    "matched": matched, "errorCode": trace.get("errorCode"),
                    "routeAttemptCount": trace.get("routeAttemptCount"),
                })
            cli = case.get("cli", {})
            checks = {
                "originalCasePassed": case.get("passed") is True,
                "fixedCli": receipt["codexVersion"] == "codex-cli 0.155.1",
                "exactTarget": route.get("provider") == receipt["provider"]
                and route.get("credential") == receipt["credential"],
                "exactAudits": bool(rows) and exact,
                "stageCount": len(rows) == case["expectedTools"] + 1,
                "cliToolCount": len(cli.get("commands", [])) == case["expectedTools"],
                "exactAuditSet": {r["id"] for r in rows} == {r["id"] for r in case.get("audits", [])},
                "keyRevoked": revoked,
            }
            cases.append({"name": case["name"], "checks": checks, "passed": all(checks.values()), "audits": audits})
    result = {
        "observedAt": datetime.now(timezone.utc).isoformat(), "sourceReceipt": str(source),
        "sourceReceiptSha256": hashlib.sha256(source.read_bytes()).hexdigest(),
        "configurationUnchanged": unchanged, "cases": cases, "modelRequestsReplayed": 0,
        "passed": receipt.get("passed") is True and len(cases) == 4
        and all(c["passed"] for c in cases) and all(unchanged.values()),
    }
    destination = source.with_name(source.stem + "-reconciled.safe.json")
    with destination.open("x", encoding="utf-8", newline="\n") as output:
        json.dump(result, output, ensure_ascii=False, indent=2)
        output.write("\n")
    print(json.dumps({"evidence": str(destination), **result}, ensure_ascii=False))
    return 0 if result["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
