import assert from "node:assert/strict";
import { access } from "node:fs/promises";
import test from "node:test";
import { appFixture, browserTest } from "./gemini-canvas-browser-pool.app-fixtures.mjs";
import { downloadWebMusic } from "../gemini-canvas-browser-pool-web-music-download.mjs";

// Synthetic ID3 bytes prove download ownership/container admission, not audio decoding.
const bytes = Buffer.from("ID3\x04\0\0\0\0\0\0offline-fixture");
const html = `<button aria-label="下载音乐作品" onclick="document.querySelector('#audio').hidden=false">下载</button>
<button id="audio" hidden onclick="const a=document.createElement('a');a.href='data:audio/mpeg;base64,${bytes.toString("base64")}';a.download='fixture.mp3';a.click()">纯音频</button>`;

test("offline Edge downloads MP3 bytes and deletes only its owned temporary file", browserTest, async t => {
  const { page } = await appFixture(t, { html });
  let download;
  let path;
  const errors = [];
  page.on("pageerror", error => errors.push(error.message));
  const inherited = async value => { download = value; path = await value.path(); };
  page.on("download", inherited);
  const result = await downloadWebMusic(page, 10_000).catch(async error => {
    t.diagnostic(JSON.stringify({ errors, html: await page.content() }));
    throw error;
  });
  assert.deepEqual(Buffer.from(result.bodyBase64, "base64"), bytes);
  assert.equal(result.mimeType, "audio/mpeg");
  assert.ok(download);
  assert.ok(path);
  await assert.rejects(access(path), { code: "ENOENT" });
  assert.deepEqual(page.listeners("download"), [inherited]);
  page.off("download", inherited);
});
