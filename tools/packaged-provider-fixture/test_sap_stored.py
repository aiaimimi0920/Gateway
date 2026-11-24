"""Pure peer assertions including one-shot cancellation and predecessor identity."""
import unittest
from sap_stored import PATH, Stored


class StoredTest(unittest.TestCase):
    def test_terminal_without_usage_stays_unknown_on_get(self):
        peer = Stored()
        peer.complete_next_create(usage=False)
        code, created = peer.reply("POST", PATH, {"input": "fixture-stored", "store": True, "background": True})
        self.assertEqual((code, created["status"]), (200, "completed"))
        self.assertNotIn("usage", created)
        self.assertEqual(peer.reply("GET", PATH + "/" + created["id"], None), (200, created))

    def test_terminal_create_is_explicit_and_one_shot(self):
        peer = Stored()
        create = {"input": "fixture-stored", "store": True, "background": True}
        peer.complete_next_create()
        self.assertEqual(peer.reply("POST", PATH, dict(create, store=False))[0], 400)
        code, first = peer.reply("POST", PATH, create)
        self.assertEqual((code, first["status"], first["usage"]["total_tokens"]), (200, "completed", 5))
        code, second = peer.reply("POST", PATH, dict(create, previous_response_id=first["id"]))
        self.assertEqual((code, second["status"]), (200, "queued"))
        self.assertNotIn("usage", second)

    def test_lifecycle_and_previous_identity(self):
        peer = Stored()
        create = {"input": "fixture-stored", "store": True, "background": True}
        code, first = peer.reply("POST", PATH, create)
        self.assertEqual((code, first["status"]), (200, "queued"))
        self.assertNotIn("usage", first)
        path = PATH + "/" + first["id"]
        self.assertEqual(peer.reply("POST", PATH, dict(create, previous_response_id=first["id"]))[0], 400)
        code, loaded = peer.reply("GET", path, None)
        self.assertEqual((code, loaded["usage"]["total_tokens"]), (200, 5))
        self.assertEqual(peer.reply("POST", PATH, create)[0], 400)
        code, second = peer.reply("POST", PATH, dict(create, previous_response_id=first["id"]))
        self.assertEqual(code, 200)
        second_path = PATH + "/" + second["id"]
        self.assertEqual(peer.reply("POST", second_path + "/cancel", None)[0], 200)
        self.assertEqual(peer.reply("POST", second_path + "/cancel", None)[0], 409)
        self.assertEqual(peer.reply("DELETE", second_path, None)[1]["deleted"], True)
        self.assertEqual(peer.reply("GET", second_path, None)[0], 404)
        self.assertEqual(len(peer.snapshot()), 5)
        self.assertEqual(len(peer.all_attempts()), 9)

    def test_reject_wrong_wire_and_public_id(self):
        peer = Stored()
        create = {"input": "fixture-stored", "store": True, "background": True}
        for wrong in (dict(create, store=False), dict(create, model="x"), dict(create, stream=True),
                      dict(create, background=False),
                      dict(create, previous_response_id="resp_gw_" + "a" * 32)):
            self.assertEqual(peer.reply("POST", PATH, wrong)[0], 400)
        self.assertEqual(peer.snapshot(), [])
        self.assertEqual(len(peer.all_attempts()), 5)

    def test_wire_attempt_is_not_double_counted(self):
        peer = Stored()
        peer.record_attempt("GET", PATH + "/unknown")
        self.assertEqual(peer.reply("GET", PATH + "/unknown", None, recorded=True)[0], 404)
        self.assertEqual(peer.all_attempts(), [("GET", PATH + "/unknown")])
