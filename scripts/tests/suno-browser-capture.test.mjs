import test from "node:test";
import assert from "node:assert/strict";
import { createSunoResponses } from "../suno-browser/responses.mjs";
const origin = "https://suno.fixture.invalid";
function fixture() {
  let options, stops = 0;
  const owner = createSunoResponses({}, origin, (_, input) => {
    options = input;
    return { ready: Promise.resolve(), async stop() { stops++; } };
  });
  const emit = (kind, text = "{}", extra = {}) => options.onResponse({
    url: () => `${origin}/api/${kind === "generate" ? "generate/v2-web/" : "feed/v3"}`,
    request: () => ({ method: () => "POST" }), status: () => 200,
    headers: () => ({ "content-type": "application/json" }), text: async () => text, ...extra,
  });
  return { owner, emit, get options() { return options; }, get stops() { return stops; } };
}
test("Suno capture chooses native page-only admission with lowered budgets and exact origin/path", async () => {
  const f = fixture();
  assert.equal(f.options.bodyBytes, 128 * 1024);
  assert.equal(f.options.binaryBodyBytes, 128 * 1024);
  assert.equal(f.options.pageOnly, true);
  assert.equal(f.options.failOnBodyError, true);
  assert.equal(f.options.requestFilter(`${origin}/api/feed/v3/?x=1`), true);
  for (const url of ["https://foreign.invalid/api/feed/v3", `${origin}/api/feed/v3/extra`, "not a URL"]) {
    assert.equal(f.options.requestFilter(url), false);
  }
  await f.owner.stop();
});
test("Suno capture ignores pre-submission feeds and non-POST before binding generation", async () => {
  const f = fixture();
  f.emit("feed", '{"wrong":true}');
  f.emit("generate", "{}", { request: () => ({ method: () => "GET" }) });
  f.emit("generate", '{"ours":true}');
  assert.deepEqual(await f.owner.next("generate", 100), { ours: true });
  f.emit("feed", '{"current":true}');
  assert.deepEqual(await f.owner.next("feed", 100), { current: true });
  await f.owner.stop();
});
test("Suno capture fails on ambiguous second generation without consuming its body", async () => {
  const f = fixture();
  f.emit("generate"); f.emit("generate", "private duplicate");
  await assert.rejects(f.owner.next("generate", 100), { code: "suno_browser_ambiguous_generation" });
  await f.owner.stop();
});
test("Suno metadata queue rejects a seventeenth unconsumed response", async () => {
  const f = fixture();
  f.emit("generate");
  for (let i = 0; i < 16; i++) f.emit("feed");
  await assert.rejects(f.owner.next("generate", 100), { code: "suno_browser_capture_queue_limit" });
  assert.ok(f.stops >= 1);
  await f.owner.stop();
});
test("Suno cumulative admission accepts exact 1MiB then refuses another native read", async () => {
  const f = fixture();
  const text = JSON.stringify({ padding: "x".repeat(128 * 1024 - 14) });
  assert.equal(Buffer.byteLength(text), 128 * 1024);
  let reads = 0;
  for (let i = 0; i < 8; i++) {
    const kind = i === 0 ? "generate" : "feed";
    f.emit(kind, text, { text: async () => { reads++; return text; } });
    await f.owner.next(kind, 100);
  }
  f.emit("feed", "{}", { text: async () => { assert.fail("read after cumulative limit"); } });
  await assert.rejects(f.owner.next("feed", 100), { code: "suno_browser_body_limit" });
  assert.equal(reads, 8);
  await f.owner.stop();
});
test("Suno capture error wins over the native empty-body settlement and omits raw details", async () => {
  const f = fixture();
  f.emit("generate", "", { text: async () => {
    f.options.onError(Object.assign(new Error("Bearer private-fixture"), { code: "gateway_program_handle_capture_limit_exceeded" }));
    return "";
  } });
  await assert.rejects(f.owner.next("generate", 100), (error) => {
    assert.equal(error.code, "suno_browser_body_limit");
    assert.ok(!error.message.includes("private-fixture")); return true;
  });
  await f.owner.stop();
});
test("Suno successful HTML is rejected before any body access", async () => {
  const f = fixture();
  f.emit("generate", "", { headers: () => ({ "content-type": "text/html" }),
    text: () => assert.fail("HTML must not be read") });
  await assert.rejects(f.owner.next("generate", 100), { code: "suno_browser_content_type" });
  await f.owner.stop();
});
test("Suno invalid JSON and HTTP errors omit response documents", async () => {
  for (const [status, text, code] of [[200, "private malformed {", "suno_browser_invalid_generation_json"],
    [503, "private upstream", "suno_browser_generation_failed"]]) {
    const f = fixture();
    f.emit("generate", text, { status: () => status });
    await assert.rejects(f.owner.next("generate", 100), (error) => {
      assert.equal(error.code, code); assert.ok(!error.message.includes("private"));
      assert.equal(error.body, undefined); return true;
    });
    await f.owner.stop();
  }
});
test("Suno capture timeout stops its owner and rejects late responses", async () => {
  const f = fixture();
  await assert.rejects(f.owner.next("generate", 10), { status: 504 });
  f.emit("generate", '{"late":true}');
  await assert.rejects(f.owner.next("generate", 100), { status: 504 });
  assert.ok(f.stops >= 1);
  await f.owner.stop();
});
test("Suno stop settles its pending waiter and is idempotent", async () => {
  const f = fixture();
  const pending = f.owner.next("generate", 1000);
  const rejected = assert.rejects(pending, { code: "suno_browser_capture_stopped" });
  await f.owner.stop(); await rejected; await f.owner.stop();
});
test("Suno redirect does not admit a successful document", async () => {
  const f = fixture();
  f.emit("generate", "", { isRedirect: () => true });
  await assert.rejects(f.owner.next("generate", 100), { code: "suno_browser_redirect" });
  await f.owner.stop();
});
