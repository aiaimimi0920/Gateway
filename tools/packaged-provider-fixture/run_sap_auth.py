"""Exercise OAuth budget, single-flight, expiry and debit boundaries in a packaged backend."""
import argparse
from concurrent.futures import ThreadPoolExecutor
from contextlib import ExitStack
import json
from pathlib import Path
import re
import secrets
from threading import Barrier, Event, Lock
import time
import uuid

from hosted_product import SENSENOVA_HOSTED
from run_sap_hosted import Product
from run_sensenova_hosted import LINSHI, assert_public, setup
from sap_peer import AUTH, AZURE, DEPLOY, MODEL, SAP
from server_process import Backend, PG, REDIS, fixture_url, package_identity, require, sql


class ControlledOAuth(SAP):
    def __init__(self, root):
        super().__init__(root)
        self.replies = 0
        self.reply_lock = Lock()
        self.recovery_waiting = Event()
        self.release_timeout = Event()
        self.release_recovery = Event()

    def token_response(self, host):
        with self.reply_lock:
            self.replies += 1
            exchange = self.replies
        if exchange == 1:
            self.release_timeout.wait(25)
        elif exchange == 2:
            self.recovery_waiting.set()
            self.release_recovery.wait(5)
        response = json.loads(super().token_response(host))
        response['expires_in'] = 5 if exchange == 2 else 900
        return json.dumps(response).encode()


def exercise(backend, alias, bearer, peer, hidden):
    body = SENSENOVA_HOSTED.request_body('chat/completions', alias, False)
    oauth = dict(host=AUTH, path='/oauth/token', stream=None)
    chat = dict(host=AZURE, path=f'{DEPLOY}/chat/completions', stream=False)

    def call(expected=200):
        return backend.request('/v1/chat/completions', body, expected=expected, bearer=bearer,
                               secrets=hidden, timeout=24)

    started = time.monotonic()
    try:
        error, _ = call(expected=502)
        elapsed = time.monotonic() - started
        require(json.loads(error)['error']['code'] == 'sap_aicore_token_timeout', 'Expected the SAP OAuth budget error')
        require(14 <= elapsed <= 23 and peer.snapshot()[0] == [oauth],
                'OAuth timeout must not submit inference or replay OAuth')
    finally:
        peer.release_timeout.set()

    def success():
        text, kind = call()
        assert_public('chat/completions', False, alias, text, kind, MODEL)

    rendezvous = Barrier(2, timeout=8)

    def concurrent_success():
        rendezvous.wait()
        success()

    with ThreadPoolExecutor(max_workers=2) as pool:
        active = [pool.submit(concurrent_success) for _ in range(2)]
        try:
            require(peer.recovery_waiting.wait(8), 'No OAuth after failure')
            # Keep the first OAuth in-flight long enough for the second dispatched request to join it.
            require(not peer.release_recovery.wait(0.5), 'OAuth was released prematurely')
            require(peer.snapshot()[0] == [oauth, oauth] and all(not f.done() for f in active),
                    'Recovery did not keep both calls behind one OAuth exchange')
        finally:
            peer.release_recovery.set()
        for result in active:
            result.result(timeout=25)
    require(peer.snapshot()[0] == [oauth, oauth, chat, chat], 'Concurrent generation did not reuse a single OAuth exchange')
    time.sleep(5.1)
    success()
    calls = peer.snapshot()[0]
    require(calls == [oauth, oauth, chat, chat, oauth, chat], 'Expired token was reused or a request replayed')
    return elapsed, calls


