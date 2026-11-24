"""Restarted EXEs recover local accounting without provider access or replay."""
import re
import time
import uuid

from sap_stored import PATH
from sap_stored_cases import literal, public
from sap_stored_unsent import produce
from server_process import require, sql


def remaining(pg, source):
    return int(sql(pg, "SELECT remaining_messages FROM gateway_access_key_balances "
                   f"WHERE access_key_id={literal(source)}"))


def snapshot(peer):
    return peer.stored.all_attempts(), peer.snapshot(), peer.oauth_attempts()


def ledger(pg, identity):
    return sql(pg, "SELECT r.phase,r.settled,r.input_tokens,r.output_tokens,h.open "
               "FROM gateway_sap_response_receipts r JOIN gateway_sap_response_balance_holds h "
               f"ON h.response_id=r.id WHERE r.id={literal(identity)}")


def exercise(instances, start, peer, pg, alias, owner, sources, credential):
    active = [backend for backend in instances if backend.process.poll() is None]
    require(len(active) == 2, "Recovery requires two active original EXEs")
    source = sql(pg, "SELECT id FROM gateway_access_keys WHERE id IN ("
                 + ",".join(literal(value) for value in sources)
                 + ") AND (expires_at IS NULL OR expires_at>now())")
    require(source in sources, "Expected exactly one remaining fixture member")
    # The dedicated database owner installs only a randomly named, member-scoped fault.
    require(sql(pg, "SELECT current_schema()") == "public", "Unexpected fixture schema")
    require(sql(pg, "SELECT tableowner=current_user FROM pg_tables WHERE schemaname='public' "
                "AND tablename='gateway_sap_response_balance_holds'") == "t", "Fixture owner required")
    name = "sap_recovery_" + uuid.uuid4().hex
    literal(name)
    before = remaining(pg, source)
    calls = peer.stored.all_attempts()
    pending = []
    installed = False
    try:
        sql(pg, f"""BEGIN;
            CREATE FUNCTION public.{name}() RETURNS trigger LANGUAGE plpgsql AS $fault$
            BEGIN RAISE EXCEPTION 'synthetic SAP ledger close failure'; END; $fault$;
            CREATE TRIGGER {name} BEFORE UPDATE OF open ON public.gateway_sap_response_balance_holds
            FOR EACH ROW WHEN (OLD.key_ref={literal(source)} AND OLD.open=1 AND NEW.open=0)
            EXECUTE FUNCTION public.{name}(); COMMIT;""")
        installed = True
        for number, (with_usage, tokens) in enumerate(((True, "2|3"), (False, "|")), 1):
            peer.stored.complete_next_create(usage=with_usage)
            failure = public(active[0], owner["token"], "/v1/responses",
                dict(model=alias, input="fixture-stored", store=True, background=True), expected=502)
            error = failure.get("error", {})
            require(error.get("code") == "sap_aicore_response_settlement_pending", "Wrong recovery error")
            ids = set(re.findall(r"\bresp_gw_[a-f0-9]{32}\b", error.get("message", "")))
            require(len(ids) == 1, "Missing local recovery identity")
            identity = ids.pop()
            require(all(identity != prior for prior, _ in pending), "Terminal receipt was reused")
            pending.append((identity, tokens))
            require(ledger(pg, identity) == f"completed|0|{tokens}|1", "Terminal evidence was not retained")
            require(remaining(pg, source) == before - number, "Create did not debit exactly once")
            require(peer.stored.all_attempts() == calls + [("POST", PATH)] * number, "Create replayed")
        proven = produce(active[0], peer, pg, alias, owner, source, credential)
        unknown = produce(active[0], peer, pg, alias, owner, source, credential, deny_proof=True)
        require(proven != unknown and remaining(pg, source) == before - 4,
                "Live rejected submits did not debit distinct holds")
        for backend in active:
            backend.drain()
        require(all(ledger(pg, receipt) == f"completed|0|{tokens}|1" for receipt, tokens in pending),
                "Fault was bypassed before restart")
        require(all(ledger(pg, receipt) == "submitting|0|||1" for receipt in (proven, unknown)),
                "Rejected create did not retain a pending hold before restart")
    finally:
        # No fixture data is deleted. Only these two newly created fault objects retire.
        if installed:
            sql(pg, f"BEGIN; DROP TRIGGER {name} ON public.gateway_sap_response_balance_holds; "
                    f"DROP FUNCTION public.{name}(); COMMIT;")

    network = snapshot(peer)
    where = f"FROM gateway_request_audits WHERE access_key_id={literal(owner['id'])}"
    audit_count = sql(pg, "SELECT count(*) " + where)
    require(audit_count == "17", "Recovery source request audit is missing or duplicated")
    for receipt, _ in pending:
        require(sql(pg, "SELECT count(*) " + where + " AND status NOT IN ('started','running')"
                    f" AND route_trace->>'storedResponseId'={literal(receipt)} AND route_attempt_count=1") == "1",
                "Recovery source audit lacks terminal exact-send evidence")
    for receipt in (proven, unknown):
        require(sql(pg, "SELECT count(*) " + where + " AND status NOT IN ('started','running')"
                    f" AND route_trace->>'storedResponseId'={literal(receipt)} AND route_attempt_count=0"
                    " AND provider_account_id IS NULL AND upstream_status IS NULL") == "1",
                "Rejected stored create lacks terminal zero-send evidence")
    restarted = [start("recovery-a"), start("recovery-b")]

    def settled():
        return all(ledger(pg, receipt) == f"completed|1|{tokens}|0" for receipt, tokens in pending)

    deadline = time.monotonic() + 20
    while time.monotonic() < deadline:
        if settled() and ledger(pg, proven) == "submitting|0|||0":
            break
        time.sleep(0.25)
    require(settled(), "Restart did not settle both terminal holds without inventing usage")
    require(ledger(pg, proven) == "submitting|0|||0", "Durable proof did not refund")
    require(ledger(pg, unknown) == "submitting|0|||1", "Unknown submission was inferred")
    require(remaining(pg, source) == before - 3, "Recovery duplicated or invented a refund")
    # Observe another scheduled pass, not just the first startup race.
    time.sleep(11)
    require(all(backend.process.poll() is None for backend in restarted), "Recovery EXE exited early")
    require(settled() and remaining(pg, source) == before - 3, "Later worker pass changed settled accounting")
    require(ledger(pg, unknown) == "submitting|0|||1", "Later pass refunded unknown work")
    require(snapshot(peer) == network, "Local recovery accessed OAuth or the provider")
    require(sql(pg, "SELECT count(*) " + where) == audit_count, "Recovery invented a public request")
    for backend in restarted:
        backend.drain()
    require(snapshot(peer) == network, "Recovery drain accessed OAuth or the provider")
    return dict(terminalReceipt=pending[0][0], noUsageTerminalReceipt=pending[1][0],
        noUsageTokensStayedNull=True, liveUnsentProofReceipt=proven, liveProofWriteFailureReceipt=unknown,
        localRefunds=1, rejectedCreates=2, rejectedCreateSends=0,
        pids=[backend.process.pid for backend in restarted], additionalPublicSends=2,
        automaticRecoverySends=0, automaticRecoveryOAuthPosts=0, publicAudits=int(audit_count),
        originalMember=source, remainingMessages=before - 3, secondPeriodStable=True)
