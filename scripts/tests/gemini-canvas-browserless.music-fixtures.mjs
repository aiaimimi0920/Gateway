import path from "node:path";
import vm from "node:vm";

export const flushMusic = async () => { for (let index = 0; index < 12; index += 1) await Promise.resolve(); };
export function deferredMusic() {
  let resolve, reject;
  const promise = new Promise((yes, no) => { resolve = yes; reject = no; });
  return { promise, resolve, reject };
}
export const musicAudio = (mimeType = "audio/ogg") => JSON.stringify({ serverContent: {
  audioChunks: [{ data: Buffer.from([1, 2, 3, 4]).toString("base64"), mimeType }],
} });

export async function musicHarness(app, options = {}) {
  const calls = [], jsonWrites = [], writes = [], sent = [], sockets = [], handlerErrors = [], timers = new Map(), counts = {};
  const failure = new Error("fixture music boundary failed");
  const context = { args: {}, browserState: {}, outDir: "fixture-music-output", apiKey: "fixture key/+",
    model: "fixture-music", prompt: " fixture prompt ", timeoutMs: 120000, ...options.context };
  const boundary = (kind, details = {}) => {
    counts[kind] = (counts[kind] ?? 0) + 1;
    calls.push({ kind, ...details });
    if (options.failAt === `${kind}:${counts[kind]}`) throw failure;
  };
  class FixtureSocket {
    constructor(url) { boundary("construct", { url }); this.url = url; this.listeners = new Map(); sockets.push(this); }
    addEventListener(type, listener) {
      const handlers = this.listeners.get(type) ?? new Set(); handlers.add(listener); this.listeners.set(type, handlers);
    }
    removeEventListener(type, listener) { this.listeners.get(type)?.delete(listener); }
    send(text) { boundary("send", { text }); sent.push(JSON.parse(text)); }
    close() { boundary("close"); if (options.closeEmits) void this.emit("close", { code: 1000, reason: "fixture close" }); }
    async emit(type, event = {}) {
      const handlers = [...(this.listeners.get(type) ?? [])];
      await Promise.all(handlers.map(async (listener) => {
        try { await listener(event); } catch (error) { handlerErrors.push(error); }
      }));
      await flushMusic();
    }
  }
  const dependencies = { ...app, path, Buffer, Error, WebSocket: FixtureSocket,
    setTimeout(callback, ms) { const timer = { callback, ms }; boundary("timer", { ms }); timers.set(timer, callback); return timer; },
    clearTimeout(timer) { boundary("clear"); timers.delete(timer); },
    async writeJson(filePath, value) { boundary("json", { filePath, value }); jsonWrites.push({ filePath, value }); },
    async writeBuffer(filePath, bytes) {
      boundary("write", { filePath, bytes }); writes.push({ filePath, bytes });
      if (options.writeGate && writes.length === 1) await options.writeGate.promise;
    },
  };
  const probe = vm.runInNewContext(`(${app.probeMusic.toString()})`, dependencies, { timeout: 1000 });
  const state = { settled: false, value: null, error: null };
  const pending = probe(context);
  pending.then((value) => { state.value = value; state.settled = true; }, (error) => { state.error = error; state.settled = true; });
  await flushMusic();
  const fireTimeout = async () => {
    for (const [timer, callback] of [...timers]) { timers.delete(timer); callback(); }
    await flushMusic();
  };
  return { context, calls, jsonWrites, writes, sent, sockets, socket: sockets[0], handlerErrors, timers, counts, failure, state, pending, fireTimeout };
}
