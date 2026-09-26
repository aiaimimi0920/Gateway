import assert from "node:assert/strict";
import test from "node:test";
import { app, appFixture, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";
import { executorFixture, plain, turn } from "./gemini-canvas-browser-pool.executor-fixtures.mjs";

const url = "wss://fixture.invalid/music";
const setup = { model: "synthetic-model" };
const request = { url, jsonBody: { setup } };
const variants = [
  ["preview", "PreviewMusicNoKey", app.executeCanvasProxyPreviewNoKeyMusic],
  ["page", "PageMusicNoKey", app.executeCanvasProgramPageNoKeyMusic],
];

const audioBudgetFixture = (limit, options = {}) => {
  let configuredLimit;
  const fixture = executorFixture({
    ...options,
    prepareEvaluateInput: (input) => {
      configuredLimit = input.maxAudioBytes;
      return { ...input, maxAudioBytes: limit };
    },
  });
  return { ...fixture, get configuredLimit() { return configuredLimit; } };
};

for (const [label, prefix, execute] of variants) {
  const run = (f, input = request, timeout = 4500) => execute(f.target, input, timeout);

  test("no-key " + label + " music accepts audio at the byte limit", async () => {
    const f = audioBudgetFixture(4), pending = run(f), socket = f.sockets[0];
    await socket.message({ serverContent: { audioChunks: [{ data: "AQIDBA==" }] } });
    await f.fire(1800);
    const result = await pending;
    assert.equal(f.configuredLimit, 16 * 1024 * 1024);
    assert.equal(result.ok, true);
    assert.equal(result.bodyBase64, "AQIDBA==");
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });

  test("no-key " + label + " music stops on oversize audio without retry or further decode", async () => {
    const decoded = [];
    const f = audioBudgetFixture(4, {
      atob: (value) => {
        decoded.push(value);
        return atob(value);
      },
    });
    const pending = run(f, {
      url: "wss://generativelanguage.googleapis.com/synthetic",
      jsonBody: { setup },
    });
    const socket = f.sockets[0];
    await socket.message({ serverContent: { audioChunks: [{ data: "AQID" }] } });
    await socket.message({ serverContent: { audioChunks: [{ data: "BAU=" }] } });
    const result = await pending;
    await socket.message({ serverContent: { audioChunks: [{ data: "Bg==" }] } });
    assert.equal(result.status, 413);
    assert.equal(result.code, "gemini_canvas_browser_body_too_large");
    assert.equal(result.errorName, prefix + "BodyTooLarge");
    assert.match(result.errorMessage, /5 bytes/);
    assert.equal(result.bodyBase64, undefined);
    assert.deepEqual(decoded, ["AQID"]);
    assert.equal(f.sockets.length, 1);
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });
  test(`no-key ${label} music guards delayed audio after timeout`, async () => {
    const f = executorFixture(), pending = run(f), socket = f.sockets[0];
    let releaseText;
    const late = socket.emit("message", { data: { text: () => new Promise((resolve) => { releaseText = resolve; }) } });
    await f.fire(4500);
    const result = await pending;
    releaseText('{"server_content":{"audio_chunks":[{"data":"AQI="}]}}');
    await late;
    assert.equal(result.errorName, prefix + "Timeout");
    assert.equal(result.bodyBase64, undefined);
    assert.equal(f.timers.size, 0);
    assert.equal(socket.closeCalls, 1);
  });

  test(`no-key ${label} music guards delayed setup after settlement`, async () => {
    const f = executorFixture();
    const pending = run(f, { url, jsonBody: { setup, client_content: { turns: [] }, music_generation_config: { temperature: 1 } } });
    const socket = f.sockets[0];
    let releaseText;
    const late = socket.emit("message", { data: { text: () => new Promise((resolve) => { releaseText = resolve; }) } });
    await socket.emit("error", { message: "settled before decoding" });
    const result = await pending;
    releaseText('{"setup_complete":true}');
    await late;
    assert.equal(result.errorName, prefix + "WsError");
    assert.equal(result.setupComplete, false);
    assert.equal(socket.sent.length, 0);
    assert.equal(f.timers.size, 0);
    assert.equal(socket.closeCalls, 1);
  });

  test(`no-key ${label} music guards queued open after settlement`, async () => {
    const f = executorFixture(), pending = run(f), socket = f.sockets[0];
    await f.fire(4500);
    const result = await pending;
    await socket.emit("open");
    assert.equal(result.errorName, prefix + "Timeout");
    assert.equal(socket.sent.length, 0);
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music guards queued message decoding after settlement`, async () => {
    const f = executorFixture(), pending = run(f), socket = f.sockets[0];
    let decodes = 0;
    await f.fire(4500);
    const result = await pending;
    await socket.emit("message", { data: { text: async () => {
      decodes += 1; return '{"server_content":{"audio_chunks":[{"data":"AQI="}]}}';
    } } });
    assert.equal(result.errorName, prefix + "Timeout");
    assert.equal(decodes, 0);
    assert.equal(f.timers.size, 0);
    assert.equal(socket.closeCalls, 1);
  });

  test(`no-key ${label} music sends handshake frames once in protocol order`, async () => {
    const f = executorFixture();
    const content = { turns: [{ text: "synthetic" }] }, config = { temperature: 0.5 };
    const pending = run(f, { url, jsonBody: { setup, client_content: content, music_generation_config: config, playback_control: " PAUSE " }, bodyText: "invalid-json" });
    const socket = f.sockets[0];
    await socket.emit("open");
    assert.deepEqual(socket.sent, [{ setup }]);
    await socket.message({ setupComplete: {} });
    await socket.message({ setup_complete: true });
    assert.deepEqual(socket.sent, [{ setup }, { client_content: content }, { music_generation_config: config }, { playback_control: "PAUSE" }]);
    await socket.message({ serverContent: { audioChunks: [{ data: "AQI=", mimeType: "audio/pcm" }] } });
    await f.fire(1800);
    const result = await pending;
    assert.equal(result.setupComplete, true);
    assert.equal(result.bodyBase64, "AQI=");
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
    await socket.emit("error", { message: "late error" });
    assert.equal(socket.closeCalls, 1);
  });

  test(`no-key ${label} music merges binary chunks and bounds diagnostic events`, async () => {
    const f = executorFixture(), large = Buffer.alloc(40001, 172);
    const pending = run(f);
    const socket = f.sockets[0];
    await socket.emit("open");
    await socket.emit("message", { data: { text: async () => '{"setup_complete":true}' } });
    for (let index = 0; index < 45; index += 1) await socket.emit("message", { data: `${index}:` + "x".repeat(1400) });
    await socket.message({ server_content: { audio_chunks: [{ data: " " }, { data: large.toString("base64"), mime_type: "audio/pcm" }] } });
    await socket.message({ serverContent: { audioChunks: [{ data: "AwQ=" }] } });
    assert.deepEqual([...f.timers.values()].map((timer) => timer.ms).sort((a, b) => a - b), [1800, 4500]);
    await f.fire(1800);
    const result = await pending;
    assert.equal(result.status, 101);
    assert.equal(result.contentType, "audio/pcm");
    assert.deepEqual(Buffer.from(result.bodyBase64, "base64"), Buffer.concat([large, Buffer.from([3, 4])]));
    assert.deepEqual(JSON.parse(result.bodyText), { audioChunkCount: 2, mimeType: "audio/pcm" });
    assert.equal(result.events.length, 20);
    assert.ok(result.events.every((event) => event.length <= 1200));
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music returns audio and close metadata when the socket closes`, async () => {
    const f = executorFixture(), pending = run(f), socket = f.sockets[0];
    await socket.message({ serverContent: { audioChunks: [{ data: "AQI=" }] } });
    await socket.emit("close", { code: 1001, reason: "synthetic done" });
    const result = await pending;
    assert.equal(result.ok, true);
    assert.equal(result.status, 101);
    assert.equal(result.bodyBase64, "AQI=");
    assert.deepEqual(JSON.parse(result.bodyText), { audioChunkCount: 1, mimeType: "audio/L16;codec=pcm;rate=48000;channels=2", closeCode: 1001, closeReason: "synthetic done" });
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music maps a socket close before audio`, async () => {
    const f = executorFixture(), pending = run(f), socket = f.sockets[0];
    await socket.emit("close", { code: 1008, reason: "fixture policy" });
    const result = await pending;
    assert.equal(result.status, 599);
    assert.equal(result.errorName, prefix + "WsClosed");
    assert.equal(result.errorMessage, "music websocket closed before audio: code=1008 reason=fixture policy");
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music handles both filtered-prompt field spellings`, async () => {
    for (const field of ["filteredPrompt", "filtered_prompt"]) {
      const f = executorFixture(), pending = run(f), socket = f.sockets[0];
      await socket.message({ [field]: "synthetic filter" });
      const result = await pending;
      assert.equal(result.status, 400);
      assert.equal(result.errorName, prefix + "FilteredPrompt");
      assert.equal(result.errorMessage, "synthetic filter");
      assert.equal(socket.closeCalls, 1);
      assert.equal(f.timers.size, 0);
    }
  });

  test(`no-key ${label} music maps socket errors even when closing throws`, async () => {
    const f = executorFixture({ closeError: new Error("close failed") }), pending = run(f), socket = f.sockets[0];
    await socket.emit("error", { error: new Error("synthetic websocket failure") });
    const result = await pending;
    assert.equal(result.errorName, prefix + "WsError");
    assert.equal(result.errorMessage, "synthetic websocket failure");
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music total deadline clears the audio idle deadline`, async () => {
    const f = executorFixture(), pending = run(f, request, 99), socket = f.sockets[0];
    await socket.message({ serverContent: { audioChunks: [{ data: "AQI=" }] } });
    assert.equal(f.timers.size, 2);
    await f.fire(99);
    const result = await pending;
    assert.equal(result.ok, false);
    assert.equal(result.errorName, prefix + "Timeout");
    assert.equal(result.status, 599);
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music retries the empty-key URL and decodes bodyText requests`, async () => {
    const f = executorFixture(), googleUrl = "wss://generativelanguage.googleapis.com/synthetic";
    const pending = run(f, { url: googleUrl, bodyText: JSON.stringify({ setup, music_generation_config: {} }) });
    const first = f.sockets[0];
    assert.equal(first.url, googleUrl + "?key=");
    await first.emit("error", { message: "first failed" });
    await turn();
    assert.equal(first.closeCalls, 1);
    assert.equal(f.sockets.length, 2);
    const second = f.sockets[1];
    assert.equal(second.url, googleUrl);
    assert.equal(f.timers.size, 1);
    await second.emit("open");
    await second.message({ setup_complete: true });
    assert.deepEqual(second.sent, [{ setup }, { playback_control: "PLAY" }]);
    await second.emit("close", { code: 1006, reason: "second failed" });
    const result = await pending;
    assert.equal(result.errorName, prefix + "WsClosed");
    assert.equal(result.setupComplete, true);
    assert.equal(second.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music rejects missing setup and invalid serialized bodies`, async () => {
    const f = executorFixture(), pending = run(f, { url, bodyText: "invalid-json" }), socket = f.sockets[0];
    await socket.emit("open");
    const result = await pending;
    assert.equal(result.status, 400);
    assert.equal(result.errorName, prefix + "InvalidRequest");
    assert.deepEqual(socket.sent, []);
    assert.equal(socket.closeCalls, 1);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music returns its initial failure when the URL is empty`, async () => {
    const f = executorFixture();
    const result = await run(f, { ...request, url: " " });
    assert.equal(result.errorName, prefix + "Error");
    assert.equal(result.status, 599);
    assert.deepEqual(plain(result.events), []);
    assert.equal(f.sockets.length, 0);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music preserves constructor rejection before timer allocation`, async () => {
    const error = new Error("synthetic constructor rejection");
    const f = executorFixture({ constructorError: error });
    await assert.rejects(run(f), (actual) => actual === error);
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music malformed audio leaves the attempt pending until its deadline`, async () => {
    const f = executorFixture(), pending = run(f), socket = f.sockets[0];
    // Existing async-listener decode rejection is preserved, not repaired here.
    await assert.rejects(socket.message({ serverContent: { audioChunks: [{ data: "not-base64!" }] } }), (error) => error.name === "InvalidCharacterError");
    assert.equal(socket.closeCalls, 0);
    assert.equal(f.timers.size, 1);
    await f.fire(4500);
    assert.equal((await pending).errorName, prefix + "Timeout");
    assert.equal(f.timers.size, 0);
  });

  test(`no-key ${label} music executes a self-contained callback in a real offline browser`, browserTest, async (t) => {
    const f = await appFixture(t);
    await f.page.evaluate(() => {
      window.fixtureSockets = [];
      window.WebSocket = class {
        constructor(url) {
          this.url = url; this.sent = []; this.listeners = {}; this.closes = 0;
          window.fixtureSockets.push(this);
          queueMicrotask(() => this.emit("open", {}));
        }
        addEventListener(type, callback) { this.listeners[type] = callback; }
        emit(type, event) { this.listeners[type]?.(event); }
        send(text) {
          const value = JSON.parse(text); this.sent.push(value);
          if (value.setup) queueMicrotask(() => {
            this.emit("message", { data: '{"setup_complete":true}' });
            this.emit("message", { data: '{"server_content":{"audio_chunks":[{"data":"AQID"}]}}' });
            this.emit("close", { code: 1000, reason: "offline done" });
          });
        }
        close() { this.closes += 1; }
      };
    });
    const result = await execute(label === "preview" ? f.page.mainFrame() : f.page, request, 5000);
    assert.equal(result.ok, true);
    assert.equal(result.bodyBase64, "AQID");
    const sockets = await f.page.evaluate(() => window.fixtureSockets.map(({ url, sent, closes }) => ({ url, sent, closes })));
    assert.deepEqual(sockets, [{ url, sent: [{ setup }, { playback_control: "PLAY" }], closes: 1 }]);
  });
}
