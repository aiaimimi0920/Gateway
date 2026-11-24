"""Hosted parity assertions for SAP's pre-inference OAuth rejection."""
import re
import time

from run_sensenova_hosted import check_balance
from server_process import require, sql


def safe_ids(*values):
    require(all(re.fullmatch(r"[A-Za-z0-9_-]+", value) for value in values),
            "Invalid SAP audit fixture identifiers")


def health(pg, account, credential, model):
    safe_ids(account, credential, model)
    # Include timestamps and error fields: neutral means no write, not merely 'still active'.
    return sql(pg, "SELECT jsonb_build_object('credential', "
               "(SELECT jsonb_build_array(status, failure_count, cooldown_until, last_error, last_health_check_at, updated_at) "
               f"FROM gateway_provider_credentials WHERE id = '{credential}' AND provider_account_id = '{account}'), "
               "'model', (SELECT jsonb_agg(to_jsonb(s) ORDER BY s.id) "
               "FROM gateway_provider_credential_model_states s "
               f"WHERE provider_account_id = '{account}' AND "
               f"(provider_credential_ref = '{credential}' OR provider_credential_id = '{credential}') "
               f"AND model = '{model}'))")


def check_oauth_rejection(pg, account, credential, key, alias, model, completed, before):
    safe_ids(account, credential, key, alias, model)
    where = f"FROM gateway_request_audits WHERE requested_model = '{alias}' OR access_key_id = '{key}'"
    rejected = ("status = 'failed' AND route_attempt_count = 0 AND upstream_status IS NULL "
                "AND prompt_tokens IS NULL AND completion_tokens IS NULL AND total_tokens IS NULL "
                "AND COALESCE(provider_account_id, '') = '' AND route_trace->>'realCredentialRef' IS NULL "
                "AND route_trace->'attemptedProviderIds' = '[]'::jsonb "
                "AND route_trace->>'errorCode' = 'sap_aicore_token_failed' "
                f"AND access_key_id = '{key}' AND requested_model = '{alias}'")
    query = f"SELECT count(*), count(*) FILTER (WHERE status = 'completed'), count(*) FILTER (WHERE {rejected}) {where}"
    end = time.monotonic() + 15
    actual = ""
    while time.monotonic() < end:
        actual = sql(pg, query)
        if actual == f"{completed + 1}|{completed}|1":
            break
        require(int(actual.split("|")[0]) <= completed + 1, "Duplicate SAP OAuth rejection audit")
        time.sleep(0.3)
    else:
        raise RuntimeError(f"SAP OAuth rejection audit did not prove zero inference: {actual}")
    check_balance(pg, key, completed)
    require(health(pg, account, credential, model) == before, "Wrong CA changed SAP credential or model health")
