import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { fetchPage, navigationFixture, navigationResponse, downloadFile } from "./gemini-canvas-browser-pool.payload-fixtures.mjs";
import { BROWSER_POOL_BINARY_BODY_LIMIT_BYTES } from "../gemini-canvas-browser-pool-body.mjs";

const app = await importTestableScript();
const rawUrl = "http://lh3.googleusercontent.com/payload";
const normalizedUrl = "https://lh3.googleusercontent.com/payload";
const navigationUrl = "https://fixture.invalid/download.mp3";

for (const [kind, extract, defaultMime, fetchedMime, errorCode, wrongTypeCode] of [
  ["audio", app.extractAudioBytes, "audio/wav", "audio/ogg", "gemini_canvas_tts_audio_fetch_failed", "gemini_canvas_tts_non_audio_asset"],
  ["image", app.extractImageBytes, "image/png", "image/webp", "gemini_canvas_image_fetch_failed", "gemini_canvas_non_image_asset"],
]) {
  test(`payload ${kind} captured bytes bypass the page and preserve supplied or default MIME`, async () => {
    assert.deepEqual(await extract(null, { bodyBase64: "captured" }), { mimeType: defaultMime, bodyBase64: "captured" });
    assert.deepEqual(await extract(null, { bodyBase64: "captured", mimeType: "fixture/custom" }), { mimeType: "fixture/custom", bodyBase64: "captured" });
  });

  test(`payload ${kind} browser callback normalizes URL includes credentials and encodes multiple chunks`, async () => {
    const body = Buffer.from(Array.from({ length: 0x10003 }, (_, index) => index % 256));
    const { page, calls } = fetchPage({ body, contentType: fetchedMime });
    assert.deepEqual(await extract(page, { url: rawUrl, mimeType: defaultMime }), { mimeType: fetchedMime, bodyBase64: body.toString("base64") });
    assert.deepEqual(calls, [{ url: normalizedUrl, options: { credentials: "include" } }]);
  });

  test(`payload ${kind} missing response type falls back to asset then default MIME`, async () => {
    for (const mimeType of [undefined, fetchedMime]) {
      const { page } = fetchPage();
      assert.equal((await extract(page, { url: rawUrl, mimeType })).mimeType, mimeType || defaultMime);
    }
  });

  test(`payload ${kind} fetch and body-read exceptions retain error code status URL and cause`, async () => {
    const failure = Object.assign(new Error("fixture transport failed"), { status: 503 });
    for (const options of [{ fetchFailure: failure }, { bodyFailure: failure }]) {
      const { page } = fetchPage(options);
      await assert.rejects(extract(page, { url: rawUrl }), (error) => {
        assert.deepEqual([error.code, error.status, error.url, error.cause], [errorCode, 503, normalizedUrl, failure.message]);
        return true;
      });
    }
  });

  test(`payload ${kind} unsuccessful status and empty successful bodies are rejected`, async () => {
    for (const options of [{ status: 403 }, { status: 204, body: Buffer.alloc(0) }]) {
      const { page } = fetchPage({ ...options, contentType: fetchedMime });
      await assert.rejects(extract(page, { url: rawUrl }), (error) => {
        assert.deepEqual([error.code, error.status, error.url], [errorCode, options.status, normalizedUrl]);
        assert.equal(Object.hasOwn(error, "cause"), false);
        return true;
      });
    }
  });

  test(`payload ${kind} rejects incompatible MIME with its distinct resource error`, async () => {
    const { page } = fetchPage({ contentType: "text/html" });
    await assert.rejects(extract(page, { url: rawUrl, mimeType: defaultMime }), (error) => {
      assert.deepEqual([error.code, error.status, error.mimeType, error.url], [wrongTypeCode, 200, "text/html", normalizedUrl]);
      return true;
    });
  });

  test(`payload ${kind} rejects a declared oversized body before reading it`, async () => {
    const { page } = fetchPage({
      contentType: fetchedMime,
      contentLength: String(BROWSER_POOL_BINARY_BODY_LIMIT_BYTES + 1),
      bodyFailure: new Error("body reader must not run"),
    });
    await assert.rejects(extract(page, { url: rawUrl }), (error) => {
      assert.equal(error.status, 413);
      assert.match(error.cause, /body exceeded/);
      assert.doesNotMatch(error.cause, /body reader must not run/);
      return true;
    });
  });

  test(`payload ${kind} refuses unknown-size fallback before arrayBuffer`, async () => {
    const { page, bodyReads } = fetchPage({ contentType: fetchedMime, contentLength: null });
    await assert.rejects(extract(page, { url: rawUrl }), (error) => {
      assert.equal(error.status, 502);
      assert.match(error.cause, /no declared size/);
      return true;
    });
    assert.equal(bodyReads(), 0);
  });

  test(`payload ${kind} ignores representation length for a bodyless response`, async () => {
    const { page, bodyReads } = fetchPage({
      status: 304,
      contentType: fetchedMime,
      contentLength: String(BROWSER_POOL_BINARY_BODY_LIMIT_BYTES + 1),
    });
    await assert.rejects(extract(page, { url: rawUrl }), (error) => {
      assert.equal(error.code, errorCode);
      assert.equal(error.status, 304);
      return true;
    });
    assert.equal(bodyReads(), 0);
  });
}

