"""Verify read-only SAP deployment discovery through an isolated packaged Gateway."""
import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import secrets
import uuid

from run_sensenova_hosted import LINSHI
from sap_catalog import PATH, PROTOCOLS
from sap_peer import AUTH, AWS, AZURE, CERT_AUTH, GROUP, SAP
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require

ROUTE = "console/sap-aicore-deployments"


def verify_catalog(backend, peer, hidden, grant):
    expected, results = [], {}
    for protocol, certificate, host, auth, offsets in (
        ("azure", False, AZURE, AUTH, (0, 4)),
        ("bedrock", True, AWS, CERT_AUTH, (0,)),
        ("orchestration", False, AZURE, AUTH, (0,)),
    ):
        body = {"resourceGroup": GROUP, "serviceKey": peer.service_key(certificate)}
        if protocol != "azure":
            body["protocol"] = protocol
        catalog = backend.management(ROUTE, body, grant=grant, secrets=hidden)
        expected.append({"host": auth, "path": "/oauth/token", "stream": None})
        expected.extend({"host": host, "path": f"{PATH}:{protocol}:{offset}", "stream": None} for offset in offsets)
        want = [f"catalog-{protocol}-{suffix}" for suffix in ("a", "b")[:len(offsets)]]
        rows = catalog["deployments"]
        require(catalog["truncated"] is False and [row["id"] for row in rows] == want,
                f"{protocol}: wrong list, pagination or protocol/origin/state filtering")
        for row in rows:
            require(row["deploymentUrl"] == f"https://{host}/v2/inference/deployments/{row['id']}"
                    and row.get("modelName") == PROTOCOLS[protocol][2], f"{protocol}: wrong deployment binding")
        require(peer.snapshot()[0] == expected, f"{protocol}: wrong page request, unexpected retry or inference")
        results[protocol] = want
    return results


def run(package, evidence):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), "Use a new linshi subdirectory")
    package = package.resolve(strict=True)
    result = package_identity(package)
    pg = fixture_url("SAP_AICORE_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url("SAP_AICORE_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    evidence.mkdir()
    scope = "packaged-sap-aicore-catalog-" + uuid.uuid4().hex
    token, signing = secrets.token_hex(32), secrets.token_hex(32)
    peer = SAP(evidence)
    hidden = [token, signing, peer.client_id, peer.secret, peer.basic, peer.client_key,
              *peer.tokens.values(), peer.service_key(False), peer.service_key(True)]
    stage = "startup"
    try:
        with peer.serve(), ExitStack() as stack:
            def start(name, ca=None, graceful=True):
                backend = Backend(package, evidence / name, peer, scope, token, signing, pg, redis,
                                  ca=ca, provider="SAP_AICORE")
                stack.callback(backend.close, graceful=graceful)
                return backend

            guard = start("missing-ca", evidence / "absent.pem", graceful=False)
            require(guard.process.wait(timeout=15) != 0, "Package failed to reject missing SAP fixture CA")
            logs = b"".join(path.read_bytes() for path in (evidence / "missing-ca").glob("*.log"))
            require(b"SAP AI Core fixture CA must be an absolute file path" in logs, "No verified fixture trust guard")
            backend = start("catalog")
            backend.ready()
            stage = "auth-and-config"
            routes = evidence / "catalog" / "routes.yaml"
            original = routes.read_bytes()
            revision = backend.management("console/route-config", secrets=hidden)["routeConfig"]["revision"]
            body = {"resourceGroup": GROUP, "serviceKey": peer.service_key(False)}
            backend.request("/v1/internal/gateway/" + ROUTE, body, expected=401, secrets=hidden)
            backend.management(ROUTE, body, expected=403, secrets=hidden)
            grant = backend.management("console/session/confirm-secret-access", {"token": token}, secrets=hidden)["grant"]
            hidden.append(grant)
            forged = json.loads(body["serviceKey"])
            forged["clientid"] = "not-a-fixture-client"
            body["serviceKey"] = json.dumps(forged)
            error = backend.management(ROUTE, body, expected=400, secrets=hidden, grant=grant)
            require(error.get("error", {}).get("code") == "sap_aicore_fixture_request_invalid"
                    and peer.snapshot()[0] == [], "Refused discovery reached the peer")
            stage = "catalog"
            results = verify_catalog(backend, peer, hidden, grant)
            require(routes.read_bytes() == original and
                    backend.management("console/route-config", secrets=hidden)["routeConfig"]["revision"] == revision,
                    "Read-only discovery modified route configuration")
            backend.drain()
            stage = "redaction"
            for log in evidence.rglob("*.log"):
                data = log.read_bytes()
                require(not any(value.encode() in data for value in hidden), "Backend log exposed a fixture credential")
    except Exception:
        (evidence / "failure.json").write_text(json.dumps(dict(result, status="failed", stage=stage,
            peerCalls=peer.snapshot()[0]), indent=2) + "\n", encoding="utf-8")
        raise
    (evidence / "result.json").write_text(json.dumps(dict(result, status="passed", deployments=results,
        protections=["missing-ca", "unauthenticated", "no-secret-grant", "non-fixture-key"],
        peerCalls=peer.snapshot()[0], inferenceCalls=0, routeRevisionUnchanged=True, gracefulDrain=True),
        indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    run(args.package, args.evidence)
