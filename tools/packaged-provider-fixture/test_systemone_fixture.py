"""Check the decision peer's fixed request and public response contracts."""
import json
import unittest

from hosted_product import COMMANDCODE_SYSTEMONE
from run_sensenova_hosted import assert_public
from systemone_fixture import ANSWERS, USAGE, SystemOnePeer, decision_body


class SystemOneFixtureTest(unittest.TestCase):
    def test_exact_nonstreaming_decision(self):
        peer = object.__new__(SystemOnePeer)
        path, = peer.post_paths
        self.assertEqual(list(COMMANDCODE_SYSTEMONE.public_cases()), [("systemone", False)])
        request = COMMANDCODE_SYSTEMONE.request_body("systemone", peer.model, False)
        self.assertEqual(request, decision_body(peer.model))
        self.assertIsNone(peer.reply(request, True, path))
        self.assertIsNone(peer.reply(dict(request, stream=False), False, path))
        self.assertIsNone(peer.reply(request, False, f"{path}?extra=1"))
        response, content_type = peer.reply(request, False, path)
        self.assertEqual(json.loads(response), {"model": peer.model, "answers": ANSWERS, "usage": USAGE})
        rewritten = dict(json.loads(response), model="decision-alias")
        assert_public("systemone", False, "decision-alias", json.dumps(rewritten), content_type, peer.model)
        for wrong in (dict(rewritten, model=peer.model), dict(rewritten, usage={}), dict(rewritten, choices=[])):
            with self.assertRaises(RuntimeError):
                assert_public("systemone", False, "decision-alias", json.dumps(wrong), content_type, peer.model)
