"""Check packaged SAP management probes against a non-forwarding TLS peer."""
import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import secrets
import uuid

from run_sap_hosted import Product
from run_sensenova_hosted import LINSHI
from sap_hosted_audit import safe_ids
from sap_peer import AZURE, DEPLOY, GROUP, PROMPT, SAP, VERSION
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require, sha256, sql

PRIVATE = "sap-probe-fixture-private-error"


def rejection(status):
    def send(handler):
        body = json.dumps({"error": PRIVATE}).encode()
        handler.send_response(status)
        handler.send_header("Content-Type", "application/json")
        handler.send_header("Retry-After", "120")
        handler.send_header("Content-Length", str(len(body)))
        handler.end_headers()
        handler.wfile.write(body)
        return True
    return send


def run(package, evidence):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), "Use a new linshi directory")
    package = package.resolve(strict=True)
    identity = package_identity(package)
    pg = fixture_url("SAP_AICORE_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url("SAP_AICORE_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    name = "012-gateway-sap-probe-evidence.sql"
    migration = package / "deploy/postgres/initdb" / name
    source = Path(__file__).resolve().parents[2] / "deploy/postgres/initdb" / name
    digest = sha256(migration)
    require(digest == sha256(source), "SAP probe migration differs from source")
    evidence.mkdir()
    peer = SAP(evidence)
    token, signing = secrets.token_hex(32), secrets.token_hex(32)
    key = peer.service_key(False)
    hidden = [token, signing, key, peer.secret, peer.client_id, peer.basic, *peer.tokens.values(), PRIVATE]
    scope = "packaged-sap-aicore-probe-" + uuid.uuid4().hex
    product = Product(AZURE)
    model = product.model
    stage = "startup"
    rows = []
    try:
        with peer.serve(), ExitStack() as stack:
            def start(label, doc=None, ready=True):
                instance = Backend(package, evidence / label, peer, scope, token, signing, pg, redis,
                                   ca=None if ready else evidence / "absent.pem", route_doc=doc, provider="SAP_AICORE")
                stack.callback(instance.close, graceful=ready)
                if ready:
                    instance.ready()
                return instance

            guard = start("missing-ca", ready=False)
            require(guard.process.wait(timeout=15) != 0 and any(
                b"SAP AI Core fixture CA must be an absolute file path" in path.read_bytes()
                for path in (evidence / "missing-ca").glob("*.log")), "Missing SAP fixture trust guard")
            sql(pg, migration.read_text(encoding="utf-8"))
            writer = start("writer")
            for phase, status in (("oauth", 429), ("inference", 503)):
                stage = phase
                account = writer.management("provider-accounts", {"label": scope + "-" + phase,
                    "adapter": "openai_compatible", "protocolFamily": "openai", "protocolProfile": product.profile,
                    "status": "active", "executionMode": "direct_http", "payload": product.payload()})["providerAccount"]["id"]
                credential = "fixture-" + phase
                safe_ids(scope, account, credential, model)
                route = {"id": account, "preset": "sap-aicore-chat", "base_url": f"https://{AZURE}{DEPLOY}",
                         "explicit_credentials_only": True, "headers": {"AI-Resource-Group": GROUP},
                         "extra_body": {"sap_api_version": VERSION}, "supported_models": [model],
                         "credentials": [{"id": credential, "api_key": key, "supported_models": [model]}]}
                probe = start(phase, {"providers": [route], "model_routes": []})
                require(writer.process.pid != probe.process.pid, "Expected independent Hosted processes")
                grant = probe.management("console/session/confirm-secret-access", {"token": token}, secrets=hidden)["grant"]
                hidden.append(grant)
                endpoint = f"console/providers/{account}/probe"
                body = {"model": model, "prompt": PROMPT, "credentialIds": [credential]}
                before_oauth = peer.oauth_attempts()
                before_inference = peer.inference_attempts()
                probe.management(endpoint, body, expected=403, secrets=hidden)
                require(peer.oauth_attempts() == before_oauth and peer.inference_attempts() == before_inference,
                        "Missing secret grant reached SAP")
                peer.oauth_override = rejection(status) if phase == "oauth" else None
                peer.inference_override = rejection(status) if phase == "inference" else None
                reply = probe.management(endpoint, body, grant=grant, secrets=hidden)["result"]
                require(reply["status"] == "failed" and reply["totalCount"] == 1
                        and reply["results"][0]["credentialId"] == credential, "Wrong management probe result")
                probe.drain()
                sent = phase == "inference"
                observed = status if sent else None
                expected_calls = [{"host": AZURE, "path": f"{DEPLOY}/chat/completions"}] if sent else []
                require(peer.oauth_attempts() == before_oauth + 1
                        and peer.inference_attempts() == before_inference + expected_calls, "Probe replayed or sent to wrong phase")
                audit = json.loads(sql(pg, "SELECT coalesce(jsonb_agg(jsonb_build_object('status',status,"
                    "'upstream',upstream_status,'tokens',total_tokens,'evidence',sap_evidence,"
                    "'credential',credential_id,'model',model,'product',product)), '[]'::jsonb) "
                    "FROM gateway_internal_model_probe_audits "
                    f"WHERE namespace='{scope}' AND provider_id='{account}'"))
                code = "sap_aicore_upstream_rejected" if sent else "sap_aicore_token_rejected"
                expected = {"attempts": int(sent), "upstreamStatus": observed, "failure": {
                    "phase": phase, "code": code, "status": status, "retryAfterMs": 120000}}
                require(audit == [{"status": "failed", "upstream": observed, "tokens": None, "evidence": expected,
                                   "credential": credential, "model": model, "product": "sap_aicore"}],
                        "Probe terminal audit did not match exact phase evidence")
                health = sql(pg, "SELECT provider_credential_ref,model,failure_count,last_upstream_status "
                                 f"FROM gateway_provider_credential_model_states WHERE provider_account_id='{account}'")
                require(health == (f"{credential}|{model}|1|{status}" if sent else ""), "Probe health attribution differs")
                require(sql(pg, "SELECT count(*) FROM gateway_request_audits "
                                f"WHERE provider_account_id='{account}'") == "0", "Probe entered public request accounting")
                rows.append(dict(expected, provider=account, credential=credential, pid=probe.process.pid))
            writer.drain()
            for path in evidence.rglob("*.log"):
                require(not any(item.encode() in path.read_bytes() for item in hidden),
                        "A probe emitted fixture secret or private error")
    except Exception:
        (evidence / "failure.json").write_text(json.dumps(dict(identity, status="failed", stage=stage)) + "\n", encoding="utf-8")
        raise
    result = dict(identity, status="passed", migrationSha256=digest, probes=rows,
                  oauthAttempts=peer.oauth_attempts(), inferenceAttempts=peer.inference_attempts(), gracefulDrain=True)
    (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    run(args.package, args.evidence)