test("payload non-Error evaluation rejection is converted to a string cause", async () => {
  const page = { async evaluate() { throw "fixture rejection"; } };
  await assert.rejects(app.extractAudioBytes(page, {}), (error) => {
    assert.deepEqual([error.code, error.status, error.url, error.cause], ["gemini_canvas_tts_audio_fetch_failed", 500, null, "fixture rejection"]);
    return true;
  });
});

test("payload audio-looking URL retains acceptance when response MIME is generic", async () => {
  const { page } = fetchPage({ contentType: "application/octet-stream" });
  assert.equal((await app.extractAudioBytes(page, { url: "https://fixture.invalid/music.wav" })).mimeType, "application/octet-stream");
});

test("payload navigation prioritizes download over navigation error and closes its temporary page", async (t) => {
  const body = Buffer.from([0, 255, 128, 7]), file = await downloadFile(t, body);
  const download = { path: async () => file, suggestedFilename: () => 'a"udio.mp3' };
  const { entry, calls } = navigationFixture({ download, navigationError: new Error("download aborted navigation") });
  assert.deepEqual(await app.downloadBinaryViaNavigation(entry, navigationUrl, 90000), {
    status: 200, ok: true, finalUrl: navigationUrl, contentType: "audio/mpeg",
    headers: { "content-disposition": 'attachment; filename="audio.mp3"' }, bodyText: null, bodyBase64: body.toString("base64"),
  });
  assert.deepEqual(calls, [["newPage"], ["event", "download", { timeout: 30000 }], ["goto", navigationUrl, { waitUntil: "commit", timeout: 90000 }], ["close"]]);
});

test("payload navigation unnamed download infers MIME from URL and uses bare attachment", async (t) => {
  const file = await downloadFile(t, Buffer.from("unnamed"));
  const { entry, calls } = navigationFixture({ download: { path: async () => file, suggestedFilename: () => "" } });
  const result = await app.downloadBinaryViaNavigation(entry, navigationUrl, 25);
  assert.equal(result.contentType, "audio/mpeg");
  assert.deepEqual(result.headers, { "content-disposition": "attachment" });
  assert.deepEqual(calls[1], ["event", "download", { timeout: 25 }]);
  assert.deepEqual(calls.at(-1), ["close"]);
});

