"""Check public SSE incrementally against a gated peer and an independent EXE."""
from contextlib import closing
from http.client import HTTPConnection, IncompleteRead
import json
import re

from sap_stored import PATH
from sap_stored_cases import literal
from sap_stored_recovery_cases import ledger, remaining
from server_process import require, sql


def read_event(reply, hidden):
    # The projected fixture emits exactly one event line and one data line.
    raw = tuple(reply.readline(8193) for _ in range(3))
    require(all(0 < len(line) <= 8192 for line in raw), "Missing or oversized SSE frame")
    require(raw[0].startswith(b"event: ") and raw[1].startswith(b"data: ")
            and raw[2] in (b"\n", b"\r\n"), "Invalid SSE frame envelope")
    try:
        text = b"".join(raw).decode("utf-8")
        name = raw[0][7:].strip().decode("ascii")
        event = json.loads(raw[1][6:])
    except ValueError:
        raise RuntimeError("Stored SSE data is not valid UTF-8/JSON") from None
    require(not any(secret in text for secret in hidden), "Stored SSE exposed private evidence")
    require(isinstance(event, dict) and event.get("type") == name,
            "Stored SSE event type mismatch")
    return event


def binding(pg, identity):
    return json.loads(sql(pg, """
        SELECT json_build_array(r.upstream_id, r.provider_ref, r.credential_ref,
             p.public_model, p.source_key_ref, h.key_ref, h.reserved)::text
        FROM gateway_sap_response_receipts r
        JOIN gateway_sap_response_route_pins p ON p.response_id=r.id
        JOIN gateway_sap_response_balance_holds h ON h.response_id=r.id
        WHERE r.id=""" + literal(identity)))


def audits(pg, owner, identity):
    return json.loads(sql(pg, """
        SELECT COALESCE(json_agg(json_build_array(route_trace->>'storedResponseOperation',
            status, upstream_status, route_attempt_count, provider_account_id,
            route_trace->>'realCredentialRef', stream, total_tokens)
            ORDER BY route_trace->>'storedResponseOperation'), '[]'::json)::text
        FROM gateway_request_audits WHERE access_key_id=""" + literal(owner)
        + " AND route_trace->>'storedResponseId'=" + literal(identity)))


def create(writer, pg, model, token, sources, peer, hidden, index, account, credential):
    with closing(HTTPConnection("127.0.0.1", writer.port, timeout=12)) as conn:
        conn.request("POST", "/v1/responses",
                     json.dumps(dict(model=model, input="fixture-stored", store=True, stream=True)),
                     {"Content-Type": "application/json", "Authorization": f"Bearer {token}"})
        with conn.getresponse() as reply:
            require(reply.status == 200 and reply.headers.get_content_type() == "text/event-stream",
                    f"Stored SSE did not open (HTTP {reply.status})")
            first = read_event(reply, hidden)
            response = first.get("response", {})
            identity = response.get("id", "")
            require(first["type"] == "response.created" and first.get("sequence_number") == 0
                    and response.get("model") == model and response.get("usage") is None
                    and re.fullmatch(r"resp_gw_[a-f0-9]{32}", identity), "Bad created event")
            upstream, provider, exact, alias, source, hold, reserved = binding(pg, identity)
            require((upstream, provider, exact, alias, hold, reserved) ==
                    (f"resp_fixture_{index + 1}", account, credential, model, source, 1)
                    and source in sources, "SSE binding not committed before the first frame")
            require(ledger(pg, identity) == "pending|0|||1", "Invalid SSE pre-terminal hold")
            require(sum(remaining(pg, key) for key in sources) == 15 - index,
                    "SSE create did not reserve exactly one message")
            peer.stored.release[index].set()
            delta = read_event(reply, hidden)
            require(delta["type"] == "response.output_text.delta" and delta.get("sequence_number") == 1
                    and delta.get("delta") == "fixture answer", "Bad SSE delta")
            if index == 0:
                end = read_event(reply, hidden)
                require(end["type"] == "response.completed" and end.get("sequence_number") == 2
                        and end["response"]["id"] == identity and end["response"]["model"] == model
                        and end["response"]["usage"] == dict(input_tokens=2, output_tokens=3, total_tokens=5)
                        and reply.read(1) == b"", "Invalid or duplicate stored SSE terminal event")
            else:
                try:
                    reply.read(1)
                except IncompleteRead as error:
                    require(not error.partial, "Truncated SSE leaked a terminal frame")
                except ConnectionResetError:
                    pass  # An aborted HTTP/1.1 body can surface as a TCP reset on Windows.
                else:
                    raise RuntimeError("Truncated SSE did not abort the HTTP body")
                require(ledger(pg, identity) == "pending|0|||1", "Truncated SSE changed the receipt")
            return identity


def exercise(writer, reader, peer, pg, model, owner, sources, account, credential, secrets):
    hidden = (*secrets, "resp_fixture_", "private-stored", "never-publish")
    token = owner["token"]
    try:
        first, second = (create(writer, pg, model, token, sources, peer, hidden, index,
                                account, credential) for index in range(2))
    finally:
        for release in peer.stored.release:
            release.set()
    require(first != second, "Distinct creates shared a receipt")
    path = "/v1/responses/" + second
    raw, kind = reader.request(path, bearer=token, secrets=hidden)
    response = json.loads(raw)
    require(kind == "application/json" and response.get("id") == second and response.get("model") == model
            and response.get("usage") == dict(input_tokens=2, output_tokens=3, total_tokens=5),
            "A second EXE did not recover the truncated response")
    require(all(ledger(pg, identity) == "completed|1|2|3|0" for identity in (first, second)),
            "Stored SSE settlement did not close both open holds")
    for source in sources:
        charges = sum(binding(pg, identity)[4] == source for identity in (first, second))
        require(remaining(pg, source) == 8 - charges, "GET repeated the scoped charge")
    expected = [("POST", PATH), ("POST", PATH), ("GET", PATH + "/resp_fixture_2")]
    require(peer.stored.snapshot() == peer.stored.all_attempts() == expected,
            "Stored SSE did not make exactly two creates and one recovery GET")
    suffix = [200, 1, account, credential]
    require(audits(pg, owner["id"], first) == [["create", "completed", *suffix, True, 5]]
            and audits(pg, owner["id"], second) ==
            [["create", "failed", *suffix, True, None], ["retrieve", "completed", *suffix, False, None]],
            "Stored SSE audit identity, outcome or usage mismatch")
    return dict(publicAudits=3, inferenceAttempts=3, chargedMessages=2, crossInstanceRecovery=True)
