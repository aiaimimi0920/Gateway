import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import fs from "node:fs";
import test from "node:test";
import { loadOwners, temporaryRoot, browserEvaluate } from "./gemini-canvas-image-edit-broad.fixtures.mjs";

const node = (attrs = {}, props = {}) => ({ getAttribute: (name) => attrs[name] ?? null, ...props });
const page = (globals) => ({ evaluate: (callback) => browserEvaluate(callback, globals) });

test("broad prepared owners import natively without launching browser setup", async (t) => {
  const capture = await import("../gemini-canvas-image-edit-broad-capture.mjs");
  const pageOwner = await import("../gemini-canvas-image-edit-broad-page.mjs");
  assert.equal(capture.createImageEditBroadCapture({ marker: "fixture", outDir: temporaryRoot(t) }).events.length, 0);
  assert.equal(typeof pageOwner.collectImageEditBroadPageState, "function");
  assert.deepEqual(await pageOwner.exportImageEditBroadPageBlobs({ evaluate: async () => [] }, temporaryRoot(t)), []);
});

test("broad page snapshot preserves DOM filters indices bounds and result schema", async () => {
  const elements = {
    img: [node({ src: "data:image/gif;base64,AA" }), node({ src: "blob:one", alt: "AI 生成" }, { naturalWidth: 512, naturalHeight: 256 }), node()],
    a: [node(), node({ href: "https://fixture.invalid/a.png" }, { textContent: "  image  " })],
    "*": [node({}, { background: "none" }), node({}, { background: "url(blob:one)", tagName: "DIV", className: "c".repeat(220), clientWidth: 300, clientHeight: 4, textContent: "t".repeat(220) })],
    canvas: [node({}, { width: 255, height: 255 }), node({}, { width: 256, height: 1, clientWidth: 128, clientHeight: 1 })],
    button: [node(), node({ "aria-label": "Download" }, { textContent: "  save  " })],
  };
  const result = structuredClone(await loadOwners().collectImageEditBroadPageState(page({
    document: { querySelectorAll: (selector) => elements[selector], title: "fixture title", body: { innerText: "b".repeat(12010) } },
    location: { href: "https://gemini.google.com/app/fixture" }, getComputedStyle: (value) => ({ backgroundImage: value.background }),
  })));
  assert.deepEqual(Object.keys(result), ["imageNodes", "anchorNodes", "cssBgNodes", "canvasNodes", "buttonNodes", "pageState"]);
  assert.deepEqual(result.imageNodes, [{ i: 1, src: "blob:one", alt: "AI 生成", width: 512, height: 256 }]);
  assert.deepEqual(result.anchorNodes, [{ i: 1, text: "image", href: "https://fixture.invalid/a.png" }]);
  assert.equal(result.cssBgNodes[0].className.length, 200);
  assert.equal(result.cssBgNodes[0].text.length, 200);
  assert.equal(result.canvasNodes[0].i, 1);
  assert.deepEqual(result.buttonNodes, [{ i: 1, text: "save", aria: "Download" }]);
  assert.equal(result.pageState.bodyText.length, 12000);
  assert.equal(result.pageState.title, "fixture title");
});

test("broad page snapshot keeps empty collections and propagates evaluation failure", async () => {
  const app = loadOwners(), empty = structuredClone(await app.collectImageEditBroadPageState(page({ document: { querySelectorAll: () => [], title: "", body: { innerText: "" } }, location: { href: "about:blank" } })));
  assert.deepEqual(empty, { imageNodes: [], anchorNodes: [], cssBgNodes: [], canvasNodes: [], buttonNodes: [], pageState: { url: "about:blank", title: "", bodyText: "" } });
  const failure = new Error("fixture evaluate failure");
  await assert.rejects(app.collectImageEditBroadPageState({ evaluate: async () => { throw failure; } }), (error) => error === failure);
});

test("broad page blob export executes isolated callback and preserves multichunk bytes", async (t) => {
  const outDir = temporaryRoot(t), bytes = Buffer.alloc(70001);
  for (let i = 0; i < bytes.length; i += 1) bytes[i] = i % 256;
  const requested = [];
  const captures = await loadOwners().exportImageEditBroadPageBlobs(page({
    document: { querySelectorAll: () => [node({ src: "https://fixture.invalid/ignored.png" }), node({ src: "blob:one", alt: "AI 生成" })] },
    fetch: async (url) => { requested.push(url); return { headers: { get: () => "image/png" }, arrayBuffer: async () => bytes.buffer.slice(bytes.byteOffset, bytes.byteOffset + bytes.byteLength) }; },
    btoa: (value) => Buffer.from(value, "latin1").toString("base64"), Uint8Array,
  }), outDir);
  assert.deepEqual(requested, ["blob:one"]);
  assert.equal(captures[0].fileName, "page-blob-ai-generated-1.png");
  assert.equal(captures[0].byteLength, bytes.length);
  assert.equal(captures[0].sha256, createHash("sha256").update(bytes).digest("hex"));
  assert.deepEqual(fs.readFileSync(captures[0].outputPath), bytes);
  assert.equal("dataBase64" in captures[0], false);
});

test("broad page blob export retains empty-byte and per-blob fetch failures", async (t) => {
  const captures = structuredClone(await loadOwners().exportImageEditBroadPageBlobs(page({
    document: { querySelectorAll: () => [node({ src: "blob:empty" }), node({ src: "blob:error", alt: "bad" })] },
    fetch: async (url) => { if (url === "blob:error") throw new Error("fixture fetch failure"); return { headers: { get: () => null }, arrayBuffer: async () => new ArrayBuffer(0) }; },
    btoa: (value) => Buffer.from(value, "latin1").toString("base64"), Uint8Array,
  }), temporaryRoot(t)));
  assert.deepEqual(captures, [{ i: 0, alt: "", src: "blob:empty", error: "missing blob data" }, { i: 1, alt: "bad", src: "blob:error", error: "Error: fixture fetch failure" }]);
});

test("broad page blob export preserves MIME priority label bounds and missing metadata", async (t) => {
  const input = [
    ["IMAGE/JPEG", "所上传图片的预览图", "uploaded-preview", "jpg"], ["image/png", "X".repeat(60), "x".repeat(48), "png"],
    ["image/webp", "a b", "a-b", "webp"], ["image/gif", "!", "blob", "gif"], ["application/binary", "", "blob", "bin"],
  ].map(([mimeType, alt, label, ext], i) => ({ mimeType, alt, label, ext, i, src: "blob:" + i, byteLength: 1, dataBase64: "YQ==" }));
  const result = await loadOwners().exportImageEditBroadPageBlobs({ evaluate: async () => [...input, null] }, temporaryRoot(t));
  for (const item of input) assert.equal(result[item.i].fileName, "page-blob-" + item.label + "-" + item.i + "." + item.ext);
  assert.deepEqual(structuredClone(result.at(-1)), { i: null, alt: "", src: "", error: "missing blob data" });
});

test("broad page blob export propagates artifact write failure", async (t) => {
  const failure = new Error("fixture write failure");
  const app = loadOwners({ writeFileSync() { throw failure; } });
  await assert.rejects(app.exportImageEditBroadPageBlobs({ evaluate: async () => [{ i: 0, dataBase64: "YQ==", mimeType: "image/png" }] }, temporaryRoot(t)), (error) => error === failure);
});
