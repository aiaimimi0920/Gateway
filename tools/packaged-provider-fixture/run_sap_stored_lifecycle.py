"""Isolated double-EXE, restart, cancellation and exact settlement acceptance."""
import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import secrets
import uuid

from run_sap_hosted import Product
from run_sensenova_hosted import LINSHI, setup
from sap_peer import AZURE, SAP
from sap_stored import DEPLOY, Stored
from sap_stored_cases import aggregate, exercise
from sap_stored_recovery_cases import exercise as recover
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require, sha256, sql


class StoredProduct(Product):
    def payload(self):
        value = super().payload()
        value["extra_body"]["sap_stored_responses"] = True
        return value


def run(package, evidence):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(),
            "Use a fresh evidence directory under linshi")
    package = package.resolve(strict=True)
    identity = package_identity(package)
    pg = fixture_url("SAP_AICORE_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url("SAP_AICORE_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    evidence.mkdir()
    # Only the label/port-verified dedicated fixture database is eligible.
    schema = package / "deploy/postgres/initdb"
    source_schema = Path(__file__).resolve().parents[2] / "deploy/postgres/initdb"
    identity["migrationSha256"] = {}
    for filename in ("008-gateway-sap-response-receipts.sql", "009-gateway-sap-response-mutations.sql",
                     "010-gateway-sap-response-route-pins.sql", "011-gateway-sap-response-recovery.sql"):
        require(sha256(schema / filename) == sha256(source_schema / filename), "Package migration mismatch")
        identity["migrationSha256"][filename] = sha256(schema / filename)
        sql(pg, (schema / filename).read_text(encoding="utf-8"))
    scope = "packaged-sap-aicore-stored-" + uuid.uuid4().hex
    management, signing = secrets.token_hex(32), secrets.token_hex(32)
    peer = SAP(evidence)
    peer.stored = Stored()
    hidden = [management, signing, peer.secret, peer.basic, peer.client_key, *peer.tokens.values()]
    instances = []
    stage = "startup"
    with peer.serve():
        try:
            with ExitStack() as stack:
                def start(label, *, ready=True):
                    instance = Backend(package, evidence / label, peer, scope, management, signing,
                        pg, redis, provider="SAP_AICORE", ca=None if ready else evidence / "absent.pem")
                    instances.append(instance)
                    stack.callback(instance.close, graceful=ready)
                    if ready:
                        instance.ready()
                    return instance
                guard = start("missing-ca", ready=False)
                require(guard.process.wait(timeout=15) != 0, "Missing fixture CA was not rejected")
                logs = b"".join(p.read_bytes() for p in (evidence / "missing-ca").glob("*.log"))
                require(b"SAP AI Core fixture CA must be an absolute file path" in logs, "Missing SAP isolation guard")
                writer, reader = start("writer"), start("reader")
                require(writer.process.pid != reader.process.pid and writer.port != reader.port, "Not independent backends")
                stage = "configure"
                key = peer.service_key(False)
                hidden.append(key)
                product = StoredProduct(AZURE, DEPLOY, "responses")
                account, credential, normal, _ = setup(writer, scope, key, hidden, product)
                owner, sources = aggregate(writer, pg, scope, normal, hidden)
                saved = reader.management(f"provider-accounts/{account}/credentials", secrets=hidden)
                require(saved["providerAccount"]["id"] == account and any(
                    row["id"] == credential and row["providerAccountId"] == account
                    for row in saved["credentials"]), "Reader did not load the exact stored credential")
                stage = "lifecycle"
                checks = exercise(writer, reader, lambda: start("restarted"), peer, pg,
                                  scope, owner, sources, account, credential)
                stage = "recovery"
                checks["recovery"] = recover(instances, start, peer, pg, scope, owner, sources)
                stage = "drain"
                for instance in instances:
                    if instance.process.poll() is None:
                        instance.drain()
                stage = "redaction"
                for log in evidence.rglob("*.log"):
                    require(not any(value.encode() in log.read_bytes() for value in hidden),
                            "A backend log exposed synthetic credentials")
        except Exception:
            (evidence / "failure.json").write_text(json.dumps(dict(identity, status="failed", stage=stage,
                calls=peer.stored.all_attempts(), exits=[p.process.poll() for p in instances]), indent=2) + "\n", encoding="utf-8")
            raise
    result = dict(identity, status="passed", scope=scope, checks=checks,
        pids=[p.process.pid for p in instances], exits=[p.process.poll() for p in instances],
        storedCalls=peer.stored.snapshot(), storedAttempts=peer.stored.all_attempts(),
        peerCalls=peer.snapshot()[0], gracefulDrain=True)
    (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    run(args.package, args.evidence)
