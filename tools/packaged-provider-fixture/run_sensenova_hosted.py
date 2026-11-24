"""Opt-in packaged Hosted acceptance against the dedicated first-three stores."""

import argparse
from contextlib import ExitStack
import json
import re
import secrets
import threading
import time
import uuid
from pathlib import Path
from urllib.parse import urlencode

from hosted_product import SENSENOVA_HOSTED
from sensenova import ANSWER, certificate_files
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require, response_error_code, sql
from systemone_fixture import ANSWERS, USAGE

PROJECT = "platform-default-project"
TENANT = "platform-default-tenant"
TERMINAL = {"chat/completions": "data: [DONE]", "messages": "event: message_stop", "responses": "event: response.completed"}
LINSHI = Path("C:/Users/Public/nas_home/AI/GameEditor/linshi")


def setup(first, alias, fixture_key, secret_values, product):
    upstream_model = product.peer.model
    account = first.management("provider-accounts", {
        "label": alias, "adapter": "openai_compatible", "protocolFamily": "openai",
        "protocolProfile": product.profile, "status": "active", "executionMode": "direct_http",
        "payload": product.payload(),
    })["providerAccount"]["id"]
    credentials = f"provider-accounts/{account}/credentials"
    credential = first.management(credentials, {
        "label": alias, "status": "active", "sourceKind": "manual", "syncMode": "manual",
        "payload": {"api_key": fixture_key, "supported_models": [upstream_model]},
    }, secrets=(fixture_key,))["providerCredential"]["id"]
    model_alias = first.management("model-aliases", {
        "alias": alias, "projectId": PROJECT, "scopeType": "project", "providerAccountId": account,
        "upstreamModel": upstream_model,
    })["modelAlias"]["alias"]
    require(model_alias == alias, "The Hosted alias was not persisted")
    access_ids = []
    for kind in product.endpoint_kinds.values():
        model = {"modelCode": alias, "endpointKind": kind, "upstreamModel": upstream_model}
        capability_id = first.management("access/provider-capabilities", dict(model, providerAccountId=account, enabled=True))["id"]
        access_ids.append(first.management("access/platform-access", dict(
            model, providerCapabilityId=capability_id, platformTier="standard", status="active", operatorWeight=1,
            routingPriority=0, enabledForSale=True))["id"])
    bundle = first.management("access/bundles", {
        "projectId": PROJECT, "slug": alias, "displayName": alias, "status": "active", "billingMode": "message_prepaid",
    })["id"]
    first.management(f"access/bundles/{bundle}/items/replace", {"platformAccessIds": access_ids})
    key = first.management("access/keys", {
        "ownerType": "user", "ownerId": alias, "resolvedProjectId": PROJECT, "resolvedTenantId": TENANT,
        "keyKind": "normal", "publicKeyPrefix": "neuro", "displayName": alias, "bundleIds": [bundle],
    })
    secret_values.append(key["token"])
    # Fund the positive matrix and the separate wrong-CA admission.
    funded = len(list(product.public_cases())) + 1
    first.management(f"access/keys/{key['id']}/balances/adjust", {
        "balanceMode": "message_prepaid", "status": "active", "totalMessages": funded, "remainingMessages": funded,
    }, secrets=secret_values)
    return account, credential, key["id"], key["token"]


def assert_public(endpoint, stream, alias, text, content_type, upstream_model):
    require(upstream_model not in text, f"{endpoint}: public response exposed the upstream model")
    if endpoint == "systemone":
        reply = json.loads(text)
        require(not stream and content_type == "application/json" and reply.get("model") == alias
                and reply.get("answers") == ANSWERS and reply.get("usage") == USAGE and "choices" not in reply,
                "System One: wrong decision, model, usage or wire format")
        return
    require(ANSWER in text, f"{endpoint}: public answer missing")
    if stream:
        lines = [line for line in text.splitlines() if line]
        terminal = TERMINAL[endpoint]
        tail = -1 if endpoint == "chat/completions" else -2
        require(content_type == "text/event-stream" and alias in text and lines.count(terminal) == 1
                and len(lines) >= -tail and lines[tail] == terminal,
                f"{endpoint}: missing, duplicate or nonfinal SSE terminal")
        if endpoint == "chat/completions":
            frames = [json.loads(line[6:]) for line in lines if line.startswith("data: ") and line != terminal]
            require(frames and all(frame.get("model") == alias for frame in frames),
                    "Chat SSE frames did not consistently report the public alias")
            require(not any("error" in frame for frame in frames), "Chat SSE contained an error frame")
            choices = [choice for frame in frames for choice in frame.get("choices", [])]
            require(all(choice.get("index") == 0 for choice in choices)
                    and "".join(choice.get("delta", {}).get("content") or "" for choice in choices) == ANSWER
                    and [choice["finish_reason"] for choice in choices if choice.get("finish_reason") is not None] == ["stop"],
                    "Chat SSE changed the exact answer or completion reason")
            usages = [frame["usage"] for frame in frames if frame.get("usage") is not None]
            require(len(usages) == 1 and all(usages[0].get(key) == value for key, value in
                    (("prompt_tokens", 2), ("completion_tokens", 3), ("total_tokens", 5))),
                    "Chat SSE did not report exactly one complete 2/3/5-token usage")
    else:
        reply = json.loads(text)
        require(content_type == "application/json" and reply["model"] == alias,
                f"{endpoint}: wrong JSON content type or model")
        usage = reply["usage"]
        fields = ("prompt_tokens", "completion_tokens") if endpoint == "chat/completions" else ("input_tokens", "output_tokens")
        require(tuple(usage.get(field) for field in fields) == (2, 3), f"{endpoint}: wrong public usage")
        if endpoint == "chat/completions":
            choices = reply.get("choices", [])
            require(len(choices) == 1 and choices[0].get("index") == 0
                    and choices[0].get("message", {}).get("role") == "assistant"
                    and choices[0]["message"].get("content") == ANSWER
                    and choices[0].get("finish_reason") == "stop" and usage.get("total_tokens") == 5,
                    "Chat JSON changed the exact answer, completion reason or total usage")


