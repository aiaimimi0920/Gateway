import assert from "node:assert/strict";
import test from "node:test";
import {
  assertBytesWithinLimit,
  assertDeclaredBodyWithinLimit,
  assertTextWithinLimit,
  BROWSER_POOL_BINARY_BODY_LIMIT_BYTES,
  BROWSER_POOL_BODY_LIMIT_CODE,
  BROWSER_POOL_BODY_SIZE_UNKNOWN_CODE,
  BROWSER_POOL_TEXT_BODY_LIMIT_BYTES,
  readFetchResponseBody,
  readPlaywrightResponseBody,
  readPlaywrightResponseText,
} from "../gemini-canvas-browser-pool-body.mjs";

test("browser-pool body limits are finite and text stays below the binary budget", () => {
  assert.ok(BROWSER_POOL_TEXT_BODY_LIMIT_BYTES > 0);
  assert.ok(BROWSER_POOL_BINARY_BODY_LIMIT_BYTES > BROWSER_POOL_TEXT_BODY_LIMIT_BYTES);
  assert.equal(assertTextWithinLimit("é", BROWSER_POOL_TEXT_BODY_LIMIT_BYTES), "é");
  assert.throws(
    () => assertTextWithinLimit("x".repeat(BROWSER_POOL_TEXT_BODY_LIMIT_BYTES + 1), BROWSER_POOL_TEXT_BODY_LIMIT_BYTES),
    (error) => error.code === BROWSER_POOL_BODY_LIMIT_CODE && error.status === 413,
  );
});

test("declared Playwright length rejects before invoking a native body reader", async () => {
  let textReads = 0;
  let bodyReads = 0;
  const response = {
    headers: () => ({ "content-length": String(BROWSER_POOL_BINARY_BODY_LIMIT_BYTES + 1) }),
    async text() { textReads++; return "unreachable"; },
    async body() { bodyReads++; return Buffer.from("unreachable"); },
  };
  assert.throws(
    () => assertDeclaredBodyWithinLimit(response.headers(), BROWSER_POOL_BINARY_BODY_LIMIT_BYTES),
    (error) => error.code === BROWSER_POOL_BODY_LIMIT_CODE,
  );
  await assert.rejects(
    readPlaywrightResponseText(response, BROWSER_POOL_TEXT_BODY_LIMIT_BYTES),
    (error) => error.code === BROWSER_POOL_BODY_LIMIT_CODE,
  );
  await assert.rejects(
    readPlaywrightResponseBody(response, BROWSER_POOL_BINARY_BODY_LIMIT_BYTES),
    (error) => error.code === BROWSER_POOL_BODY_LIMIT_CODE,
  );
  assert.equal(textReads, 0);
  assert.equal(bodyReads, 0);
});

test("Playwright whole-body reads fail closed when response size is unknown", async () => {
  let textReads = 0;
  let bodyReads = 0;
  const response = {
    headers: () => ({}),
    status: () => 200,
    request: () => ({ method: () => "GET" }),
    async text() { textReads++; return "unreachable"; },
    async body() { bodyReads++; return Buffer.from("unreachable"); },
  };
  await assert.rejects(
    readPlaywrightResponseText(response),
    (error) => error.code === BROWSER_POOL_BODY_SIZE_UNKNOWN_CODE && error.status === 502,
  );
  await assert.rejects(
    readPlaywrightResponseBody(response),
    (error) => error.code === BROWSER_POOL_BODY_SIZE_UNKNOWN_CODE && error.status === 502,
  );
  assert.equal(textReads, 0);
  assert.equal(bodyReads, 0);
});

test("Playwright HEAD responses skip whole-body reads even with a representation length", async () => {
  let bodyReads = 0;
  const response = {
    headers: () => ({ "content-length": String(BROWSER_POOL_BINARY_BODY_LIMIT_BYTES + 1) }),
    status: () => 200,
    request: () => ({ method: () => "HEAD" }),
    async body() { bodyReads++; return Buffer.from("unreachable"); },
  };
  assert.deepEqual(await readPlaywrightResponseBody(response), new Uint8Array(0));
  assert.equal(bodyReads, 0);
});

