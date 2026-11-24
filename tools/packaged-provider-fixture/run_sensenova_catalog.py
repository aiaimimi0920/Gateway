"""Isolated package verification for the real SenseNova management catalog route."""
import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import secrets
import threading
import uuid

from sensenova import CATALOG_MODELS, HOST, Fixture
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require

LINSHI = Path("C:/Users/Public/nas_home/AI/GameEditor/linshi")
ROUTE = "console/sensenova-model-catalog"
POOL = "sensenova-catalog-fixture"
ACCOUNT = "saved-sensenova-fixture"


def seed(key):
    return {"providers": [{"id": POOL, "preset": "sensenova-token-plan", "base_url": f"https://{HOST}/v1",
                           "explicit_credentials_only": True, "credentials": [{"id": ACCOUNT, "api_key": key}]}],
            "model_routes": []}


def run(package, evidence):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), "Use a fresh evidence directory under linshi")
    package = package.resolve(strict=True)
    result = package_identity(package)
    pg = fixture_url("SENSENOVA_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url("SENSENOVA_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    evidence.mkdir()
    namespace = "packaged-sensenova-catalog-" + uuid.uuid4().hex
    token, signing = secrets.token_hex(32), secrets.token_hex(32)
    stage, peer = "setup", None
    try:
        with ExitStack() as stack:
            peer = stack.enter_context(Fixture(evidence))
            thread = threading.Thread(target=peer.serve_forever)
            thread.start()
            stack.callback(thread.join, 5)
            stack.callback(peer.shutdown)

            stage = "startup-guard"
            guard = Backend(package, evidence / "missing-ca", peer, namespace, token, signing, pg, redis,
                            ca=evidence / "missing.pem")
            stack.callback(guard.close)
            require(guard.process.wait(timeout=15) != 0, "Package did not reject a missing fixture CA")
            logs = b"".join(path.read_bytes() for path in (evidence / "missing-ca").glob("*.log"))
            require(b"SenseNova fixture CA must be an absolute file path" in logs,
                    "Startup did not prove the SenseNova trust guard")

            stage = "protected-catalog"
            backend = Backend(package, evidence / "catalog", peer, namespace, token, signing, pg, redis,
                              route_doc=seed(peer.key))
            stack.callback(backend.close, graceful=True)
            backend.ready()
            route_file = evidence / "catalog" / "routes.yaml"
            unchanged = route_file.read_bytes()
            saved = {"credentialId": ACCOUNT}
            backend.request("/v1/internal/gateway/" + ROUTE, saved, expected=401, secrets=(peer.key,))
            backend.management(ROUTE, saved, expected=403, secrets=(peer.key,))
            require(peer.catalog_calls == 0 and not peer.calls(), "Unauthorized request reached the peer")
            confirmation = backend.management("console/session/confirm-secret-access", {"token": token},
                                              secrets=(peer.key,))
            grant = confirmation["grant"]
            redacted = (peer.key, grant, signing)
            failure = backend.management(ROUTE, {"providerId": POOL, "apiKey": "cpk-not-a-fixture-key"},
                                         expected=400, secrets=redacted, grant=grant)
            require(failure.get("error", {}).get("code") == "sensenova_fixture_key_invalid"
                    and peer.catalog_calls == 0, "Fixture did not fail closed before network")
            before = backend.management("console/route-config", secrets=redacted)["routeConfig"]["revision"]
            for body in ({"providerId": POOL, "apiKey": peer.key}, saved):
                response = backend.management(ROUTE, body, secrets=redacted, grant=grant)
                require(response["revision"] == before["id"], "Catalog was not bound to the active revision")
                catalog = response["catalog"]
                require(catalog["sourceUrl"] == f"https://{HOST}/v1/models"
                        and catalog["models"] == list(CATALOG_MODELS), "Unexpected filtered directory")
            after = backend.management("console/route-config", secrets=redacted)["routeConfig"]["revision"]
            require(before == after and route_file.read_bytes() == unchanged, "Read-only lookup wrote route configuration")
            require(peer.catalog_calls == 2 and not peer.calls(), "Unexpected catalog or inference traffic")
            backend.drain()
            stage = "log-redaction"
            for path in evidence.rglob("*.log"):
                require(not any(value.encode() in path.read_bytes() for value in (token, *redacted)),
                        "Fixture log contained a secret")
    except Exception:
        (evidence / "failure.json").write_text(json.dumps({**result, "status": "failed", "stage": stage,
            "catalogCalls": peer.catalog_calls if peer else 0}, indent=2) + "\n", encoding="utf-8")
        raise
    (evidence / "result.json").write_text(json.dumps({**result, "status": "passed", "catalogCalls": 2,
        "inferenceCalls": 0, "routeRevisionUnchanged": True, "models": list(CATALOG_MODELS)}, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    run(args.package, args.evidence)
