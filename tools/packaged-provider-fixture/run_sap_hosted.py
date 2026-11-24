"""Prove SAP secret OAuth and direct X.509 OAuth from two real Hosted backends."""
import argparse
from contextlib import ExitStack
from dataclasses import dataclass
import json
from pathlib import Path
import secrets
import uuid

from hosted_product import ENDPOINTS, SENSENOVA_HOSTED
import sap_native as native
import sap_tools
from run_sensenova_hosted import LINSHI, assert_public, check_audits, check_balance, setup
from sap_hosted_audit import check_oauth_rejection, health
from sap_peer import AUTH, AWS, AZURE, CERT_AUTH, DEPLOY, GROUP, MODEL, SAP, VERSION
from sensenova import ANSWER, certificate_files
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require


@dataclass(frozen=True)
class Product:
    host: str
    profile = "sap_aicore_chat"
    deployment: str = DEPLOY
    mode: str = "chat"

    @property
    def peer(self):
        # Hosted routing must resolve the deployment ID, not the vendor model label.
        return self

    @property
    def model(self):
        return self.deployment.rsplit("/", 1)[-1]

    @property
    def endpoint_kinds(self):
        if self.mode in ("chat", "chat-image"):
            return ENDPOINTS
        endpoint = "responses" if self.mode in ("responses", "responses-image") else "chat/completions"
        return {endpoint: ENDPOINTS[endpoint]}

    def payload(self):
        mode = self.mode.removesuffix("-tools")
        if mode == "bedrock-image":
            mode = "bedrock"
        settings = {"chat": {"sap_api_version": VERSION},
                    "responses": {"sap_api_version": VERSION, "sap_native_responses": True},
                    "responses-image": {"sap_api_version": VERSION, "sap_native_responses": True,
                                        "sap_native_responses_image_input": True},
                    "chat-image": {"sap_api_version": VERSION, "sap_image_input": True},
                    "chat-reasoning": {"sap_api_version": native.O3_VERSION, "sap_o3_mini": True},
                    "orchestration": {"sap_orchestration_model": native.ORCHESTRATION_MODEL},
                    "orchestration-image": {"sap_orchestration_model": native.ORCHESTRATION_MODEL,
                                            "sap_orchestration_image_input": True},
                    "orchestration-reasoning": {"sap_orchestration_model": native.REASONING_MODEL,
                                                "sap_orchestration_reasoning": True},
                    "bedrock": {"sap_bedrock_converse": True,
                                "sap_bedrock_image_input": self.mode != "bedrock-tools"}}[mode]
        return {"adapter": "openai_compatible", "auth_mode": "sap_aicore_service_key",
                "base_url": f"https://{self.host}{self.deployment}", "supported_models": [self.model],
                "headers": {"AI-Resource-Group": GROUP}, "extra_body": settings}

    def public_cases(self):
        return ((endpoint, stream, turn) for endpoint in self.endpoint_kinds for stream in (False, True)
                for turn in (range(2) if self.mode in sap_tools.MODES else range(1)))

    def request(self, endpoint, alias, stream, assistant=None):
        body = SENSENOVA_HOSTED.request_body(endpoint, alias, stream)
        if self.mode in sap_tools.MODES:
            return sap_tools.public_request(body, self.mode, assistant, native)
        if self.mode == "responses":
            body.update(reasoning={"effort": "low"})
        elif self.mode == "responses-image" or (self.mode == "chat-image" and endpoint == "responses"):
            body["input"] = native.RESPONSES_IMAGE_INPUT
        elif self.mode == "chat-image":
            image = ({"type": "image", "source": {"type": "base64", "media_type": "image/png", "data": native.PNG}}
                     if endpoint == "messages" else
                     {"type": "image_url", "image_url": {"url": native.IMAGE_URL, "detail": "high"}})
            prompt = native.PROMPT + " via Messages" if endpoint == "messages" else native.PROMPT
            body["messages"][0]["content"] = [{"type": "text", "text": prompt}, image]
        elif self.mode == "chat-reasoning":
            body.update(reasoning_effort="high" if stream else "low", max_tokens=32)
        elif self.mode.startswith("orchestration"):
            body.pop("stream_options", None)
            if self.mode == "orchestration-image":
                body["messages"][0]["content"] = native.CHAT_IMAGE_INPUT
            elif self.mode == "orchestration-reasoning":
                body["reasoning_effort"] = "xhigh" if stream else "high"
                if stream:
                    require(assistant is not None, "Reasoning continuation requires the actual first reply")
                    body["messages"] += [assistant, {"role": "user", "content": "continue"}]
        elif self.mode == "bedrock":
            body["messages"][0]["content"] = [{"type": "text", "text": "inspect"}, {
                "type": "image_url", "image_url": {"url": f"data:image/png;base64,{native.PNG}"}}]
        return body

    def path(self, stream):
        if self.mode.startswith("orchestration"):
            return f"{self.deployment}/v2/completion"
        if self.mode.startswith("bedrock"):
            return f"{self.deployment}/{'converse-stream' if stream else 'converse'}"
        suffix = {"chat": "chat/completions", "responses": "responses", "orchestration": "v2/completion",
                  "chat-image": "chat/completions", "chat-reasoning": "chat/completions", "responses-image": "responses"}[self.mode]
        return f"{self.deployment}/{suffix}"