for (const stage of ["download-path", "download-read"]) {
  test(`payload navigation ${stage} failure still closes its page`, async (t) => {
    const file = await downloadFile(t, Buffer.from("fixture"));
    const failure = new Error("fixture path failed");
    const download = {
      async path() { if (stage === "download-path") throw failure; return file + ".missing"; },
      suggestedFilename: () => "fixture.bin",
    };
    const { entry, calls } = navigationFixture({ download });
    await assert.rejects(app.downloadBinaryViaNavigation(entry, navigationUrl, 25), (error) => stage === "download-path" ? error === failure : error.code === "ENOENT");
    assert.deepEqual(calls.at(-1), ["close"]);
  });
}

test("payload navigation returns response status headers final URL and textual body", async () => {
  const body = Buffer.from('{"fixture":true}');
  const { entry, calls } = navigationFixture({ response: navigationResponse({ body, contentType: "application/json", status: 404 }) });
  assert.deepEqual(await app.downloadBinaryViaNavigation(entry, navigationUrl, 25), {
    status: 404, ok: false, finalUrl: "https://fixture.invalid/final", contentType: "application/json",
    headers: { "content-type": "application/json", "content-length": String(body.byteLength), "x-fixture": "retained" }, bodyText: body.toString("utf8"), bodyBase64: body.toString("base64"),
  });
  assert.deepEqual(calls.at(-1), ["close"]);
});

test("payload navigation binary or absent content type leaves bodyText null", async () => {
  for (const contentType of ["application/octet-stream", null]) {
    const { entry, calls } = navigationFixture({ response: navigationResponse({ contentType }) });
    const result = await app.downloadBinaryViaNavigation(entry, navigationUrl, 25);
    assert.equal(result.contentType, contentType);
    assert.equal(result.bodyText, null);
    assert.equal(result.bodyBase64, Buffer.from("fixture").toString("base64"));
    assert.deepEqual(calls.at(-1), ["close"]);
  }
});

test("payload navigation error is rethrown unchanged when download is absent", async () => {
  const failure = new Error("fixture navigation failed"), { entry, calls } = navigationFixture({ navigationError: failure });
  await assert.rejects(app.downloadBinaryViaNavigation(entry, navigationUrl, 25), (error) => error === failure);
  assert.deepEqual(calls.at(-1), ["close"]);
});

test("payload navigation absent response retains its 599 error and closes the page", async () => {
  const { entry, calls } = navigationFixture({ response: null });
  await assert.rejects(app.downloadBinaryViaNavigation(entry, navigationUrl, 25), (error) => {
    assert.deepEqual([error.status, error.code], [599, "gemini_canvas_navigation_fetch_missing_response"]);
    return true;
  });
  assert.deepEqual(calls.at(-1), ["close"]);
});

test("payload navigation response body failure closes the temporary page", async () => {
  const failure = new Error("fixture body detached");
  const { entry, calls } = navigationFixture({ response: navigationResponse({ bodyFailure: failure }) });
  await assert.rejects(app.downloadBinaryViaNavigation(entry, navigationUrl, 25), (error) => error === failure);
  assert.deepEqual(calls.at(-1), ["close"]);
});

test("payload navigation close failure cannot replace success or the primary failure", async () => {
  const closeError = new Error("fixture already closed"), failure = new Error("primary navigation failure");
  const success = navigationFixture({ closeError });
  assert.equal((await app.downloadBinaryViaNavigation(success.entry, navigationUrl, 25)).ok, true);
  const failed = navigationFixture({ closeError, navigationError: failure });
  await assert.rejects(app.downloadBinaryViaNavigation(failed.entry, navigationUrl, 25), (error) => error === failure);
  assert.deepEqual(success.calls.at(-1), ["close"]);
  assert.deepEqual(failed.calls.at(-1), ["close"]);
});

test("payload navigation page allocation failure starts no navigation or cleanup", async () => {
  const failure = new Error("fixture context closed"), { entry, calls } = navigationFixture({ newPageError: failure });
  await assert.rejects(app.downloadBinaryViaNavigation(entry, navigationUrl, 25), (error) => error === failure);
  assert.deepEqual(calls, [["newPage"]]);
});
