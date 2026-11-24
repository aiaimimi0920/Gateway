"""Revoke the selected Hosted SAP credential during OAuth, before inference."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer
import json
from pathlib import Path
import secrets
import threading
import uuid

from hosted_product import SENSENOVA_HOSTED
from run_sap_hosted import Product
from run_sensenova_hosted import LINSHI, assert_public, check_audits, check_balance, setup
from sap_hosted_audit import safe_ids
from sap_peer import AUTH, AZURE, SAP
from sensenova import build_reply
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require, sql

MODEL = "ordinary-fixture-model"
ENDPOINT = "chat/completions"


class OrdinaryHandler(BaseHTTPRequestHandler):
    def setup(self):
        self.request.settimeout(4)
        super().setup()

    def log_message(self, *_):
        pass

    def do_POST(self):
        with self.server.lock:
            self.server.attempts += 1
            if self.path == "/v1/chat/completions":
                self.server.expected_paths += 1
        size = self.headers.get("Content-Length", "")
        if (self.path != "/v1/chat/completions" or not size.isascii() or not size.isdecimal()
                or len(size) > 5 or self.headers.get_all("Authorization") != ["Bearer " + self.server.key]):
            return self.send_error(400)
        try:
            body = json.loads(self.rfile.read(int(size)))
        except ValueError:
            return self.send_error(400)
        if not isinstance(body, dict) or body.get("model") != MODEL:
            return self.send_error(400)
        with self.server.lock:
            self.server.accepted += 1
        payload, kind = build_reply(body.get("stream") is True, model=MODEL)
        self.send_response(200)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(payload)))
        self.end_headers()
        self.wfile.write(payload)


def add_ordinary(writer, ordinary, alias, hidden):
    account = writer.management("provider-accounts", {
        "label": alias + "-ordinary", "adapter": "openai_compatible", "protocolFamily": "openai",
        "status": "active", "executionMode": "direct_http", "payload": {
            "adapter": "openai_compatible", "base_url": f"http://127.0.0.1:{ordinary.server_port}/v1",
            "supported_models": [MODEL]},
    })["providerAccount"]["id"]
    credential = writer.management(f"provider-accounts/{account}/credentials", {
        "label": alias + "-ordinary", "status": "active", "sourceKind": "manual", "syncMode": "manual",
        "payload": {"api_key": ordinary.key, "supported_models": [MODEL]},
    }, secrets=hidden)["providerCredential"]["id"]
    model = {"modelCode": alias, "endpointKind": "chat_completions", "upstreamModel": MODEL}
    capability = writer.management("access/provider-capabilities",
                                   dict(model, providerAccountId=account, enabled=True))["id"]
    access = writer.management("access/platform-access", dict(
        model, providerCapabilityId=capability, platformTier="standard", status="active", operatorWeight=1,
        routingPriority=-1, enabledForSale=True))["id"]
    return account, credential, access


class ChatProduct(Product):
    @property
    def endpoint_kinds(self):
        return {ENDPOINT: "chat_completions"}

    def public_cases(self):
        return ((ENDPOINT, False, 0),)


def exact_trace(pg, alias, key, sap, ordinary, upstream):
    safe_ids(alias, key, *sap, *ordinary, upstream)
    raw = sql(pg, f"SELECT route_trace FROM gateway_request_audits WHERE access_key_id='{key}' "
                  f"AND requested_model='{alias}' AND upstream_status=200")
    trace = json.loads(raw)
    require(trace["routeAttemptCount"] == 1 and trace["attemptedProviderIds"] == [ordinary[0]]
            and trace["realCredentialRef"] == ordinary[1], "Wrong actual route attribution")
    selected = trace["selectedCandidate"]
    require((selected["providerAccountId"], selected["realCredentialRef"], selected["resolvedModel"])
            == (*ordinary, MODEL), "Wrong selected identity")
    matching = [row for row in trace["rejectedCandidates"] if row["providerAccountId"] == sap[0]
                and row["realCredentialRef"] == sap[1]]
    require(len(matching) == 1 and matching[0]["reason"] == "upstream_credential_unavailable"
            and matching[0]["resolvedModel"] == upstream, "Missing exact SAP rejection")
    require(sql(pg, "SELECT count(*) FROM gateway_provider_credential_model_states "
                    f"WHERE provider_account_id='{sap[0]}' AND failure_count>0") == "0", "Unsent SAP was penalized")


def run(package, evidence):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), "Use a new linshi directory")
    package = package.resolve(strict=True)
    identity = package_identity(package)
    pg = fixture_url("SAP_AICORE_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url("SAP_AICORE_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    evidence.mkdir()
    scope = "packaged-sap-aicore-fallback-" + uuid.uuid4().hex
    token, signing = secrets.token_hex(32), secrets.token_hex(32)
    peer = SAP(evidence)
    key = peer.service_key(False)
    hidden = [token, signing, key, peer.secret, peer.client_id, peer.basic, *peer.tokens.values()]
    stages = []
    stage = "startup"
    try:
        with peer.serve(), ExitStack() as stack:
            ordinary = stack.enter_context(ThreadingHTTPServer(("127.0.0.1", 0), OrdinaryHandler))
            ordinary.key, ordinary.lock, ordinary.attempts = secrets.token_hex(32), threading.Lock(), 0
            ordinary.expected_paths = ordinary.accepted = 0
            hidden.append(ordinary.key)
            server = threading.Thread(target=ordinary.serve_forever)
            server.start()
            stack.callback(server.join, 5)
            stack.callback(ordinary.shutdown)

            def start(label, ready=True):
                backend = Backend(package, evidence / label, peer, scope, token, signing, pg, redis,
                                  ca=None if ready else evidence / "absent.pem", provider="SAP_AICORE",
                                  disable_system_proxy=True)
                stack.callback(backend.close, graceful=ready)
                if ready:
                    backend.ready()
                return backend

            guard = start("missing-ca", False)
            require(guard.process.wait(timeout=15) != 0 and any(
                b"SAP AI Core fixture CA must be an absolute file path" in p.read_bytes()
                for p in (evidence / "missing-ca").glob("*.log")), "Missing SAP fixture trust guard")
            writer = start("writer")
            product = ChatProduct(AZURE)
            for number, (label, stream) in enumerate((("json", False), ("sse", True)), 1):
                stage = "setup-" + label
                alias = scope + "-" + label
                account, credential, key_id, bearer = setup(writer, alias, key, hidden, product)
                fallback, exact, access = add_ordinary(writer, ordinary, alias, hidden)
                safe_ids(alias, account)
                bundle = sql(pg, f"SELECT id FROM gateway_access_bundles WHERE slug='{alias}'")
                primary = sql(pg, "SELECT id FROM gateway_platform_access_catalog "
                                  f"WHERE model_code='{alias}' AND endpoint_kind='chat_completions' AND provider_capability_id IN "
                                  f"(SELECT id FROM gateway_provider_capability_catalog WHERE provider_account_id='{account}')")
                safe_ids(bundle, primary)
                writer.management(f"access/bundles/{bundle}/items/replace", {"platformAccessIds": [primary, access]})
                reader = start(label)
                entered, release = threading.Event(), threading.Event()

                def barrier(handler):
                    entered.set()
                    if not release.wait(10):
                        handler.send_error(504)
                        return True
                    return False

                peer.oauth_override = barrier
                stage = "oauth-barrier-" + label
                body = SENSENOVA_HOSTED.request_body(ENDPOINT, alias, stream)
                with ThreadPoolExecutor(1) as pool:
                    future = pool.submit(reader.request, "/v1/" + ENDPOINT, body, bearer=bearer, secrets=hidden, timeout=20)
                    try:
                        require(entered.wait(8), "SAP did not reach the OAuth barrier")
                        require(peer.inference_attempts() == [] and ordinary.attempts == number - 1,
                                "Inference started before credential revocation")
                        updated = writer.management(f"provider-credentials/{credential}", {"status": "disabled"},
                                                    method="PUT", secrets=hidden, timeout=6)["providerCredential"]
                        require(updated["id"] == credential and updated["status"] == "disabled",
                                "Did not revoke selected credential")
                    finally:
                        release.set()
                    text, kind = future.result(timeout=20)
                peer.oauth_override = None
                stage = "audit-" + label
                assert_public(ENDPOINT, stream, alias, text, kind, MODEL)
                check_audits(pg, fallback, exact, key_id, alias, MODEL, 1)
                check_balance(pg, key_id, 1)
                exact_trace(pg, alias, key_id, (account, credential), (fallback, exact), product.model)
                require(peer.inference_attempts() == [] and ordinary.attempts == ordinary.accepted == number
                        and peer.oauth_attempts() == number
                        and peer.snapshot()[0] == [{"host": AUTH, "path": "/oauth/token", "stream": None}] * number,
                        "Unexpected OAuth, SAP inference or fallback replay")
                reader.drain()
                stages.append(label)
            writer.drain()
            for path in evidence.rglob("*.log"):
                require(not any(item.encode() in path.read_bytes() for item in hidden), "Fixture secret in runtime logs")
    except Exception:
        details = {k: getattr(ordinary, k) for k in ("attempts", "expected_paths", "accepted")} if "ordinary" in locals() else {}
        (evidence / "failure.json").write_text(json.dumps(dict(identity, status="failed", stage=stage,
            ordinary=details, oauthAttempts=peer.oauth_attempts(), sapInferenceAttempts=peer.inference_attempts())) + "\n",
            encoding="utf-8")
        raise
    result = dict(identity, status="passed", lanes=stages, oauthAttempts=peer.oauth_attempts(),
                  sapInferenceAttempts=0, ordinaryInferenceAttempts=ordinary.attempts,
                  publicAudits=2, messageDebits=2, exactRejectionAndAttribution=True, gracefulDrain=True)
    (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", required=True, type=Path)
    parser.add_argument("--evidence", required=True, type=Path)
    args = parser.parse_args()
    run(args.package, args.evidence)