def check_audits(pg, account, credential, key_id, alias, upstream_model, expected):
    require(all(re.fullmatch(r"[A-Za-z0-9_-]+", value) for value in (account, credential, key_id, alias))
            and re.fullmatch(r"[A-Za-z0-9_./-]+", upstream_model),
            "Identifiers cannot be used for the fixture audit query")
    where = f"FROM gateway_request_audits WHERE requested_model = '{alias}' OR access_key_id = '{key_id}'"
    verified = ("status = 'completed' AND prompt_tokens = 2 AND completion_tokens = 3 AND total_tokens = 5 "
                f"AND route_attempt_count = 1 AND requested_model = '{alias}' AND resolved_model = '{upstream_model}' AND provider_account_id = '{account}' "
                f"AND route_trace->>'realCredentialRef' = '{credential}' AND access_key_id = '{key_id}'")
    query = f"SELECT count(*), count(*) FILTER (WHERE {verified}) {where}"
    end = time.monotonic() + 15
    while time.monotonic() < end:
        count, exact = map(int, sql(pg, query).split("|"))
        require(count <= expected, "Duplicate public audits")
        if (count, exact) == (expected, expected):
            break
        time.sleep(0.5)
    else:
        raise RuntimeError(f"Expected exactly {expected} completed 2/3/5-token public audits; got {count}/{exact}")
    require(sql(pg, f"SELECT count(*) FROM gateway_internal_model_probe_audits WHERE provider_id = '{account}'") == "0",
            "Public calls unexpectedly wrote internal probe audits")


def check_balance(pg, key_id, completed):
    require(re.fullmatch(r"[A-Za-z0-9_-]+", key_id), "Invalid fixture balance key")
    query = ("SELECT total_messages, remaining_messages FROM gateway_access_key_balances "
             f"WHERE access_key_id = '{key_id}' AND balance_mode = 'message_prepaid'")
    require(sql(pg, query) == f"{completed + 1}|1", "Successful calls did not debit exactly one message each")


