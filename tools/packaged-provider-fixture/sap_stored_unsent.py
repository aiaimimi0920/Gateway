"""Prove that the packaged submit owner records a real pre-send rejection."""
import re
from uuid import uuid4

from sap_stored_cases import literal, public
from server_process import require, sql


def produce(writer, peer, pg, alias, owner, source, credential, *, deny_proof=False):
    """Run after successful creates, while the scoped hold-close fault is active."""
    name = "sap_unsent_" + uuid4().hex
    member = literal(source)
    exact = literal(credential)
    proof_fault = f"""
        CREATE FUNCTION public.{name}_proof() RETURNS trigger LANGUAGE plpgsql AS $fault$
        BEGIN
            IF EXISTS(SELECT 1 FROM gateway_sap_response_route_pins p
                JOIN gateway_sap_response_receipts r ON r.id=p.response_id
                WHERE p.response_id=NEW.response_id AND p.source_key_ref={member}
                    AND r.credential_ref={exact}) THEN
                RAISE EXCEPTION 'synthetic proof write failure';
            END IF;
            RETURN NEW;
        END; $fault$;
        CREATE TRIGGER {name}_proof BEFORE INSERT ON public.gateway_sap_response_unsent_proofs
            FOR EACH ROW EXECUTE FUNCTION public.{name}_proof();
    """ if deny_proof else ""
    # The live submit path checks authorization before and after committing the
    # reserve. Revoke in that transaction, not at OAuth (which is too early).
    # No test hook in the executable and no test-generated proof row.
    sql(pg, f"""BEGIN;
        CREATE FUNCTION public.{name}() RETURNS trigger LANGUAGE plpgsql AS $fault$
        BEGIN
            IF EXISTS(SELECT 1 FROM gateway_sap_response_receipts
                WHERE id=NEW.response_id AND credential_ref={exact} AND phase='submitting') THEN
                UPDATE gateway_provider_credentials SET status='disabled'
                    WHERE id={exact} AND status='active';
                IF NOT FOUND THEN RAISE EXCEPTION 'unexpected fixture credential state'; END IF;
            END IF;
            RETURN NEW;
        END; $fault$;
        CREATE TRIGGER {name} AFTER INSERT ON public.gateway_sap_response_balance_holds
            FOR EACH ROW WHEN (NEW.key_ref={member})
            EXECUTE FUNCTION public.{name}(); {proof_fault} COMMIT;""")
    attempts = peer.stored.all_attempts()
    inference = peer.inference_attempts()
    try:
        refusal = public(writer, owner["token"], "/v1/responses",
            dict(model=alias, input="fixture-stored", store=True, background=True), expected=502)
    finally:
        remove_proof_fault = (
            f"DROP TRIGGER {name}_proof ON public.gateway_sap_response_unsent_proofs; "
            f"DROP FUNCTION public.{name}_proof();" if deny_proof else ""
        )
        sql(pg, f"BEGIN; DROP TRIGGER {name} ON public.gateway_sap_response_balance_holds; "
                f"DROP FUNCTION public.{name}(); {remove_proof_fault} "
                f"UPDATE gateway_provider_credentials SET status='active' WHERE id={exact} AND status='disabled'; "
                "COMMIT;")
    error = refusal.get("error", {})
    ids = set(re.findall(r"\bresp_gw_[a-f0-9]{32}\b", error.get("message", "")))
    require(error.get("code") == "sap_aicore_response_refund_pending" and len(ids) == 1,
            "Expected an unresolved refund with a local receipt ID")
    identity = ids.pop()
    require(peer.stored.all_attempts() == attempts and peer.inference_attempts() == inference,
            "Pre-send rejection reached inference")
    require(sql(pg, "SELECT count(*) FROM gateway_sap_response_receipts r "
                "JOIN gateway_sap_response_route_pins pin ON pin.response_id=r.id "
                f"WHERE r.id={literal(identity)} AND r.phase='submitting' AND r.upstream_id IS NULL "
                f"AND r.credential_ref={exact} AND pin.source_key_ref={member}") == "1",
            "Live submit owner did not retain the exact receipt")
    proofs = sql(pg, "SELECT p.revision=r.revision FROM gateway_sap_response_unsent_proofs p "
                 f"JOIN gateway_sap_response_receipts r ON r.id=p.response_id WHERE r.id={literal(identity)}")
    require(proofs == ("" if deny_proof else "t"), "Wrong durable proof state for the injected fault")
    return identity
