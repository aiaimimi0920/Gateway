import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { PassThrough } from "node:stream";
import { fileURLToPath } from "node:url";
import test, { mock } from "node:test";
import { readStdin } from "../aistudio-live-probe/cli-io.mjs";

const probe = fileURLToPath(new URL("../probe-aistudio-live-request.mjs", import.meta.url));

test("AI Studio rejects invalid JSON without publishing input secrets", () => {
  const secret = "S3CR3T42";
  const result = spawnSync(process.execPath, [probe], {
    input: secret, encoding: "utf8", timeout: 10000,
  });
  assert.equal(result.status, 1, result.stderr);
  assert.equal(result.stdout.includes(secret), false);
  assert.equal(result.stderr.includes(secret), false);
  const output = JSON.parse(result.stdout);
  assert.equal(output.error.code, "aistudio_probe_invalid_json");
  assert.equal(output.error.message, "AI Studio probe input must be valid JSON.");
});

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
    for (const event of ["data", "end", "error", "close"]) {
      assert.equal(stream.listenerCount(event), 0);
    }
  };
  try { await run({ stream, pending, expire: () => expire(), cleaned }); } finally {
    timeout.mock.restore();
    clear.mock.restore();
    stream.destroy();
  }
}

test("AI Studio stdin preserves split UTF8 and whitespace and strips leading BOM", async () => {
  await withReader(async ({ stream, pending, cleaned }) => {
    const value = ' {"text":"\u4f60\u597d"} ';
    for (const byte of Buffer.from(`\ufeff${value}`)) stream.write(Buffer.from([byte]));
    stream.end();
    assert.equal(await pending, value);
    cleaned();
  });
});

test("AI Studio stdin accepts empty and exact-limit input", async () => {
  for (const size of [0, 16 * 1024 * 1024]) {
    await withReader(async ({ stream, pending, cleaned }) => {
      stream.end(Buffer.alloc(size, 32));
      assert.equal((await pending).length, size);
      cleaned();
    });
  }
});

test("AI Studio stdin rejects cumulative raw byte overflow", async () => {
  await withReader(async ({ stream, pending, cleaned }) => {
    const rejected = assert.rejects(pending, { status: 413, code: "aistudio_probe_input_too_large" });
    stream.write(Buffer.alloc(16 * 1024 * 1024, 32));
    stream.write("x");
    await rejected;
    cleaned();
  });
});

test("AI Studio stdin deadline releases an unfinished pipe", async () => {
  await withReader(async ({ stream, pending, expire, cleaned }) => {
    const rejected = assert.rejects(pending, { status: 408, code: "aistudio_probe_input_timeout" });
    stream.write("{");
    expire();
    await rejected;
    cleaned();
  });
});

test("AI Studio stdin errors and premature close settle once", async () => {
  for (const event of ["error", "close"]) {
    await withReader(async ({ stream, pending, expire, cleaned }) => {
      const failure = new Error("synthetic pipe failure");
      const rejected = assert.rejects(pending, (error) => event === "error"
        ? error === failure : error.code === "aistudio_probe_input_closed");
      stream.emit(event, failure);
      await rejected;
      expire();
      cleaned();
    });
  }
});
