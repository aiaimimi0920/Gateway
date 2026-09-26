import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { PassThrough } from "node:stream";
import test from "node:test";
import vm from "node:vm";

const source = await readFile(new URL("../chatgpt-web-session-worker.mjs", import.meta.url), "utf8");
const body = source.slice(source.indexOf("async function readStdin()"), source.indexOf("function printAndExit("));

function fixture() {
  const stdin = new PassThrough();
  let deadline;
  let timerCleared = false;
  const read = vm.runInNewContext(`(${body})`, {
    process: { stdin }, Buffer,
    createWorkerError: (status, code, message) => Object.assign(new Error(message), { status, code }),
    setTimeout: (callback, ms) => { assert.equal(ms, 30_000); deadline = callback; return 1; },
    clearTimeout: () => { timerCleared = true; },
  });
  return { stdin, read, expire: () => { assert.equal(typeof deadline, "function"); deadline(); },
    cleaned: () => {
      assert.equal(timerCleared, true);
      for (const event of ["data", "end", "error", "close"]) assert.equal(stdin.listenerCount(event), 0);
    } };
}

test("stdin preserves split UTF8 and removes listeners after end", async () => {
  const f = fixture();
  const pending = f.read();
  const payload = Buffer.from(' {"text":"\u4f60\u597d"} ');
  for (const byte of payload) f.stdin.write(Buffer.from([byte]));
  f.stdin.end();
  assert.equal(await pending, '{"text":"\u4f60\u597d"}');
  f.cleaned();
});

test("stdin rejects over16MiB before concatenating a complete request", async () => {
  const f = fixture();
  const pending = f.read();
  const rejected = assert.rejects(pending, (error) => error.code === "chatgpt_web_input_too_large");
  f.stdin.write(Buffer.alloc(16 * 1024 * 1024 + 1, 32));
  f.stdin.end();
  await rejected;
  f.cleaned();
});

test("stdin read deadline rejects even when sender never closes its pipe", async () => {
  const f = fixture();
  const pending = f.read();
  const rejected = assert.rejects(pending, (error) => error.code === "chatgpt_web_input_timeout");
  f.expire();
  await rejected;
  f.cleaned();
  f.stdin.destroy();
});

test("empty and exact-limit stdin are accepted without reading uninitialized buffer bytes", async () => {
  for (const payload of [Buffer.alloc(0), Buffer.alloc(16 * 1024 * 1024, 32)]) {
    const f = fixture();
    const pending = f.read();
    f.stdin.end(payload);
    assert.equal(await pending, "{}");
    f.cleaned();
  }
});

test("stdin stream error and premature close release listeners and deadline", async () => {
  for (const event of ["error", "close"]) {
    const f = fixture();
    const pending = f.read();
    const error = new Error("synthetic pipe error");
    const rejected = assert.rejects(pending, (received) => event === "error" ? received === error : received.code === "chatgpt_web_input_closed");
    f.stdin.emit(event, error);
    await rejected;
    f.cleaned();
    f.stdin.destroy();
  }
});
