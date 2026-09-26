import assert from "node:assert/strict";
import { PassThrough } from "node:stream";
import test, { mock } from "node:test";
import { readStdin } from "../producer-browser/stdin.mjs";

async function withReader(run) {
  const stream = new PassThrough();
  let expire;
  let cleared = false;
  const timeout = mock.method(globalThis, "setTimeout", (callback, ms) => {
    assert.equal(ms, 30000);
    expire = callback;
    return 41;
  });
  const clear = mock.method(globalThis, "clearTimeout", (id) => {
    assert.equal(id, 41);
    cleared = true;
  });
  const pending = readStdin(stream);
  const cleaned = () => {
    assert.equal(cleared, true);
    assert.equal(stream.isPaused(), true);
    for (const event of ["data", "end", "error", "close"]) assert.equal(stream.listenerCount(event), 0);
  };
  try { await run({ stream, pending, expire: () => expire(), cleaned }); } finally {
    timeout.mock.restore();
    clear.mock.restore();
    stream.destroy();
  }
}

test("Producer stdin preserves raw whitespace and UTF8 split across byte chunks", async () => {
  await withReader(async ({ stream, pending, expire, cleaned }) => {
    const value = ' {"text":"\u4f60\u597d"} ';
    for (const byte of Buffer.from(value)) stream.write(Buffer.from([byte]));
    stream.end();
    assert.equal(await pending, value);
    expire();
    cleaned();
  });
});

test("Producer stdin accepts empty and exact-limit raw input without fabricated JSON", async () => {
  for (const size of [0, 16 * 1024 * 1024]) {
    await withReader(async ({ stream, pending, cleaned }) => {
      stream.end(Buffer.alloc(size, 32));
      const value = await pending;
      assert.equal(value.length, size);
      if (size) assert.equal(value.trim(), "");
      cleaned();
    });
  }
});

test("Producer stdin rejects raw byte overflow and releases its pipe listeners", async () => {
  await withReader(async ({ stream, pending, cleaned }) => {
    const rejected = assert.rejects(pending, { code: "producer_browser_input_too_large", status: 413 });
    stream.write(Buffer.alloc(16 * 1024 * 1024 + 1, 32));
    await rejected;
    cleaned();
  });
});

test("Producer stdin EOF deadline rejects an open pipe without waiting for browser setup", async () => {
  await withReader(async ({ stream, pending, expire, cleaned }) => {
    const rejected = assert.rejects(pending, { code: "producer_browser_input_timeout", status: 408 });
    stream.write("{");
    expire();
    await rejected;
    cleaned();
  });
});

test("Producer stdin pipe errors and premature close settle once and clear the deadline", async () => {
  for (const event of ["error", "close"]) {
    await withReader(async ({ stream, pending, expire, cleaned }) => {
      const failure = new Error("synthetic pipe error");
      const rejected = assert.rejects(pending, (error) => event === "error" ? error === failure : error.code === "producer_browser_input_closed");
      stream.emit(event, failure);
      await rejected;
      expire();
      cleaned();
    });
  }
});
