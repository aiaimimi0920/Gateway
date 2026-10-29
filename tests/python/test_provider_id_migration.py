"""No credentials, external requests, or filesystem writes in migration tests."""
import importlib.util
import json
import sqlite3
import unittest
from pathlib import Path

spec = importlib.util.spec_from_file_location('provider_id_migration',
    Path(__file__).resolve().parents[2] / 'tools/provider_id_migration.py')
migration = importlib.util.module_from_spec(spec)
spec.loader.exec_module(migration)


class ProviderIdMigrationTests(unittest.TestCase):
    def document(self):
        return {'providers': [{'id': 'chatgpt', 'label': 'chatgpt',
            'preset': 'chatgpt-codex-oauth-official-api',
            'credentials': [{'id': 'chatgpt-credential', 'api_key': 'chatgpt'}]}],
            'model_routes': [{'pattern': 'chatgpt', 'provider_ids': ['chatgpt']}],
            'account_groups': [{'provider_credential_ids': ['chatgpt-credential']}]}

    def db(self):
        db = sqlite3.connect(':memory:')
        for table in ('refill_tasks', 'refill_deliveries', 'credential_test_results',
                      'credential_test_slots', 'gateway_cash_requests', 'local_balance_inflight'):
            db.execute(f'CREATE TABLE {table}(id TEXT PRIMARY KEY)')
        for table in ('request_audits', 'credential_model_states', 'local_access_keys'):
            db.execute(f'CREATE TABLE {table}(id TEXT PRIMARY KEY,payload TEXT)')
        db.execute('CREATE TABLE provider_pricing(provider_id TEXT PRIMARY KEY,payload TEXT)')
        audit = {'providerAccountId': 'chatgpt', 'prompt': 'chatgpt',
                 'routeTrace': {'attemptedProviderIds': ['chatgpt'],
                                'selectedCandidate': {'providerAccountId': 'chatgpt',
                                                      'protocolProfile': 'chatgpt'}}}
        db.execute('INSERT INTO request_audits VALUES (?,?)', ('request-1', json.dumps(audit)))
        db.commit()
        return db

    def test_document_preserves_credentials_protocol_and_groups(self):
        before = self.document()
        after, mapping = migration.migrate_document(before)
        self.assertEqual(mapping, {'chatgpt': 'codex-oauth'})
        self.assertEqual(after['providers'][0]['credentials'], before['providers'][0]['credentials'])
        self.assertEqual(after['account_groups'], before['account_groups'])
        self.assertEqual(after['model_routes'][0]['provider_ids'], ['codex-oauth'])
        self.assertEqual(after['model_routes'][0]['pattern'], 'chatgpt')
        self.assertEqual(before['providers'][0]['id'], 'chatgpt')
        self.assertEqual(migration.migrate_document(after), (after, {}))

    def test_collision_and_implicit_credential_fail_closed(self):
        doc = self.document()
        doc['providers'].append({'id': 'codex-oauth'})
        with self.assertRaises(ValueError): migration.migrate_document(doc)
        doc = self.document()
        del doc['providers'][0]['credentials'][0]['id']
        with self.assertRaises(ValueError): migration.migrate_document(doc)

    def test_runtime_precise_fields_repeat_and_reverse(self):
        with self.db() as db:
            before = db.execute('SELECT payload FROM request_audits').fetchone()[0]
            stats = migration.migrate_runtime(db, {'chatgpt': 'codex-oauth'})
            self.assertEqual(stats['request_audits'], 1)
            after = json.loads(db.execute('SELECT payload FROM request_audits').fetchone()[0])
            self.assertEqual(after['prompt'], 'chatgpt')
            self.assertEqual(after['routeTrace']['selectedCandidate']['protocolProfile'], 'chatgpt')
            self.assertEqual(migration.migrate_runtime(db, {'chatgpt': 'codex-oauth'})['request_audits'], 0)
            migration.migrate_runtime(db, {'codex-oauth': 'chatgpt'})
            self.assertEqual(json.loads(before), json.loads(db.execute('SELECT payload FROM request_audits').fetchone()[0]))

    def test_model_identity_collision_rolls_back_audits(self):
        with self.db() as db:
            state = {'providerAccountId': 'chatgpt', 'providerCredentialId': '',
                     'providerCredentialRef': 'ignored', 'protocolProfile': None, 'model': 'model'}
            state['id'] = migration.model_state_id(state)
            target = dict(state, providerAccountId='codex-oauth')
            target['id'] = migration.model_state_id(target)
            for value in (state, target):
                db.execute('INSERT INTO credential_model_states VALUES (?,?)', (value['id'], json.dumps(value)))
            db.commit()
            with self.assertRaises(ValueError): migration.migrate_runtime(db, {'chatgpt': 'codex-oauth'})
            audit = json.loads(db.execute('SELECT payload FROM request_audits').fetchone()[0])
            self.assertEqual(audit['providerAccountId'], 'chatgpt')

    def test_unreviewed_queue_state_rejected(self):
        with self.db() as db:
            db.execute("INSERT INTO credential_test_slots VALUES ('opaque-hash')")
            db.commit()
            with self.assertRaises(ValueError): migration.migrate_runtime(db, {'chatgpt': 'codex-oauth'})


if __name__ == '__main__':
    unittest.main()
