"""Fixed System One wire contract for the non-forwarding Hosted acceptance peer."""
import json

from commandcode import CommandCodePeer
from sensenova import Fixture

STATE = {"message": "synthetic refund request"}
QUESTIONS = {"urgent": {"type": "noul", "instructions": "Should we prioritize this?"}}
ANSWERS = {"urgent": {"type": "noul", "noul": 0.96}}
USAGE = {"input_tokens": 2, "output_tokens": 3}


def decision_body(model):
    return {"model": model, "state": STATE, "questions": QUESTIONS}


class SystemOnePeer(Fixture):
    host = CommandCodePeer.host
    key_prefix = CommandCodePeer.key_prefix
    model = "typesafe/jev"
    post_paths = ("/provider/v1/systemone",)

    def reply(self, request, stream, path):
        if stream or path not in self.post_paths or request != decision_body(self.model):
            return None
        reply = {"model": self.model, "answers": ANSWERS, "usage": USAGE}
        return json.dumps(reply).encode(), "application/json"
