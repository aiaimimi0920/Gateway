import test from "node:test";
import assert from "node:assert/strict";
import { sessionReceipt } from "../lumalabs-session/session-receipt.mjs";

const input = { baseUrl: "https://app.lumalabs.ai", sessionBinding: "synthetic-attempt" };
const cookie = { name: "wos-session", value: "synthetic-successor", domain: "app.lumalabs.ai",
  path: "/", httpOnly: true, secure: true, sameSite: "Lax", expires: 2000000000 };
const context = (cookies) => ({ cookies: async () => cookies });

test("session receipt preserves a successor on business refusal without exporting unrelated cookies", async () => {
  const receipt = await sessionReceipt(context([cookie, { name: "unrelated", value: "private" }]), input,
    { ok: false, error: { status: 402 } });
  assert.equal(receipt.cookie.value, cookie.value);
  assert.equal(receipt.reauthenticationRequired, false);
  assert.ok(!JSON.stringify(receipt).includes("private"));
});

test("session receipt rejects duplicate or foreign scoped authentication cookies", async () => {
  for (const patch of [{ domain: "evil.invalid" }, { path: "/other" }, { secure: false },
    { httpOnly: false }, { sameSite: "None" }, { value: "bad;injection" }, { value: "x".repeat(32769) }]) {
    await assert.rejects(sessionReceipt(context([{ ...cookie, ...patch }]), input, { ok: true }));
  }
  await assert.rejects(sessionReceipt(context([cookie, cookie]), input, { ok: true }));
});

test("expired login or removed session stays a reauthentication condition", async () => {
  for (const [cookies, status] of [[[], 200], [[cookie], 401]]) {
    const receipt = await sessionReceipt(context(cookies), input, { ok: false, error: { status } });
    assert.equal(receipt.reauthenticationRequired, true);
  }
  assert.equal(await sessionReceipt(context([cookie]), { baseUrl: input.baseUrl }, { ok: true }), undefined);
});
