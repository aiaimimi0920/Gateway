import test from "node:test";
import assert from "node:assert/strict";
import path from "node:path";
import { importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";
import { musicHarness, musicAudio, flushMusic } from "./gemini-canvas-browserless.music-fixtures.mjs";

const app = await importTestableProbe();
const plain = (value) => structuredClone(value);

test("browserless music material URL normalization preserves existing host policy", () => {
  for (const value of [null, "", "not a url", "ws://localhost/path", "ws://127.0.0.1/path"]) assert.equal(app.normalizeRemoteMusicWsUrl(value), null);
  assert.equal(app.normalizeRemoteMusicWsUrl(" wss://remote.fixture.test/path "), "wss://remote.fixture.test/path");
});

for (const [label, context, expected] of [
  ["explicit", { args: { "music-ws-url": " ws://localhost/explicit " }, browserState: { musicWsUrl: "wss://material.fixture.test/ignored" } }, "ws://localhost/explicit"],
  ["browser material", { browserState: { musicWsUrl: " wss://material.fixture.test/music ", canvasProgramInvokeContract: { musicWsUrl: "wss://contract.fixture.test/ignored" } } }, "wss://material.fixture.test/music"],
  ["contract fallback", { browserState: { musicWsUrl: "ws://localhost/blocked", canvasProgramInvokeContract: { musicWsUrl: "wss://contract.fixture.test/music" } } }, "wss://contract.fixture.test/music"],
  ["default key", {}, app.DEFAULT_MUSIC_WS_URL + "?key=fixture%20key%2F%2B"],
]) {
  test(`browserless music ${label} endpoint keeps request archive before connection`, async () => {
    const h = await musicHarness(app, { context });
    assert.equal(h.socket.url, expected);
    assert.deepEqual(h.calls.slice(0, 3).map((call) => call.kind), ["json", "construct", "timer"]);
    assert.equal(h.jsonWrites[0].filePath, path.join(h.context.outDir, "music.ws.request.json"));
    assert.equal(h.jsonWrites[0].value.requestUrl, expected);
    await h.fireTimeout();
    assert.equal(h.state.value.requestUrl, expected);
    assert.equal(h.state.value.error, "music_ws_timeout");
  });
}

for (const key of ["setupComplete", "setup_complete"]) {
  test(`browserless music ${key} sends client prompt and PLAY only once`, async () => {
    const h = await musicHarness(app);
    await h.socket.emit("open");
    await h.socket.emit("message", { data: JSON.stringify({ [key]: true }) });
    await h.socket.emit("message", { data: JSON.stringify({ [key]: true }) });
    assert.deepEqual(h.sent, [{ setup: { model: "models/fixture-music" } }, { client_content: { weightedPrompts: [{ text: " fixture prompt ", weight: 1 }] } }, { playback_control: "PLAY" }]);
    const body = h.jsonWrites[0].value.requestBody;
    assert.equal(Object.hasOwn(body, "music_generation_config"), false);
    assert.equal(Object.hasOwn(body, "authuser"), false);
    await h.socket.emit("message", { data: musicAudio() });
    assert.equal(h.state.value.status, 101); assert.equal(h.state.value.ok, true);
    assert.equal(h.jsonWrites[1].value.setupComplete, true);
    assert.equal(h.timers.size, 0); assert.equal(h.counts.close, 1);
  });
}

for (const representation of ["string", "text reader", "coercion"]) {
  test(`browserless music message ${representation} accepts audio before setup`, async () => {
    const text = musicAudio(), data = representation === "string" ? text : representation === "text reader" ? { text: async () => text } : { toString: () => text };
    const h = await musicHarness(app);
    await h.socket.emit("message", { data });
    assert.equal(h.state.value.ok, true); assert.equal(h.jsonWrites[1].value.setupComplete, false);
    assert.deepEqual(h.writes[0].bytes, Buffer.from([1, 2, 3, 4]));
    assert.equal(h.state.value.audioAsset.wavPath, null); assert.deepEqual(h.sent, []);
  });
}

for (const [mime, wav] of [["audio/l16;rate=8000;channels=1", true], ["audio/pcm;rate=8000", false], ["audio/ogg", false]]) {
  test(`browserless music ${mime} preserves raw and WAV asset policy`, async () => {
    const h = await musicHarness(app);
    await h.socket.emit("message", { data: musicAudio(mime) });
    assert.equal(h.state.value.audioAsset.mimeType, mime); assert.equal(h.state.value.audioAsset.bytesLength, 4);
    assert.equal(h.writes.length, wav ? 2 : 1);
    assert.equal(h.state.value.audioAsset.rawPath, h.writes[0].filePath);
    if (wav) {
      assert.equal(h.state.value.audioAsset.wavPath, h.writes[1].filePath);
      assert.equal(h.writes[1].bytes.toString("ascii", 0, 4), "RIFF");
      assert.deepEqual([...h.writes[1].bytes.subarray(44)], [2, 1, 4, 3]);
    }
    assert.deepEqual(h.handlerErrors, []);
  });
}

test("browserless music snake chunks concatenate valid bytes and retain total chunk count", async () => {
  const h = await musicHarness(app);
  await h.socket.emit("message", { data: JSON.stringify({ server_content: { audio_chunks: [
    { data: "AQI=" }, { data: " " }, { data: "AwQ=", mime_type: "audio/ogg" },
  ] } }) });
  assert.deepEqual(h.writes[0].bytes, Buffer.from([1, 2, 3, 4]));
  assert.deepEqual(plain(h.state.value.bodySummary), { audioChunkCount: 3, audioBytes: 4, mimeType: "audio/ogg" });
});

test("browserless music empty and invalid messages retain bounded previews without assets", async () => {
  const h = await musicHarness(app);
  const invalid = "x".repeat(1400);
  await h.socket.emit("message", { data: invalid });
  await h.socket.emit("message", { data: JSON.stringify({ serverContent: { audioChunks: [{ data: " " }] } }) });
  assert.equal(h.state.settled, false); assert.deepEqual(h.writes, []);
  await h.fireTimeout();
  const record = h.jsonWrites[1].value;
  assert.equal(record.events[0], "x".repeat(1200) + "...[truncated]");
  assert.equal(record.events.length, 2);
  assert.equal(h.state.value.audioAsset, null);
});

for (const timeout of [5000, 120000]) {
  test(`browserless music timeout ${timeout} preserves capped timer and failure projection`, async () => {
    const h = await musicHarness(app, { context: { timeoutMs: timeout } });
    assert.equal([...h.timers.keys()][0].ms, Math.min(timeout, 60000));
    await h.fireTimeout();
    assert.equal(h.state.value.status, null); assert.equal(h.state.value.error, "music_ws_timeout");
    assert.deepEqual(Object.keys(h.jsonWrites[1].value).sort(), ["error", "events", "ok"]);
    assert.equal(h.jsonWrites[1].filePath, path.join(h.context.outDir, "music.ws.response.json"));
  });
}

for (const [type, event, error, message] of [["error", { message: "fixture transport" }, "music_ws_error", "fixture transport"],
  ["close", { code: 1006, reason: "fixture closed" }, "music_ws_closed_without_audio", "close code=1006 reason=fixture closed"]]) {
  test(`browserless music ${type} without audio preserves terminal error`, async () => {
    const h = await musicHarness(app);
    await h.socket.emit(type, event);
    assert.equal(h.state.value.error, error); assert.equal(h.state.value.responseText, message);
    assert.equal(h.state.value.status, null); assert.equal(h.timers.size, 0); assert.equal(h.counts.close, 1);
  });
}

for (const failAt of ["json:1", "construct:1", "json:2"]) {
  test(`browserless music outer ${failAt} rejection retains error identity`, async () => {
    const h = await musicHarness(app, { failAt });
    if (failAt === "json:2") await h.fireTimeout();
    await flushMusic();
    assert.equal(h.state.error, h.failure);
    if (failAt === "json:1") assert.equal(h.sockets.length, 0);
    if (failAt === "json:2") assert.equal(h.counts.close, 1);
  });
}
