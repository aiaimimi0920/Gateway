"""Public stored lifecycle checks; all SQL is scoped to generated fixture IDs."""
from concurrent.futures import ThreadPoolExecutor
import json
import re
import time

from run_sensenova_hosted import PROJECT, TENANT
from server_process import require, sql


def literal(value):
    require(isinstance(value, str) and re.fullmatch(r"[a-zA-Z0-9_-]+", value),
            "Unsafe fixture identifier")
    return "'" + value + "'"


def aggregate(writer, pg, alias, original, hidden):
    bundle = sql(pg, f"SELECT bundle_id FROM gateway_access_key_bundle_bindings WHERE access_key_id={literal(original)}")
    literal(bundle)
    def key(kind, bundles):
        row = writer.management("access/keys", dict(ownerType="user", ownerId=alias,
            resolvedProjectId=PROJECT, resolvedTenantId=TENANT, keyKind=kind,
            publicKeyPrefix="neuro", displayName=alias, bundleIds=bundles))
        hidden.append(row["token"])
        return row
    alternate = key("normal", [bundle])["id"]
    owner = key("auto_route", [])
    writer.management(f"access/keys/{owner['id']}/aggregate-memberships/replace",
        {"memberships": [{"memberAccessKeyId": original, "priority": 1},
                        {"memberAccessKeyId": alternate, "priority": 2}]})
    for source in (original, alternate):
        writer.management(f"access/keys/{source}/balances/adjust",
            dict(balanceMode="message_prepaid", status="active", totalMessages=8, remainingMessages=8))
    return owner, (original, alternate)


def public(backend, token, path, body=None, *, method=None, expected=200):
    text, kind = backend.request(path, body, bearer=token, method=method, expected=expected)
    require(kind == "application/json" and "resp_fixture_" not in text
            and "private-stored" not in text and "never-publish" not in text,
            "Stored response leaked private upstream identity or metadata")
    return json.loads(text)


def create(backend, token, alias, previous=None):
    body = dict(model=alias, input="fixture-stored", store=True, background=True)
    if previous:
        body["previous_response_id"] = previous
    row = public(backend, token, "/v1/responses", body)
    require(row.get("status") == "queued" and row.get("model") == alias
            and re.fullmatch(r"resp_gw_[a-f0-9]{32}", row.get("id", "")) and row.get("usage") is None,
            "Creation must publish a local ID without terminal usage")
    return row["id"]


