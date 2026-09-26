import test from "node:test";
import assert from "node:assert/strict";
import { importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";
import { musicHarness, musicAudio, deferredMusic, flushMusic } from "./gemini-canvas-browserless.music-fixtures.mjs";

const app = await importTestableProbe();
const listenerCount = (socket) => [...socket.listeners.values()].reduce((sum, listeners) => sum + listeners.size, 0);

test("browserless music timeout freezes events and blocks late frames and assets", async () => {
  const h = await musicHarness(app);
  await h.fireTimeout();
  const recorded = structuredClone(h.jsonWrites[1].value);
  await h.socket.emit("open");
  await h.socket.emit("message", { data: '{"setupComplete":true}' });
  await h.socket.emit("message", { data: musicAudio() });
  assert.deepEqual(h.sent, []);
  assert.deepEqual(h.writes, []);
  assert.deepEqual(structuredClone(h.jsonWrites[1].value), recorded);
  assert.equal(h.counts.close, 1);
  assert.equal(listenerCount(h.socket), 0);
});

test("browserless music pending body read cannot publish after timeout", async () => {
  const h = await musicHarness(app), body = deferredMusic();
  const reading = h.socket.emit("message", { data: { text: () => body.promise } });
  await flushMusic();
  await h.fireTimeout();
  body.resolve(musicAudio());
  await reading;
  await flushMusic();
  assert.deepEqual(h.writes, []);
  assert.deepEqual(structuredClone(h.jsonWrites[1].value.events), []);
  assert.equal(h.state.value.error, "music_ws_timeout");
});

test("browserless music timeout during raw write prevents starting the WAV write", async () => {
  const gate = deferredMusic(), h = await musicHarness(app, { writeGate: gate });
  const writing = h.socket.emit("message", { data: musicAudio("audio/l16;rate=8000") });
  await flushMusic();
  assert.equal(h.writes.length, 1);
  await h.fireTimeout();
  gate.resolve();
  await writing;
  await flushMusic();
  assert.equal(h.writes.length, 1);
  assert.equal(h.state.value.error, "music_ws_timeout");
  assert.equal(h.jsonWrites.length, 2);
});

test("browserless music overlapping audio callbacks cannot overwrite the first asset", async () => {
  const gate = deferredMusic(), h = await musicHarness(app, { writeGate: gate });
  const first = h.socket.emit("message", { data: musicAudio() });
  await flushMusic();
  await h.socket.emit("message", { data: JSON.stringify({ serverContent: { audioChunks: [{ data: "CQk=", mimeType: "audio/ogg" }] } }) });
  gate.resolve();
  await first;
  await flushMusic();
  assert.equal(h.writes.length, 1);
  assert.deepEqual(h.writes[0].bytes, Buffer.from([1, 2, 3, 4]));
  assert.equal(h.state.value.audioAsset.bytesLength, 4);
  assert.equal(h.counts.close, 1);
});

for (const failAt of ["send:1", "send:2", "send:3", "read:1", "write:1", "write:2"]) {
  test(`browserless music ${failAt} settles immediately and consumes callback rejection`, async () => {
    const h = await musicHarness(app, { failAt });
    try {
      if (failAt === "read:1") {
        await h.socket.emit("message", { data: { text: async () => { throw h.failure; } } });
      } else if (failAt.startsWith("send")) {
        await h.socket.emit("open");
        if (failAt !== "send:1") await h.socket.emit("message", { data: '{"setupComplete":true}' });
      } else {
        await h.socket.emit("message", { data: musicAudio("audio/l16;rate=8000") });
      }
      await flushMusic();
      assert.equal(h.state.settled, true);
      assert.equal(h.state.value.error, "music_ws_error");
      assert.equal(h.state.value.responseText, h.failure.message);
      assert.deepEqual(h.handlerErrors, []);
      assert.equal(h.timers.size, 0);
      assert.equal(h.counts.close, 1);
      assert.equal(listenerCount(h.socket), 0);
    } finally {
      if (!h.state.settled) await h.fireTimeout();
    }
  });
}

test("browserless music success ignores late events and detaches every owned listener", async () => {
  const h = await musicHarness(app);
  await h.socket.emit("message", { data: musicAudio() });
  const record = structuredClone(h.jsonWrites[1].value);
  await h.socket.emit("error", { message: "late failure" });
  await h.socket.emit("message", { data: musicAudio() });
  await h.socket.emit("close", { code: 1006, reason: "late close" });
  assert.equal(h.counts.close, 1); assert.equal(h.writes.length, 1);
  assert.deepEqual(structuredClone(h.jsonWrites[1].value), record);
  assert.equal(listenerCount(h.socket), 0);
});

test("browserless music synchronous close notification cannot re-enter terminal cleanup", async () => {
  const h = await musicHarness(app), close = h.socket.close.bind(h.socket);
  let notified = false;
  h.socket.close = () => {
    close();
    if (!notified) { notified = true; void h.socket.emit("close", { code: 1000, reason: "terminal close" }); }
  };
  await h.socket.emit("error", { message: "primary failure" });
  assert.equal(h.counts.close, 1);
  assert.equal(h.state.value.responseText, "primary failure");
  assert.equal(listenerCount(h.socket), 0);
});