def run(package, evidence, product=SENSENOVA_HOSTED):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), "Use a fresh evidence directory under linshi")
    package = package.resolve(strict=True)
    result = package_identity(package)
    pg = fixture_url(f"{product.env}_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url(f"{product.env}_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    require(sql(pg, "SELECT to_regclass('public.gateway_internal_model_probe_audits') IS NOT NULL") == "t", "Apply isolated PostgreSQL schema through 007 first")
    evidence.mkdir()
    alias = product.prefix + uuid.uuid4().hex
    management = "fixture-management-" + secrets.token_hex(24)
    signing_secret = secrets.token_hex(32)
    progress = {"status": "failed", "stage": "starting", "candidatePreview": {}}
    instances = {}
    secret_values = [management, signing_secret]

    def record_failure():
        if progress["stage"] == "verified":
            return
        progress["peerCalls"] = peer.calls()
        progress["tlsRejections"] = peer.tls_rejections
        progress["cleanup"] = {label: {"exitCode": instance.process.poll(), "drainFailure": instance.cleanup_error}
                               for label, instance in instances.items()}
        (evidence / "failure.json").write_text(json.dumps({**result, **progress}, indent=2) + "\n", encoding="utf-8")

    with ExitStack() as stack:
        peer = stack.enter_context(product.peer(evidence))
        serving = threading.Thread(target=peer.serve_forever)
        serving.start()
        stack.callback(serving.join, 5)
        stack.callback(peer.shutdown)
        stack.callback(record_failure)
        secret_values.append(peer.key)

        def check_logs():
            for path in evidence.rglob("*.log"):
                require(not any(value.encode() in path.read_bytes() for value in secret_values),
                        "A backend log contained a fixture secret")
            if progress["stage"] == "check-logs":
                progress["stage"] = "verified"

        stack.callback(check_logs)

        def start(label, ca=None, *, ready=True):
            instance = Backend(package, evidence / label, peer, alias, management, signing_secret, pg, redis, ca=ca, provider=product.env)
            instances[label] = instance
            stack.callback(instance.close, graceful=ready)
            if ready:
                instance.ready()
            return instance

        # Before creating an account or sending inference, prove the package actually
        # enforces the fixture options. A stale binary must never hit the real origin.
        guard = start("missing-ca", evidence / "nonexistent.pem", ready=False)
        require(guard.process.wait(timeout=15) != 0, "Package did not reject a missing fixture CA")
        diagnostics = b"".join(path.read_bytes() for path in (evidence / "missing-ca").glob("*.log"))
        require(f"{product.name} fixture CA must be an absolute file path".encode() in diagnostics,
                "Package did not prove the fixture startup guard")
        require(not peer.calls(), "Startup guard unexpectedly contacted the peer")
        progress["stage"] = "configure"
        first = start("configure")
        account, credential, key_id, token = setup(first, alias, peer.key, secret_values, product)
        progress["stage"] = "serve"
        second = start("serve")
        require(first.process.pid != second.process.pid and first.port != second.port, "Expected distinct backend processes and listeners")
        cross_process = second.management(f"provider-accounts/{account}/credentials", secrets=secret_values)
        require(credential in json.dumps(cross_process), "Second process could not see the configured credential")
        for endpoint, kind in product.endpoint_kinds.items():
            query = urlencode({"accessKeyId": key_id, "model": alias, "endpointKind": kind, "estimatedTokens": 32})
            candidates = second.management(f"access/preview/candidates?{query}", secrets=secret_values)
            preview = {"count": len(candidates), "availableByBalance": sum(row["availableByBalance"] for row in candidates)}
            progress["candidatePreview"][endpoint] = preview
            require(preview == {"count": 1, "availableByBalance": 1}, f"{endpoint}: expected exactly one funded route candidate")
        expected_calls = []
        for endpoint, stream in product.public_cases():
            progress["stage"] = f"public:{endpoint}:{'sse' if stream else 'json'}"
            text, kind = second.request(f"/v1/{endpoint}", product.request_body(endpoint, alias, stream),
                                        bearer=token, secrets=secret_values)
            progress["publicResponse"] = {"contentType": kind, "hasAlias": alias in text,
                                          "hasUpstreamModel": product.peer.model in text}
            if stream:
                lines = text.splitlines()
                progress["publicResponse"]["terminalCount"] = lines.count(TERMINAL[endpoint])
                progress["publicResponse"]["errorCodes"] = [code for line in lines if line.startswith("data: ")
                                                             if (code := response_error_code(line[6:]))]
            assert_public(endpoint, stream, alias, text, kind, product.peer.model)
            expected_calls.append({"host": peer.host, "path": f"{product.base_path}/{endpoint}", "model": peer.model, "stream": stream})
        require(peer.calls() == expected_calls, "Unexpected peer request order, model, path, streaming mode or duplicate send")
        progress["stage"] = "public-audits"
        check_audits(pg, account, credential, key_id, alias, product.peer.model, len(expected_calls))
        if "systemone" in product.endpoint_kinds:
            # The completed audit and the persisted message debit are separate invariants.
            query = ("SELECT total_messages, remaining_messages FROM gateway_access_key_balances "
                     f"WHERE access_key_id = '{key_id}' AND balance_mode = 'message_prepaid'")
            require(sql(pg, query) == "2|1", "System One success did not debit exactly one message")
            result["remainingMessagesAfterSuccess"] = 1
        second.drain()
        progress["stage"] = "wrong-ca"
        other = evidence / "wrong-root"
        other.mkdir()
        wrong_ca, _, _ = certificate_files(other, peer.host)
        wrong = start("untrusted", wrong_ca)
        rejected = peer.tls_rejections
        endpoint = next(iter(product.endpoint_kinds))
        wrong.request(f"/v1/{endpoint}", product.request_body(endpoint, alias, False),
                      expected=(500, 502, 503, 504), bearer=token, secrets=secret_values)
        require(peer.tls_rejections > rejected and peer.calls() == expected_calls,
                "Expected a TLS rejection without an inference request")
        wrong.drain()
        first.drain()
        progress["stage"] = "check-logs"
    result.update(status="passed", scope=alias, pids=[first.process.pid, second.process.pid],
                  ports=[first.port, second.port], peerCalls=expected_calls, completedPublicAudits=len(expected_calls),
                  missingCaRejected=True, wrongCaRejected=True, candidatePreview=progress["candidatePreview"])
    (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


def main(product=SENSENOVA_HOSTED):
    parser = argparse.ArgumentParser(description=f"Packaged Hosted {product.name} fixture")
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    run(args.package, args.evidence, product)


if __name__ == "__main__":
    main()
