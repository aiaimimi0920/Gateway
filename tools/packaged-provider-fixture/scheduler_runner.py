"""Prove packaged provider scheduler sends and internal audit ownership."""
import argparse
from contextlib import ExitStack
import hashlib
import json
from pathlib import Path
import secrets
import threading
import time
import uuid

from scheduler_product import ANSWER
from server_process import Backend, PG, REDIS, docker, fixture_url, package_identity, require, sql

LINSHI = Path("C:/Users/Public/nas_home/AI/GameEditor/linshi")


def call(product, scope):
    return {"host": product.peer.host, "path": product.peer.post_paths[0], "model": scope, "stream": False}


def read_result(identity, scope):
    raw = docker("exec", REDIS, "redis-cli", "--raw", "GET", f"gw:credential-test-result:{identity}")
    if not raw:
        return None
    result = json.loads(raw)
    case = result["assessment"]["cases"]
    require((result["providerId"], result["credentialId"], result["status"]) == (scope, "scheduled", "passed")
            and result["assessment"].get("mode") == "automatic"
            and len(case) == 1 and case[0].get("correct") is True
            and case[0].get("answer") == ANSWER and case[0]["model"] == scope,
            "Unexpected automatic probe result")
    return result


def await_result(identity, scope, product, peer, processes, previous=None):
    count = 1 if previous is None else 2
    end = time.monotonic() + (25 if previous is None else 125)
    while time.monotonic() < end:
        require(all(instance.process.poll() is None for instance in processes), "Scheduler process exited")
        require(len(peer.calls()) <= count, "Scheduler sent unexpectedly often")
        latest = read_result(identity, scope)
        if latest and (previous is None or latest["checkedAt"] != previous["checkedAt"]):
            require(peer.calls() == [call(product, scope)] * count, "Unexpected scheduler wire observations")
            return latest
        time.sleep(1)
    raise RuntimeError(f"Timed out waiting for scheduled result {count}")


def audit(pg, scope, product):
    # Only a generated ASCII scope enters SQL, never response or runner input.
    clause = f"namespace = '{scope}'"
    rows = sql(pg, "SELECT concat_ws('|', product, provider_id, credential_id, model, status, upstream_status, "
               "prompt_tokens, completion_tokens, total_tokens, CASE WHEN completed_at IS NOT NULL THEN 't' ELSE 'f' END) "
               f"FROM gateway_internal_model_probe_audits WHERE {clause} ORDER BY created_at, id LIMIT 3")
    expected = f"{product.audit_product}|{scope}|scheduled|{scope}|completed|200|2|3|5|t"
    require(rows.splitlines() == [expected] * 2,
            "Expected exactly two completed internal probe audits with exact product, account and usage")
    require(sql(pg, f"SELECT COALESCE(EXTRACT(epoch FROM MAX(created_at)-MIN(created_at)),0) >= 59 "
                f"FROM gateway_internal_model_probe_audits WHERE {clause}") == "t",
            "Two audit admissions occurred within the same 60-second schedule window")
    # Neither account nor requested model may appear in public audit.
    require(sql(pg, f"SELECT count(*) FROM gateway_request_audits WHERE requested_model = '{scope}' OR provider_account_id = '{scope}'") == "0",
            "Internal probes must not enter public request audits")


def run(package, evidence, product):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), "Use a fresh evidence directory under linshi")
    package = package.resolve(strict=True)
    result = package_identity(package)
    pg = fixture_url(f"{product.env_prefix}_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url(f"{product.env_prefix}_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    require(sql(pg, "SELECT to_regclass('public.gateway_internal_model_probe_audits') IS NOT NULL") == "t",
            "Apply isolated PostgreSQL schema through 007 first")
    evidence.mkdir()
    scope = product.scope_prefix + uuid.uuid4().hex
    identity = hashlib.sha256(json.dumps([scope, scope, "scheduled"], separators=(",", ":")).encode()).hexdigest()
    token, signing = secrets.token_hex(32), secrets.token_hex(32)
    stage, processes, peer = "setup", {}, None
    try:
        with ExitStack() as stack:
            peer = stack.enter_context(product.peer(evidence, scope))
            serving = threading.Thread(target=peer.serve_forever)
            serving.start()
            stack.callback(serving.join, 5)
            stack.callback(peer.shutdown)

            def start(name, *, ca=None, ready=True, routes=None):
                process = Backend(package, evidence / name, peer, scope, token, signing, pg, redis,
                                  ca=ca, route_doc=routes, provider=product.env_prefix)
                processes[name] = process
                stack.callback(process.close, graceful=ready)
                if ready:
                    process.ready()
                return process

            stage = "startup-guard"
            guard = start("missing-ca", ca=evidence / "missing.pem", ready=False)
            require(guard.process.wait(timeout=15) != 0, "Package did not reject a missing fixture CA")
            logs = b"".join(path.read_bytes() for path in (evidence / "missing-ca").glob("*.log"))
            require(f"{product.display_name} fixture CA must be an absolute file path".encode() in logs and not peer.calls(),
                    "Startup did not enforce the fixture transport contract")
            stage = "two-live-schedulers"
            routes = product.routes(scope, peer.key)
            first = start("first", routes=routes)
            second = start("second", routes=routes)
            require(first.process.pid != second.process.pid and first.port != second.port,
                    "Expected different backend processes and listeners")
            initial = await_result(identity, scope, product, peer, [first, second])
            latest = await_result(identity, scope, product, peer, [first, second], initial)
            stage = "audit-and-restart"
            audit(pg, scope, product)
            first.drain()
            second.drain()
            restarted = start("restarted", routes=routes)
            ttl = int(docker("exec", REDIS, "redis-cli", "TTL", f"gw:credential-test-slot:{identity}"))
            require(0 < ttl <= 60,
                    "Restart must preserve the active schedule slot")
            require(read_result(identity, scope) == latest and peer.calls() == [call(product, scope)] * 2,
                    "Restart did not retain the latest result or sent an extra probe")
            restarted.drain()
            require(peer.calls() == [call(product, scope)] * 2, "Unexpected send during drain")
            audit(pg, scope, product)
            stage = "redaction"
            for path in evidence.rglob("*.log"):
                log = path.read_bytes()
                require(not any(value.encode() in log for value in (peer.key, token, signing)),
                        "A backend log contained a synthetic secret")
    except Exception:
        (evidence / "failure.json").write_text(json.dumps({**result, "status": "failed", "stage": stage,
            "scope": scope, "peerCalls": peer.calls() if peer else [],
            "processExit": {name: instance.process.poll() for name, instance in processes.items()}}, indent=2) + "\n", encoding="utf-8")
        raise
    (evidence / "result.json").write_text(json.dumps({**result, "status": "passed", "scope": scope,
        "pids": [first.process.pid, second.process.pid, restarted.process.pid], "peerCalls": [call(product, scope)] * 2,
        "completedInternalAudits": 2, "completedPublicAudits": 0, "missingCaRejected": True}, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


def main(product):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    run(args.package, args.evidence, product)
