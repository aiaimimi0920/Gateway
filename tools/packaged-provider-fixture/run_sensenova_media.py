"""Opt-in dual-EXE SenseNova Chat image and reasoning/tool history acceptance."""
import argparse
from contextlib import ExitStack
import json
from pathlib import Path
import secrets
import threading
import time
import uuid

from run_sensenova_hosted import LINSHI, assert_public, check_audits, check_balance, setup
from sensenova import HOST, MODEL, PATH, certificate_files
from sensenova_media import PRODUCT, MediaPeer, check_first_reply, image_request, invalid_requests, reasoning_request
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require, sql


def check_failed_audits(pg, account, credential, key_id, alias):
    # Positive rows were checked before negatives. Inspect the entire namespace, not just correct rows.
    query = ("SELECT count(*), count(*) FILTER (WHERE status = 'completed'), "
             "count(*) FILTER (WHERE status = 'failed' AND route_attempt_count = 0 "
             f"AND access_key_id = '{key_id}' AND requested_model = '{alias}' "
             "AND COALESCE(provider_account_id, '') = '' "
             "AND route_trace->>'realCredentialRef' IS NULL "
             "AND route_trace->'attemptedProviderIds' = '[]'::jsonb "
             "AND jsonb_array_length(route_trace->'rejectedCandidates') = 1 "
             f"AND route_trace->'rejectedCandidates'->0->>'providerAccountId' = '{account}' "
             f"AND route_trace->'rejectedCandidates'->0->>'realCredentialRef' = '{credential}' "
             f"AND route_trace->'rejectedCandidates'->0->>'resolvedModel' = '{MODEL}' "
             "AND route_trace->'rejectedCandidates'->0->>'reason' IN "
             "('sensenova_chat_image_unsupported', 'sensenova_chat_reasoning_history_invalid')) "
             ", count(*) FILTER (WHERE status = 'failed' AND route_trace->'rejectedCandidates'->0->>'reason' "
             "= 'sensenova_chat_image_unsupported') "
             "FROM gateway_request_audits "
             f"WHERE access_key_id = '{key_id}' OR requested_model = '{alias}'")
    end = time.monotonic() + 12
    while time.monotonic() < end:
        summary = sql(pg, query)
        if summary == "8|4|4|2":
            return
        time.sleep(0.3)
    raise RuntimeError(f"Unexpected SenseNova rejection audit counts {summary}")


def run(package, evidence):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), "Use a new linshi directory")
    package = package.resolve(strict=True)
    identity = package_identity(package)
    pg = fixture_url("SENSENOVA_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url("SENSENOVA_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    evidence.mkdir()
    alias = PRODUCT.prefix + uuid.uuid4().hex
    management, signing = secrets.token_hex(32), secrets.token_hex(32)
    hidden, instances = [management, signing], {}
    stage = "startup"
    try:
        with ExitStack() as stack:
            peer = stack.enter_context(MediaPeer(evidence))
            thread = threading.Thread(target=peer.serve_forever)
            thread.start()
            stack.callback(thread.join, 5)
            stack.callback(peer.shutdown)
            hidden.append(peer.key)

            def start(label, ca=None, ready=True):
                backend = Backend(package, evidence / label, peer, alias, management, signing, pg, redis, ca=ca)
                instances[label] = backend
                stack.callback(backend.close, graceful=ready)
                if ready:
                    backend.ready()
                return backend

            guard = start("missing-ca", evidence / "absent.pem", False)
            require(guard.process.wait(timeout=15) != 0, "Package did not reject missing fixture CA")
            logs = b"".join(p.read_bytes() for p in (evidence / "missing-ca").glob("*.log"))
            require(b"SenseNova fixture CA must be an absolute file path" in logs and peer.post_attempts == 0,
                    "Package did not prove fixture isolation before setup")
            writer, reader = start("writer"), start("reader")
            require(writer.process.pid != reader.process.pid and writer.port != reader.port, "Expected two backend instances")
            stage = "configure"
            account, credential, key_id, token = setup(writer, alias, peer.key, hidden, PRODUCT)
            rows = reader.management(f"provider-accounts/{account}/credentials", secrets=hidden)
            require(credential in json.dumps(rows), "Reader did not see persisted credential")

            def call(body, expected=200, backend=reader):
                return backend.request("/v1/chat/completions", body, expected=expected, bearer=token, secrets=hidden)

            stage = "images"
            for stream in (False, True):
                text, kind = call(image_request(alias, stream))
                assert_public("chat/completions", stream, alias, text, kind, MODEL)
            stage = "reasoning-first-json"
            text, kind = call(reasoning_request(alias))
            assistant = check_first_reply(alias, text, kind)
            stage = "reasoning-followup-sse"
            text, kind = call(reasoning_request(alias, assistant))
            assert_public("chat/completions", True, alias, text, kind, MODEL)
            expected_calls = [dict(host=HOST, path=PATH, model=MODEL, stream=stream)
                              for stream in (False, True, False, True)]
            require(peer.calls() == expected_calls and peer.post_attempts == 4, "Unexpected wire or duplicate sends")
            stage = "positive-audits-and-balance"
            check_audits(pg, account, credential, key_id, alias, MODEL, 4)
            check_balance(pg, key_id, 4)
            stage = "invalid-inputs"
            for index, (body, error) in enumerate(invalid_requests(alias), 1):
                stage = f"invalid-input:{index}"
                text, _ = call(body, 400)
                require(json.loads(text)["error"]["code"] == error and peer.post_attempts == 4,
                        "Invalid input was not rejected before sending")
                check_balance(pg, key_id, 4)
            check_failed_audits(pg, account, credential, key_id, alias)
            require(sql(pg, "SELECT status, failure_count, cooldown_until IS NULL "
                        f"FROM gateway_provider_credentials WHERE id = '{credential}'") == "active|0|t",
                    "Local input rejection poisoned the healthy credential")
            stage = "wrong-ca"
            root = evidence / "wrong-root"
            root.mkdir()
            ca, _, _ = certificate_files(root, HOST)
            wrong = start("untrusted", ca)
            rejected = peer.tls_rejections
            call(image_request(alias, False), (500, 502, 503, 504), wrong)
            require(peer.tls_rejections > rejected and peer.post_attempts == 4 and peer.calls() == expected_calls,
                    "Wrong-CA request reached inference HTTP")
            check_balance(pg, key_id, 4)
            for backend in (wrong, reader, writer):
                backend.drain()
            stage = "redaction"
            for log in evidence.rglob("*.log"):
                require(not any(secret.encode() in log.read_bytes() for secret in hidden), "Log exposed fixture credentials")
        require(not thread.is_alive(), "Peer accept thread did not stop")
    except Exception:
        (evidence / "failure.json").write_text(json.dumps(dict(identity, status="failed", scope=alias, stage=stage,
            postAttempts=peer.post_attempts if "peer" in locals() else None,
            cleanup={name: {"exitCode": item.process.poll(), "drainFailure": item.cleanup_error}
                     for name, item in instances.items()}), indent=2) + "\n", encoding="utf-8")
        raise
    result = dict(identity, status="passed", scope=alias, completedPublicAudits=4, invalidInputsRejected=4,
                  postAttempts=peer.post_attempts, peerCalls=expected_calls, netDebitedMessages=4,
                  actualAssistantRoundTrip=True, missingCaRejected=True, wrongCaRejected=True,
                  rejectionAuditsVerified=True, credentialHealthPreserved=True,
                  pids={name: item.process.pid for name, item in instances.items()},
                  exits={name: item.process.poll() for name, item in instances.items()}, gracefulDrain=True)
    (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    run(args.package, args.evidence)
