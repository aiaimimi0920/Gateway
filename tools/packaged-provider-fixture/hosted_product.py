"""Fixed product profiles for Hosted acceptance; never accept caller-selected targets."""
from dataclasses import dataclass
import json
import uuid

import commandcode
from native_replies import event_stream, messages, responses
from sensenova import ANSWER, HOST, MODEL, Fixture, build_reply
from systemone_fixture import SystemOnePeer, decision_body

ENDPOINTS = {"chat/completions": "chat_completions", "messages": "messages", "responses": "responses"}
PROMPT = "packaged fixture"
COMMANDCODE_MODEL = "opaque-commandcode-fixture-model"
CHAT_IMAGE_PARTS = (
    {"type": "text", "text": PROMPT},
    {"type": "image_url", "image_url": {"url": "https://image.invalid/fixture.png", "detail": "high"}},
)


class CommandCodeHostedPeer(Fixture):
    host = commandcode.HOST
    model = COMMANDCODE_MODEL
    key_prefix = commandcode.CommandCodePeer.key_prefix
    post_paths = tuple(f"/provider/v1/{endpoint}" for endpoint in ENDPOINTS)

    def reply(self, request, stream, path):
        if path == self.post_paths[0]:
            if request.get("messages") != [{"role": "user", "content": list(CHAT_IMAGE_PARTS)}]:
                return None
            return build_reply(stream, model=self.model)
        if path == self.post_paths[1]:
            if request.get("messages") != [{"role": "user", "content": PROMPT}]:
                return None
            result = messages(self.model, ANSWER, stream, uuid.uuid4().hex)
        elif path == self.post_paths[2]:
            if request.get("input") != PROMPT or request.get("store") is not False:
                return None
            result = responses(self.model, ANSWER, stream, uuid.uuid4().hex)
        else:
            return None
        return ((event_stream(result), "text/event-stream") if stream else
                (json.dumps(result).encode(), "application/json"))


@dataclass(frozen=True)
class HostedProduct:
    name: str
    env: str
    prefix: str
    profile: str
    auth_mode: str
    base_path: str
    peer: type[Fixture]
    declared_catalogue: bool = False

    @property
    def endpoint_kinds(self):
        return {"systemone": "systemone"} if self.peer is SystemOnePeer else ENDPOINTS

    def public_cases(self):
        for endpoint in self.endpoint_kinds:
            for stream in (False,) if endpoint == "systemone" else (False, True):
                yield endpoint, stream

    def payload(self):
        model = self.peer.model
        url = f"https://{self.peer.host}{self.base_path}"
        result = {"adapter": "openai_compatible", "auth_mode": self.auth_mode,
                  "base_url": url, "supported_models": [model]}
        if self.declared_catalogue:
            result["model_catalog"] = {"sourceUrl": f"{url}/models", "checkedAt": "2026-11-23T00:00:00Z",
                                       "models": [model], "supportedEndpoints": {model: [f"/{k}" for k in self.endpoint_kinds]}}
        return result

    def request_body(self, endpoint, alias, stream):
        if endpoint == "systemone" and not stream:
            return decision_body(alias)
        request = {"model": alias, "stream": stream}
        if endpoint == "responses":
            return dict(request, input=PROMPT, store=False)
        content = list(CHAT_IMAGE_PARTS) if self.declared_catalogue and endpoint == "chat/completions" else PROMPT
        request["messages"] = [{"role": "user", "content": content}]
        if endpoint == "messages":
            request["max_tokens"] = 32
        if endpoint == "chat/completions" and stream:
            request["stream_options"] = {"include_usage": True}
        return request


SENSENOVA_HOSTED = HostedProduct("SenseNova", "SENSENOVA", "packaged-sensenova-", "sensenova_token_plan",
                                "sensenova_token_plan_key", "/v1", Fixture)
COMMANDCODE_HOSTED = HostedProduct("Command Code", "COMMANDCODE", "packaged-commandcode-", "commandcode_chat",
                                  "commandcode_api_key", "/provider/v1", CommandCodeHostedPeer, True)
COMMANDCODE_SYSTEMONE = HostedProduct("Command Code", "COMMANDCODE", "packaged-commandcode-", "commandcode_chat",
                                     "commandcode_api_key", "/provider/v1", SystemOnePeer, True)
