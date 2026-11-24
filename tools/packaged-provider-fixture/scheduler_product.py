"""Fixed provider identities and exact synthetic scheduler request contracts."""
from dataclasses import dataclass
import json
import uuid

from sensenova import ANSWER, HOST, PATH, USAGE, Fixture

PROMPT = "only OK"


class ScheduledChatPeer(Fixture):
    def __init__(self, root, model):
        self.model = model
        super().__init__(root)

    def reply(self, request, stream, path):
        if stream or request.get("messages") != [{"role": "user", "content": PROMPT}]:
            return None
        return json.dumps({
            "id": "chatcmpl-fixture-" + uuid.uuid4().hex, "created": 1700000000,
            "object": "chat.completion", "model": self.model, "usage": USAGE,
            "choices": [{"index": 0, "message": {"role": "assistant", "content": ANSWER},
                         "finish_reason": "stop"}],
        }).encode(), "application/json"


class SenseNovaSchedulerPeer(ScheduledChatPeer):
    host = HOST
    post_paths = (PATH,)

    def reply(self, request, stream, path):
        if request.get("max_tokens") != 256 or request.get("n") != 1:
            return None
        return super().reply(request, stream, path)


@dataclass(frozen=True)
class SchedulerProduct:
    display_name: str
    env_prefix: str
    audit_product: str
    scope_prefix: str
    preset: str
    base_path: str
    peer: type[ScheduledChatPeer]

    def routes(self, scope, key):
        policy = {"automaticEnabled": True, "intervalMinutes": 1, "modelSelection": "selected",
                  "models": [scope], "cases": [{"id": "text", "name": "Text", "prompt": PROMPT,
                                              "expectedAnswer": ANSWER, "difficulty": 1, "enabled": True}]}
        return {"providers": [{"id": scope, "preset": self.preset,
                              "base_url": f"https://{self.peer.host}{self.base_path}", "explicit_credentials_only": True,
                              "credentials": [{"id": "scheduled", "api_key": key, "supported_models": [scope],
                                               "test_policy": policy}]}], "model_routes": []}


SENSENOVA = SchedulerProduct("SenseNova", "SENSENOVA", "sensenova_token_plan", "packaged-sensenova-",
                             "sensenova-token-plan", "/v1", SenseNovaSchedulerPeer)