def check_settlement(pg, account, credential, key_id, alias):
    require(all(re.fullmatch(r'[A-Za-z0-9_-]+', value) for value in (account, credential, key_id, alias)),
            'Unexpected fixture audit identifiers')
    completed = ("status = 'completed' AND prompt_tokens = 2 AND completion_tokens = 3 AND total_tokens = 5 "
                 "AND route_attempt_count = 1 AND resolved_model = 'fixture-chat' "
                 f"AND provider_account_id = '{account}' AND route_trace->>'realCredentialRef' = '{credential}'")
    query = (f"SELECT count(*) FILTER (WHERE status = 'completed'), count(*) FILTER (WHERE {completed}), "
             "count(*) FILTER (WHERE status <> 'completed') FROM gateway_request_audits "
             f"WHERE access_key_id = '{key_id}' AND requested_model = '{alias}'")
    balance = ("SELECT total_messages - remaining_messages FROM gateway_access_key_balances "
               f"WHERE access_key_id = '{key_id}' AND balance_mode = 'message_prepaid'")
    end = time.monotonic() + 12
    while time.monotonic() < end:
        summary = sql(pg, query)
        debit = sql(pg, balance)
        if summary == '3|3|1' and debit == '3':
            return
        time.sleep(0.35)
    raise RuntimeError(f'Unexpected OAuth/settlement audit counts {summary} or net debit {debit}')


def run(package, evidence):
    evidence = evidence.resolve()
    require(evidence.is_relative_to(LINSHI.resolve()) and evidence != LINSHI.resolve(), 'Use a new linshi subdirectory')
    package = package.resolve(strict=True)
    identity = package_identity(package)
    pg = fixture_url('SAP_AICORE_TEST_DATABASE_URL', PG, 5432, 'postgresql', '/gateway_fixture')
    redis = fixture_url('SAP_AICORE_TEST_REDIS_URL', REDIS, 6379, 'redis', '/')
    evidence.mkdir()
    scope = 'packaged-sap-aicore-auth-' + uuid.uuid4().hex
    peer = ControlledOAuth(evidence)
    token, signing = secrets.token_hex(32), secrets.token_hex(32)
    key = peer.service_key(False)
    hidden = [token, signing, key, peer.client_id, peer.secret, peer.basic, *peer.tokens.values()]
    stage = 'startup-isolation'
    try:
        with peer.serve(), ExitStack() as stack:
            def start(name, ca=None):
                backend = Backend(package, evidence / name, peer, scope, token, signing, pg, redis,
                                  ca=ca, provider='SAP_AICORE')
                stack.callback(backend.close, graceful=ca is None)
                return backend

            guard = start('missing-ca', evidence / 'unavailable.pem')
            require(guard.process.wait(timeout=15) != 0, 'Backend did not reject missing fixture CA')
            logs = b''.join(p.read_bytes() for p in (evidence / 'missing-ca').glob('*.log'))
            require(b'SAP AI Core fixture CA must be an absolute file path' in logs, 'No verified fixture trust guard')
            backend = start('runtime')
            backend.ready()
            stage = 'setup'
            account, credential, key_id, bearer = setup(backend, scope, key, hidden, Product(AZURE))
            stage = 'oauth-timeout-recovery-expiry'
            elapsed, calls = exercise(backend, scope, bearer, peer, hidden)
            stage = 'settlement'
            check_settlement(pg, account, credential, key_id, scope)
            backend.drain()
            stage = 'redaction'
            for log in evidence.rglob('*.log'):
                data = log.read_bytes()
                require(not any(value.encode() in data for value in hidden), 'Backend log exposed synthetic credentials')
    except Exception:
        (evidence / 'failure.json').write_text(json.dumps(dict(identity, status='failed', stage=stage,
            peerCalls=peer.snapshot()[0]), indent=2) + '\n', encoding='utf-8')
        raise
    (evidence / 'result.json').write_text(json.dumps(dict(identity, status='passed', timeoutSeconds=round(elapsed, 3),
        peerCalls=calls, oauthTimeoutNoInference=True, concurrentRecoverySingleFlight=True, expiryExchanged=True,
        completedAudits=3, failedAudits=1, netDebitedMessages=3, gracefulDrain=True), indent=2) + '\n', encoding='utf-8')
    print(f'Passed: {evidence / "result.json"}')


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--package', required=True, type=Path)
    parser.add_argument('--evidence', required=True, type=Path)
    options = parser.parse_args()
    run(options.package, options.evidence)
