import assert from "node:assert/strict";
import fs from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import vm from "node:vm";
import test from "node:test";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";

const element = (fields = {}, attributes = {}) => ({
  innerText: "", tagName: "INPUT", ...fields,
  getAttribute(name) { return attributes[name] ?? null; },
});

async function fixture(t, { bodyText = "", nodes = {}, historyState = null } = {}) {
  const directory = await fs.mkdtemp(path.join(os.tmpdir(), "gateway-program-handle-snapshot-"));
  t.after(() => fs.rm(directory, { recursive: true, force: true }));
  const app = await importTestableProgramHandle({ outDir: directory }), screenshots = [];
  const document = { title: "Fixture page", body: { innerText: bodyText }, querySelectorAll: (selector) => nodes[selector] ?? [] };
  const page = {
    async evaluate(callback, argument) {
      return structuredClone(vm.runInNewContext("(" + callback.toString() + ")(__argument)", {
        document, history: { state: historyState }, location: { href: "https://fixture.invalid/page" }, __argument: argument,
      }, { timeout: 1000 }));
    },
    async screenshot(options) { screenshots.push(options); },
  };
  return { app, page, directory, document, screenshots };
}

test("standalone snapshot bounds catalogs and extracts hints before text truncation", async (t) => {
  const f = await fixture(t, {
    bodyText: "x".repeat(12010) + " /app/abcdef123456",
    historyState: { note: "y".repeat(4010), late: "/app/987654abcdef" },
    nodes: {
      'button,[role="button"],a[role="button"]': [element(), ...Array.from({ length: 201 }, (_, i) => element({ innerText: " button " + i }))],
      "a[href]": Array.from({ length: 201 }, (_, i) => element({ href: "https://fixture.invalid/" + i })),
      img: Array.from({ length: 121 }, (_, i) => element({ currentSrc: "https://fixture.invalid/" + i + ".png", naturalWidth: 300, naturalHeight: 400 })),
      '[role="textbox"], textarea, [contenteditable="true"], input': Array.from({ length: 81 }, () => element()),
    },
  });
  const result = await f.app.collectSnapshot(f.page, "bounded");
  assert.equal(result.bodyText.length, 12000);
  assert.equal(result.historyState.length, 4000);
  assert.deepEqual(result.handleHints.appPaths, ["/app/987654abcdef", "/app/abcdef123456"]);
  assert.deepEqual([result.buttons.length, result.anchors.length, result.mediaNodes.length, result.textboxes.length], [200, 200, 120, 80]);
  assert.equal(result.buttons[0].index, 1);
  assert.equal(result.buttons.at(-1).index, 200);
  assert.equal(result.buttons[0].text, "button 0");
  assert.deepEqual(JSON.parse(await fs.readFile(path.join(f.directory, "bounded.json"), "utf8")), result);
  assert.deepEqual(f.screenshots, [{ path: path.join(f.directory, "bounded.png"), fullPage: true }]);
});

test("standalone snapshot preserves raw media anchor and textbox metadata", async (t) => {
  const f = await fixture(t, { nodes: {
    audio: [element({ currentSrc: "blob:audio", controls: false, duration: Infinity }, { src: "raw-audio" })],
    video: [element({ currentSrc: "blob:video", controls: true, duration: 12, videoWidth: 640, videoHeight: 360 }, { poster: "poster.png" })],
    img: [element({ currentSrc: "image.png", naturalWidth: 300, naturalHeight: 400 }, { alt: "image" })],
    "a[href]": [element({ href: "https://fixture.invalid/download", innerText: " Download " }, { download: "raw.wav", target: "_blank", "aria-label": "label" })],
    '[role="textbox"], textarea, [contenteditable="true"], input': [element({ tagName: "TEXTAREA" }, { role: "textbox", placeholder: "prompt", "aria-label": "input" })],
  } });
  const result = await f.app.collectSnapshot(f.page, "metadata");
  assert.deepEqual(result.mediaNodes, [
    { kind: "audio", index: 0, src: "raw-audio", currentSrc: "blob:audio", controls: false, duration: null },
    { kind: "video", index: 0, src: null, currentSrc: "blob:video", controls: true, duration: 12, width: 640, height: 360, poster: "poster.png" },
    { kind: "image", index: 0, src: null, currentSrc: "image.png", alt: "image", width: 300, height: 400 },
  ]);
  assert.deepEqual(result.anchors, [{ index: 0, text: "Download", href: "https://fixture.invalid/download", ariaLabel: "label", title: null, target: "_blank", download: "raw.wav" }]);
  assert.deepEqual(result.textboxes, [{ index: 0, tag: "TEXTAREA", role: "textbox", placeholder: "prompt", ariaLabel: "input" }]);
});

test("standalone snapshot tolerates circular history and absent body", async (t) => {
  const historyState = {}; historyState.self = historyState;
  const f = await fixture(t, { historyState });
  f.document.body = null;
  const result = await f.app.collectSnapshot(f.page, "empty");
  assert.equal(result.historyState, "");
  assert.equal(result.bodyText, "");
  assert.equal(result.label, "empty");
  assert.equal(result.title, "Fixture page");
  assert.deepEqual(result.handleHints, { appPaths: [], conversationIds: [], responseIds: [], sharePaths: [] });
});

test("standalone snapshot screenshot rejection propagates after JSON persistence", async (t) => {
  const f = await fixture(t), failure = new Error("screenshot unavailable");
  f.page.screenshot = async () => { assert.equal(JSON.parse(await fs.readFile(path.join(f.directory, "failed.json"), "utf8")).label, "failed"); throw failure; };
  await assert.rejects(f.app.collectSnapshot(f.page, "failed"), (error) => error === failure);
});

test("standalone snapshot evaluation failure writes no artifacts", async (t) => {
  const f = await fixture(t), failure = new Error("page detached");
  f.page.evaluate = async () => { throw failure; };
  await assert.rejects(f.app.collectSnapshot(f.page, "failed"), (error) => error === failure);
  assert.deepEqual(await fs.readdir(f.directory), []);
  assert.deepEqual(f.screenshots, []);
});

test("standalone snapshot JSON write failure prevents screenshot capture", async (t) => {
  const f = await fixture(t), app = await importTestableProgramHandle({ outDir: path.join(f.directory, "missing") });
  await assert.rejects(app.collectSnapshot(f.page, "failed"), { code: "ENOENT" });
  assert.deepEqual(f.screenshots, []);
});