def lanes():
    return (("client-secret", False, AUTH, Product(AZURE)),
            ("x509", True, CERT_AUTH, Product(AWS)),
            ("native-responses", False, AUTH, Product(AZURE, native.NATIVE_RESPONSES, "responses")),
            ("orchestration-v2", False, AUTH, Product(AZURE, native.ORCHESTRATION, "orchestration")),
            ("bedrock-inline-png", True, CERT_AUTH, Product(AWS, native.BEDROCK, "bedrock")),
            ("chat-inline-png", False, AUTH, Product(AZURE, native.CHAT_IMAGE, "chat-image")),
            ("responses-inline-png", False, AUTH, Product(AZURE, native.RESPONSES_IMAGE, "responses-image")),
            ("chat-o3-mini", False, AUTH, Product(AZURE, native.CHAT_REASONING, "chat-reasoning")),
            ("orchestration-inline-png", False, AUTH, Product(AZURE, native.ORCHESTRATION_IMAGE, "orchestration-image")),
            ("orchestration-signed-reasoning", False, AUTH,
             Product(AZURE, native.ORCHESTRATION_REASONING, "orchestration-reasoning"))) + tuple(
        (mode, bedrock, CERT_AUTH if bedrock else AUTH,
         Product(AWS if bedrock else AZURE, f"{native.DEPLOY_ROOT}/fixture-{mode}", mode))
        for mode in sap_tools.MODES for bedrock in [mode.startswith("bedrock")])


def check_reasoning(text, stream):
    if not stream:
        assistant = json.loads(text)["choices"][0]["message"]
        require(assistant == {"role": "assistant", "content": ANSWER, "reasoning_content": native.REASONING},
                "Orchestration JSON lost signed reasoning or changed the assistant")
        return assistant
    frames = [json.loads(line[6:]) for line in text.splitlines()
              if line.startswith("data: ") and line != "data: [DONE]"]
    blocks = [block for frame in frames for choice in frame.get("choices", [])
              for block in choice.get("delta", {}).get("reasoning_content", [])]
    require(blocks == native.REASONING, "Orchestration SSE lost, changed or duplicated signed reasoning blocks")


