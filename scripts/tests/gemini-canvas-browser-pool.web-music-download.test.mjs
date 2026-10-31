import assert from "node:assert/strict";
import { EventEmitter } from "node:events";
import test from "node:test";
import { downloadWebMusic } from "../gemini-canvas-browser-pool-web-music-download.mjs";

function fixture({ visible = true, bytes = Buffer.from("ID3\x04\0\0\0\0\0\0track"), failClick = false, pending = false, menuDelayMs = 0 } = {}) {
  const page = new EventEmitter(), calls = [];
  const download = {
    async path() { return "owned-fixture-download"; }, url() { return "https://fixture.invalid/audio.mp3"; },
    async cancel() { calls.push("cancel"); }, async delete() { calls.push("delete"); },
  };
  page.getByRole = (role, { name }) => {
    const locator = { first() { return this; }, last() { return this; }, async isVisible() { return visible; }, async click() {
      if (failClick) throw new Error("fixture menu unavailable");
      if (role === "button") {
        assert.match("下载音乐作品", name); calls.push("menu");
        if (menuDelayMs) await new Promise(resolve => setTimeout(resolve, menuDelayMs));
      }
      else { assert.equal(role, "menuitem"); assert.match("纯音频 / MP3 音轨", name); calls.push("audio"); if (!pending) page.emit("download", download); }
    } };
    return locator;
  };
  page.getByText = name => page.getByRole("menuitem", { name: /纯音频|MP3 音轨/ });
  return { page, calls, read: async path => { assert.equal(path, "owned-fixture-download"); return bytes; } };
}

test("native Web music download chooses the audio menu and releases only its own listener/file", async () => {
  const h = fixture(), inherited = () => {};
  h.page.on("download", inherited);
  const result = await downloadWebMusic(h.page, 1000, h.read);
  assert.equal(result.kind, "audio"); assert.equal(result.mimeType, "audio/mpeg");
  assert.equal(Buffer.from(result.bodyBase64, "base64").subarray(0, 3).toString(), "ID3");
  assert.deepEqual(h.calls, ["menu", "audio", "delete"]);
  assert.deepEqual(h.page.listeners("download"), [inherited]);
});

for (const bytes of [Buffer.alloc(0), Buffer.from("<!doctype html>not audio")]) {
  test("native music refuses empty/HTML downloads and cleans up", async () => {
    const h = fixture({ bytes });
    await assert.rejects(downloadWebMusic(h.page, 1000, h.read), { code: "gemini_web_music_download_invalid_audio" });
    assert.deepEqual(h.calls, ["menu", "audio", "cancel", "delete"]);
    assert.equal(h.page.listenerCount("download"), 0);
  });
}

test("missing music download control causes no click, listener, or file read", async () => {
  const h = fixture({ visible: false });
  assert.equal(await downloadWebMusic(h.page, 1000, () => { throw new Error("must not read"); }), null);
  assert.deepEqual(h.calls, []); assert.equal(h.page.listenerCount("download"), 0);
});

test("menu errors and timeouts do not leave download listeners or resubmit prompts", async () => {
  for (const options of [{ failClick: true }, { pending: true }]) {
    const h = fixture(options);
    await assert.rejects(downloadWebMusic(h.page, 10, h.read));
    assert.equal(h.page.listenerCount("download"), 0);
    assert.ok(h.calls.length <= 2);
  }
});

test("oversized payloads and file read failures release their download resources", async () => {
  for (const failRead of [false, true]) {
    const h = fixture({ bytes: Buffer.alloc(16 * 1024 * 1024 + 1) });
    const read = failRead ? async () => { throw new Error("fixture read failed"); } : h.read;
    await assert.rejects(downloadWebMusic(h.page, 1000, read), failRead
      ? /fixture read failed/ : { code: "gemini_canvas_browser_body_too_large", status: 413 });
    assert.deepEqual(h.calls, ["menu", "audio", "cancel", "delete"]);
    assert.equal(h.page.listenerCount("download"), 0);
  }
});

test("an exhausted music result budget never starts a fresh download wait", async () => {
  for (const timeout of [0, -10, Number.NaN]) {
    const h = fixture();
    await assert.rejects(downloadWebMusic(h.page, timeout, h.read), {
      status: 504, code: "gemini_web_music_download_timeout",
    });
    assert.deepEqual(h.calls, []);
    assert.equal(h.page.listenerCount("download"), 0);
  }
});

test("a slow music menu cannot spend another full budget or click audio after expiry", async () => {
  const h = fixture({ menuDelayMs: 40 });
  await assert.rejects(downloadWebMusic(h.page, 10, h.read), {
    status: 504, code: "gemini_web_music_download_timeout",
  });
  await new Promise(resolve => setTimeout(resolve, 50));
  assert.deepEqual(h.calls, ["menu"]);
  assert.equal(h.page.listenerCount("download"), 0);
});
