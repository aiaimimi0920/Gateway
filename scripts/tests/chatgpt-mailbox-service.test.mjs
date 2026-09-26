import assert from "node:assert/strict";
import test from "node:test";
import { fetchMailboxCodePayload, fetchMailboxSnapshotRoot } from "../chatgpt-web-session/mailbox-snapshot.mjs";
import { recoverMailboxByEmail } from "../chatgpt-web-session/mailbox-poll.mjs";

const context = { serviceBaseUrl: "https://mailbox.example.test", apiKey: "synthetic", mailboxSessionId: "session" };
const calls = [
  () => fetchMailboxCodePayload(context),
  () => fetchMailboxSnapshotRoot(context),
  () => recoverMailboxByEmail(context, { emailAddress: "synthetic@example.test" }),
];

test("every mailbox service endpoint rejects credential redirects and installs a deadline signal", async (t) => {
  const options = [];
  t.mock.method(globalThis, "fetch", async (_url, input) => { options.push(input); return Response.json({}); });
  for (const call of calls) await call();
  assert.equal(options.length, 3);
  for (const input of options) {
    assert.equal(input.redirect, "error");
    assert.ok(input.signal instanceof AbortSignal);
  }
});

test("mailbox service body failures do not become valid empty results", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response(new ReadableStream({
    start(controller) { controller.error(new Error("synthetic stream failure")); },
  })));
  for (const call of calls) await assert.rejects(call(), /synthetic stream failure/);
});

test("all mailbox service endpoints enforce the shared body byte limit", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response(new Uint8Array(4 * 1024 * 1024 + 1)));
  for (const call of calls) await assert.rejects(call(), /mailbox_response_too_large/);
});

test("service HTTP status errors retain endpoint classification", async (t) => {
  t.mock.method(globalThis, "fetch", async () => new Response("synthetic rejection", { status: 403 }));
  for (const call of calls) await assert.rejects(call(), /mailbox_(code|snapshot|recover)_status_403/);
});