def run(package, evidence):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), "Use a new linshi subdirectory")
    package = package.resolve(strict=True)
    identity = package_identity(package)
    pg = fixture_url("SAP_AICORE_TEST_DATABASE_URL", PG, 5432, "postgresql", "/gateway_fixture")
    redis = fixture_url("SAP_AICORE_TEST_REDIS_URL", REDIS, 6379, "redis", "/")
    evidence.mkdir()
    scope = "packaged-sap-aicore-" + uuid.uuid4().hex
    management = "fixture-management-" + secrets.token_hex(24)
    signing = secrets.token_hex(32)
    fixture = SAP(evidence)
    hidden = [management, signing, fixture.client_id, fixture.secret, fixture.basic,
              fixture.client_key, *fixture.tokens.values()]
    stage = "startup"

    def start(stack, label, ca=None, *, ready=True):
        backend = Backend(package, evidence / label, fixture, scope, management, signing,
                          pg, redis, ca=ca, provider="SAP_AICORE")
        stack.callback(backend.close, graceful=ready)
        if ready:
            backend.ready()
        return backend

    with fixture.serve():
        try:
            with ExitStack() as stack:
                guard = start(stack, "missing-ca", evidence / "absent.pem", ready=False)
                require(guard.process.wait(timeout=15) != 0, "Package did not reject a missing SAP fixture CA")
                messages = b"".join(path.read_bytes() for path in (evidence / "missing-ca").glob("*.log"))
                require(b"SAP AI Core fixture CA must be an absolute file path" in messages,
                        "Package did not prove the SAP startup guard; refusing to send any request")
                writer = start(stack, "writer")
                reader = start(stack, "reader")
                require(writer.process.pid != reader.process.pid and writer.port != reader.port,
                        "Expected two separate Hosted backend listeners")
                expected = []
                checks = []
                completed = {}
                for lane, certificate, auth, product in lanes():
                    stage = f"configure:{lane}"
                    key = fixture.service_key(certificate)
                    hidden.append(key)
                    alias = scope + "-" + lane
                    account, credential, key_id, token = setup(writer, alias, key, hidden, product)
                    cross_process = reader.management(f"provider-accounts/{account}/credentials", secrets=hidden)
                    require(credential in json.dumps(cross_process), "SAP credential was not visible to reader")
                    expected.append({"host": auth, "path": "/oauth/token", "stream": None})
                    cases = list(product.public_cases())
                    assistant = None
                    for endpoint, stream, turn in cases:
                        stage = f"public:{lane}:{endpoint}:{'sse' if stream else 'json'}:turn-{turn + 1}"
                        if product.mode in sap_tools.MODES and turn == 0:
                            assistant = None
                        body = product.request(endpoint, alias, stream, assistant)
                        text, kind = reader.request(f"/v1/{endpoint}", body, bearer=token, secrets=hidden)
                        if product.mode in sap_tools.MODES:
                            assistant = sap_tools.assert_public(product.mode, alias, text, kind, stream, turn == 1)
                        else:
                            assert_public(endpoint, stream, alias, text, kind, MODEL)
                        require(MODEL not in text and native.PRIVATE not in text and native.PNG not in text,
                                "Public response exposed private native content")
                        if product.mode == "orchestration-reasoning":
                            assistant = check_reasoning(text, stream)
                        expected.append({"host": product.host, "path": product.path(stream), "stream": stream})
                    require(fixture.snapshot()[0] == expected, "SAP unexpectedly reused or repeated OAuth or sent to the wrong wire")
                    attempts = [{k: call[k] for k in ("host", "path")} for call in expected if call["stream"] is not None]
                    require(fixture.inference_attempts() == attempts and fixture.oauth_attempts() == len(completed) + 1,
                            "SAP rejected or retried an extra HTTP request")
                    stage = f"audit:{lane}"
                    check_audits(pg, account, credential, key_id, alias, product.model, len(cases))
                    check_balance(pg, key_id, len(cases))
                    completed[lane] = len(cases)
                    if product.mode == "chat":
                        checks.append((lane, alias, token, account, credential, key_id, product.model))
                stage = "wrong-ca"
                other = evidence / "wrong-root"
                other.mkdir()
                wrong_root, _, _ = certificate_files(other, AUTH)
                wrong = start(stack, "untrusted", wrong_root)
                for lane, alias, token, account, credential, key_id, model in checks:
                    stage = f"wrong-ca:{lane}"
                    _, rejected = fixture.snapshot()
                    before = health(pg, account, credential, model)
                    credential_state = json.loads(before)["credential"]
                    require(credential_state is not None and credential_state[:3] == ["active", 0, None],
                            "Expected a healthy fixture credential before the wrong-CA request")
                    text, _ = wrong.request("/v1/chat/completions", SENSENOVA_HOSTED.request_body("chat/completions", alias, False),
                                            expected=502, bearer=token, secrets=hidden)
                    require(json.loads(text)["error"]["code"] == "sap_aicore_token_failed", "Expected SAP OAuth trust rejection")
                    calls, tls_rejections = fixture.snapshot()
                    require(calls == expected and tls_rejections > rejected, "Untrusted OAuth reached HTTP or no TLS failure was recorded")
                    require(fixture.inference_attempts() == attempts and fixture.oauth_attempts() == len(completed),
                            "Wrong-CA request reached SAP HTTP")
                    check_oauth_rejection(pg, account, credential, key_id, alias, model, completed[lane], before)
                wrong.drain()
                reader.drain()
                writer.drain()
                stage = "redaction"
                for log in evidence.rglob("*.log"):
                    data = log.read_bytes()
                    require(not any(value.encode() in data for value in hidden), "A backend log exposed synthetic credentials")
        except Exception:
            (evidence / "failure.json").write_text(json.dumps(dict(identity, status="failed", stage=stage,
                calls=fixture.snapshot()[0]), indent=2) + "\n", encoding="utf-8")
            raise
    result = dict(identity, status="passed", pids=[writer.process.pid, reader.process.pid], scope=scope,
                  peerCalls=expected, completedPublicAudits=sum(completed.values()), completedByLane=completed,
                  inferenceAttempts=attempts, oauthAttempts=fixture.oauth_attempts(), messageDebitsVerified=True,
                  gracefulDrain=True, missingCaRejected=True, wrongCaRejected=[x[0] for x in checks],
                  wrongCaZeroSendAuditAndNeutralHealthVerified=True)
    (evidence / "result.json").write_text(json.dumps(result, indent=2) + "\n", encoding="utf-8")
    print(f"Passed: {evidence / 'result.json'}")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Isolated packaged SAP Hosted acceptance")
    parser.add_argument("--package", type=Path, required=True)
    parser.add_argument("--evidence", type=Path, required=True)
    args = parser.parse_args()
    run(args.package, args.evidence)