test("Playwright zero length declarations do not suppress or bypass actual body validation", async () => {
  let textReads = 0;
  let bodyReads = 0;
  const response = {
    headers: () => ({ "content-length": "0" }),
    status: () => 200,
    request: () => ({ method: () => "GET" }),
    async text() { textReads++; return "body"; },
    async body() { bodyReads++; return Buffer.from([1, 2]); },
  };
  assert.equal(await readPlaywrightResponseText(response, 4), "body");
  assert.deepEqual(await readPlaywrightResponseBody(response, 2), Buffer.from([1, 2]));
  await assert.rejects(
    readPlaywrightResponseText({ ...response, async text() { textReads++; return "oversized"; } }, 4),
    (error) => error.code === BROWSER_POOL_BODY_LIMIT_CODE && error.status === 413,
  );
  assert.equal(textReads, 2);
  assert.equal(bodyReads, 1);
});

test("Fetch response streams retain unknown-length bodies only through the fixed byte limit", async () => {
  let index = 0, reads = 0, cancellations = 0, releases = 0;
  const chunks = [Uint8Array.of(1, 2), Uint8Array.of(3)];
  const response = {
    headers: new Headers(),
    body: { getReader: () => ({
      async read() { reads++; return index < chunks.length ? { done: false, value: chunks[index++] } : { done: true }; },
      async cancel() { cancellations++; },
      releaseLock() { releases++; },
    }) },
    async arrayBuffer() { assert.fail("stream path must not read the whole body"); },
  };
  assert.deepEqual([...await readFetchResponseBody(response, 3, "fixture")], [1, 2, 3]);
  assert.equal(reads, 3);
  assert.equal(cancellations, 0);
  assert.equal(releases, 1);
});

test("Fetch response stream cancels on overflow before reading later chunks", async () => {
  let index = 0, reads = 0, cancellations = 0, releases = 0, wholeBodyReads = 0;
  const chunks = [Uint8Array.of(1, 2, 3), Uint8Array.of(4), Uint8Array.of(5)];
  const response = {
    headers: new Headers(),
    body: { getReader: () => ({
      async read() { reads++; return { done: false, value: chunks[index++] }; },
      async cancel() { cancellations++; },
      releaseLock() { releases++; },
    }) },
    async arrayBuffer() { wholeBodyReads++; return new ArrayBuffer(0); },
  };
  await assert.rejects(
    readFetchResponseBody(response, 3, "fixture"),
    (error) => error.code === BROWSER_POOL_BODY_LIMIT_CODE && error.status === 413,
  );
  assert.equal(reads, 2);
  assert.equal(cancellations, 1);
  assert.equal(releases, 1);
  assert.equal(wholeBodyReads, 0);
});

test("Fetch legacy whole-body fallback rejects unknown size before reading", async () => {
  let reads = 0;
  await assert.rejects(
    readFetchResponseBody({ headers: new Headers(), async arrayBuffer() { reads++; return new ArrayBuffer(0); } }, 3, "fixture"),
    (error) => error.code === BROWSER_POOL_BODY_SIZE_UNKNOWN_CODE && error.status === 502,
  );
  assert.equal(reads, 0);
});

test("Fetch response legacy adapter preserves bounded declared-length behavior", async () => {
  let reads = 0;
  const response = {
    headers: new Headers({ "content-length": "3" }),
    async arrayBuffer() {
      reads++;
      return Uint8Array.from([1, 2, 3]).buffer;
    },
  };
  const bytes = await readFetchResponseBody(response, 3, "fixture");
  assert.deepEqual([...bytes], [1, 2, 3]);
  assert.equal(reads, 1);
  await assert.rejects(
    readFetchResponseBody({
      headers: new Headers({ "content-length": "4" }),
      async arrayBuffer() { throw new Error("reader must not run"); },
    }, 3, "fixture"),
    (error) => error.code === BROWSER_POOL_BODY_LIMIT_CODE,
  );
});

test("byte adapter rejects values without a measurable length", () => {
  assert.throws(() => assertBytesWithinLimit({}, 1), TypeError);
});
