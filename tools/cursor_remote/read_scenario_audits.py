"""Explicit local read-only audit source; emit only scoped, non-secret receipt fields."""
import json
import sqlite3
import sys
from pathlib import Path

path = Path(sys.argv[1])
key = sys.argv[2]
if (not path.is_absolute() or path.drive.upper() != "C:" or not path.is_file()
        or path.is_symlink() or path.resolve(strict=True) != path
        or not key.startswith("local-ak-") or len(key) > 100):
    raise RuntimeError("Explicit local database and exact key required")
with sqlite3.connect(path.as_uri() + "?mode=ro", uri=True, timeout=10) as db:
    db.execute("PRAGMA query_only=ON")
    rows = [json.loads(row[0]) for row in db.execute(
        "SELECT payload FROM request_audits WHERE json_extract(payload, '$.accessKeyId')=? LIMIT 101", (key,),
    )]
if len(rows) > 100:
    raise RuntimeError("Audit bound exceeded")
audits = []
for row in rows:
    safe = {name: row.get(name) for name in (
        "id", "accessKeyId", "endpointKind", "status", "providerAccountId", "upstreamStatus",
        "createdAt", "completedAt", "promptTokens", "completionTokens", "totalTokens",
    )}
    trace = row.get("routeTrace") or {}
    safe["routeTrace"] = {name: trace[name] for name in (
        "routeRevision", "routeAttemptCount", "realCredentialRef", "selectedPipelineMode",
        "errorCode", "fallbackEligible", "cursorRemoteDeliveryUnknown", "requestedProtocolFamily",
    ) if name in trace}
    audits.append(safe)
print(json.dumps({"requests": audits}))
