import assert from "node:assert/strict";
import test from "node:test";
import { decodeMailboxRefPayload, resolveMailboxContext } from "../chatgpt-web-session/mailbox-context.mjs";
import { extractOpenAiCodeFromMessage, selectOpenAiVerificationCode, extractMailboxCodeMarker } from "../chatgpt-web-session/mailbox-codes.mjs";
import { fetchMailboxSnapshot, resolveMailboxSnapshotSessionIds, resolveIm215ConfigFromSnapshot } from "../chatgpt-web-session/mailbox-snapshot.mjs";
import { fetchIm215DirectMailboxCode, im215Request } from "../chatgpt-web-session/mailbox-im215.mjs";
import { ensureRecoveredMailboxContext, fetchImmediateMailboxCode, waitForMailboxOpenAiCode } from "../chatgpt-web-session/mailbox-poll.mjs";

const context = {
  serviceBaseUrl: "https://mailbox.example.test/", apiKey: "synthetic",
  mailboxSessionId: "session", mailboxProviderKey: "im215",
  mailboxProviderInstanceId: "instance", email: "a@example.test",
};
const message = (code, observedAt, sessionId = "session") => ({
  subject: `Your verification code: ${code}`, observedAt, sessionId,
});

test("mailbox references preserve explicit session precedence and decoded address", () => {
  const mailboxRef = `im215:instance:${encodeURIComponent(JSON.stringify({ address: "A@EXAMPLE.TEST" }))}`;
  const result = resolveMailboxContext({ mailboxRef, mailboxSessionId: "explicit" }, { email: " A@example.test " });
  assert.equal(result.mailboxSessionId, "explicit");
  assert.equal(result.mailboxProviderKey, "im215");
  assert.equal(result.mailboxProviderInstanceId, "instance");
  assert.equal(result.mailboxAddress, "a@example.test");
  assert.equal(result.email, "A@example.test");
  assert.equal(decodeMailboxRefPayload("im215:instance:%broken"), null);
});

test("verification parsing retains contextual priority, HTML and marker contracts", () => {
  assert.equal(extractOpenAiCodeFromMessage({ textBody: "Order 111111. Verification code: 222222" }), "222222");
  assert.equal(extractOpenAiCodeFromMessage({ htmlBody: "<p>Verification code: <b>123456</b></p>" }), "123456");
  assert.equal(selectOpenAiVerificationCode({ extractedCandidates: ["1234567", "654321"] }), "654321");
  assert.equal(extractMailboxCodeMarker({ receivedAt: "1970-01-01T00:00:02Z", messageId: 1000 }), 2000);
});

test("snapshot session selection keeps explicit id and matches provider instance", () => {
  const root = { sessions: [
    { id: "matching", emailAddress: "A@example.test", providerTypeKey: "IM215", providerInstanceId: "instance" },
    { id: "foreign", emailAddress: "a@example.test", providerTypeKey: "im215", providerInstanceId: "other" },
  ] };
  assert.deepEqual([...resolveMailboxSnapshotSessionIds(root, context)], ["session", "matching"]);
});

test("snapshot transport selects newest matching message and preserves marker-only mode", async (t) => {
  const root = { messages: [
    message("111111", "2026-01-01T00:00:01Z"),
    message("999999", "2026-01-01T00:00:03Z", "foreign"),
    message("222222", "2026-01-01T00:00:02Z"),
  ] };
  t.mock.method(globalThis, "fetch", async (url, options) => {
    assert.equal(url, "https://mailbox.example.test/mail/snapshot");
    assert.equal(options.headers.Authorization, "Bearer synthetic");
    return Response.json({ snapshot: root });
  });
  const marker = Date.parse("2026-01-01T00:00:02Z");
  assert.deepEqual(await fetchMailboxSnapshot(context), { code: "222222", marker });
  assert.deepEqual(await fetchMailboxSnapshot(context, { markerOnly: true }), { marker });
});

test("IM215 configuration honors selected instance and credential-set fallback", () => {
  const root = { instances: [{ id: "instance", metadata: {
    apiBase: "https://provider.example.test/v1",
    credentialSetsJson: JSON.stringify([{ items: [{ value: "synthetic" }] }]),
  } }] };
  assert.deepEqual(resolveIm215ConfigFromSnapshot(root, context), {
    baseUrl: "https://provider.example.test/v1", apiKey: "synthetic",
  });
  assert.equal(resolveIm215ConfigFromSnapshot(root, { mailboxProviderInstanceId: "other" }), null);
});

test("IM215 adapter routes key formats and resolves detail responses", async (t) => {
  const requests = [];
  t.mock.method(globalThis, "fetch", async (url, options) => {
    requests.push({ url: new URL(url), headers: options.headers });
    return Response.json(requests.length === 1
      ? { messages: [{ id: "message/id", subject: "Sign in", receivedAt: "2026-01-01T00:00:00Z" }] }
      : { data: { subject: "Verification code: 654321", receivedAt: "2026-01-01T00:00:01Z" } });
  });
  const config = { baseUrl: "https://provider.example.test/v1", apiKey: "AC-synthetic" };
  assert.deepEqual(await fetchIm215DirectMailboxCode(config, "a@example.test"), {
    code: "654321", marker: Date.parse("2026-01-01T00:00:01Z"),
  });
  assert.equal(requests[0].headers["X-API-Key"], "AC-synthetic");
  assert.equal(requests[1].url.pathname, "/v1/messages/message%2Fid");
  assert.equal(requests[1].url.searchParams.get("address"), "a@example.test");
  await im215Request({ ...config, apiKey: "synthetic" }, "GET", "/messages");
  assert.equal(requests[2].headers.Authorization, "Bearer synthetic");
});

test("recovery preserves original context on failure and returns a new recovered value", async (t) => {
  const mock = t.mock.method(globalThis, "fetch", async () => { throw new Error("offline fixture"); });
  assert.equal(await ensureRecoveredMailboxContext(context), context);
  mock.mock.mockImplementation(async () => Response.json({ result: { session: { id: "new-session", mailboxRef: "new-ref" } } }));
  const recovered = await ensureRecoveredMailboxContext(context);
  assert.equal(recovered.mailboxSessionId, "new-session");
  assert.equal(context.mailboxSessionId, "session");
});

test("immediate and polling paths return fresh direct code without a real timer", async (t) => {
  t.mock.method(globalThis, "fetch", async () => Response.json({ code: { code: "123456", messageId: 100 } }));
  assert.equal(await fetchImmediateMailboxCode(context, { minMarker: 100 }), "123456");
  assert.equal(await waitForMailboxOpenAiCode(context, { timeoutMs: 100, minMarker: 99 }), "123456");
});