def exercise(writer, reader, restart, peer, pg, alias, owner, sources, account, credential):
    token = owner["token"]
    first = create(writer, token, alias)
    writer.drain()
    writer = restart()
    path = "/v1/responses/" + first
    loaded = public(writer, token, path)
    require(loaded["id"] == first and loaded["usage"]["total_tokens"] == 5, "Restarted receipt failed")
    second = create(reader, token, alias, first)
    second_path = "/v1/responses/" + second
    with ThreadPoolExecutor(max_workers=2) as pool:
        jobs = [pool.submit(public, backend, token, second_path + "/cancel", method="POST", expected=(200, 409))
                for backend in (writer, reader)]
        results = [job.result() for job in jobs]
    require(sum(row.get("status") == "cancelled" for row in results) == 1, "Concurrent cancel was not single-send")
    require(public(reader, token, second_path)["usage"]["total_tokens"] == 5, "Cancelled usage missing")
    require(public(writer, token, second_path, method="DELETE") == {
        "id": second, "object": "response.deleted", "deleted": True}, "Delete not confirmed")
    before = peer.stored.all_attempts()
    public(reader, token, second_path, expected=404)
    require(peer.stored.all_attempts() == before, "Tombstone lookup was forwarded")
    third = create(writer, token, alias)
    third_path = "/v1/responses/" + third
    public(reader, token, third_path)  # Warm the reader, then expire just the bound member.
    bound = sql(pg, f"SELECT source_key_ref FROM gateway_sap_response_route_pins WHERE response_id={literal(third)}")
    require(bound in sources, "Receipt bound an unexpected balance")
    sql(pg, f"UPDATE gateway_access_keys SET expires_at=now()-interval '1 second' WHERE id={literal(bound)}")
    before = peer.stored.all_attempts()
    public(reader, token, third_path, expected=404)
    require(peer.stored.all_attempts() == before, "Revoked receipt used the still-authorized alternate")
    # The out-of-band expiry above deliberately leaves the reader cache stale.
    # Now refresh membership through its real API before a new-route positive control.
    survivor = next(source for source in sources if source != bound)
    writer.management(f"access/keys/{owner['id']}/aggregate-memberships/replace",
        {"memberships": [{"memberAccessKeyId": survivor, "priority": 1}]})
    fourth = create(reader, token, alias)
    alternate = sql(pg, f"SELECT source_key_ref FROM gateway_sap_response_route_pins WHERE response_id={literal(fourth)}")
    require(alternate == survivor, "The alternate did not remain routable")
    loaded = public(reader, token, "/v1/responses/" + fourth)
    require(loaded["usage"] == dict(input_tokens=2, output_tokens=3, total_tokens=5),
            "Alternate route did not report authoritative usage")
    ids = (first, second, third, fourth)
    selected = ",".join(literal(value) for value in ids)
    require(sql(pg, f"SELECT count(*) FROM gateway_sap_response_receipts WHERE id IN ({selected}) AND settled=1 AND input_tokens=2 AND output_tokens=3") == "4",
            "Terminal receipts did not settle once")
    for source in sources:
        charged = int(sql(pg, f"SELECT count(*) FROM gateway_sap_response_route_pins WHERE response_id IN ({selected}) AND source_key_ref={literal(source)}"))
        remaining = int(sql(pg, f"SELECT remaining_messages FROM gateway_access_key_balances WHERE access_key_id={literal(source)}"))
        require(remaining == 8 - charged, "Mutation/read or restart duplicated a message charge")
    calls = peer.stored.snapshot()
    from sap_stored import PATH
    expected = [("POST", PATH), ("GET", PATH + "/resp_fixture_1"), ("POST", PATH),
                ("POST", PATH + "/resp_fixture_2/cancel"), ("GET", PATH + "/resp_fixture_2"),
                ("DELETE", PATH + "/resp_fixture_2"), ("POST", PATH), ("GET", PATH + "/resp_fixture_3"),
                ("POST", PATH), ("GET", PATH + "/resp_fixture_4")]
    require(calls == expected and peer.stored.all_attempts() == expected,
            "Unexpected stored lifecycle sends")
    where = f"FROM gateway_request_audits WHERE access_key_id={literal(owner['id'])}"
    deadline = time.monotonic() + 10
    while time.monotonic() < deadline:
        if sql(pg, f"SELECT count(*) {where} AND status NOT IN ('started','running')") == "13":
            break
        time.sleep(0.25)
    require(sql(pg, f"SELECT count(*) {where}") == "13", "Missing or duplicate public audits")
    require(sql(pg, f"SELECT count(*) {where} AND status IN ('started','running')") == "0", "Running audit leaked")
    require(sql(pg, f"SELECT count(*) {where} AND route_attempt_count=0 AND provider_account_id IS NULL") == "3",
            "Local refusals falsely attributed an upstream")
    exact = (f"route_attempt_count=1 AND provider_account_id={literal(account)} "
             f"AND route_trace->>'realCredentialRef'={literal(credential)}")
    require(sql(pg, f"SELECT count(*) {where} AND {exact}") == "10", "Actual send audit identity is wrong")
    cursor = ""
    for _ in range(100):
        inventory = reader.management("console/sap-aicore-responses/pending" + ("?after=" + cursor if cursor else ""))
        rows = inventory["items"]
        require(not any(row["id"] in ids for row in rows), "Settled work remained pending")
        if len(rows) < 100:
            break
        next_cursor = rows[-1]["id"]
        require(next_cursor > cursor, "Pending inventory cursor did not advance")
        literal(next_cursor)
        cursor = next_cursor
    else:
        raise RuntimeError("Pending inventory exceeded the fixture scan budget")
    return {"receiptIds": ids, "sendCount": len(calls), "publicAudits": 13,
            "sourceMembers": sources, "singleCancel": True, "revokedOriginalRejected": True,
            "alternateStillRoutable": True}
