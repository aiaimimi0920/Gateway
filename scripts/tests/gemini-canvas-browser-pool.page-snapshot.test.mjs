import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const buttonSelector = 'button,[role="button"],a[role="button"]';
const textboxSelector = '[role="textbox"], textarea, input[type="text"], [contenteditable="true"]';
const node = (fields = {}, attributes = {}) => ({ innerText: "", ...fields, getAttribute: (name) => attributes[name] ?? null });

function snapshotPage({ nodes = {}, body = { innerText: "" }, url = "https://fixture.invalid/page", html = "", title = "Fixture" } = {}) {
  return {
    content: async () => html,
    async evaluate(callback, argument) {
      // Serialize into the browser realm so callbacks cannot capture host helpers.
      return structuredClone(vm.runInNewContext(`(${callback.toString()})(__argument)`, {
        __argument: argument, location: { href: url }, window: {},
        document: { body, title, scripts: [], documentElement: { outerHTML: html }, querySelectorAll: (selector) => nodes[selector] ?? [] },
      }, { timeout: 1000 }));
    },
  };
}

test("Shared button snapshot serializes an empty DOM without host bindings", async () => {
  assert.deepEqual(await app.collectButtonSnapshot(snapshotPage()), []);
});

test("Shared page snapshot serializes the complete empty schema", async () => {
  assert.deepEqual(await app.collectPageSnapshot(snapshotPage()), {
    buttons: [], mediaNodes: [], anchorNodes: [],
    pageState: { url: "https://fixture.invalid/page", title: "Fixture", bodyText: "" },
  });
});

for (const [name, cap, indexKey] of [["collectButtonSnapshot", 300, "index"], ["collectPageSnapshot", 200, "i"]]) {
  test(`Shared ${name} filters empty controls before its ${cap} item cap`, async () => {
    const buttons = [node(), node({}, { "aria-label": " label " }), node({}, { title: " title " }), ...Array.from({ length: 305 }, (_, i) => node({ innerText: ` button ${i} ` }))];
    const result = await app[name](snapshotPage({ nodes: { [buttonSelector]: buttons } }));
    const selected = name === "collectPageSnapshot" ? result.buttons : result;
    assert.equal(selected.length, cap);
    assert.deepEqual(selected[0], { [indexKey]: 1, text: "", ariaLabel: " label ", title: null });
    assert.deepEqual(selected[1], { [indexKey]: 2, text: "", ariaLabel: null, title: " title " });
    assert.equal(selected.at(-1)[indexKey], cap);
    assert.equal(selected.at(-1).text, `button ${cap - 3}`);
  });
}

test("Shared page snapshot retains kind-specific media metadata and source indices", async () => {
  const result = await app.collectPageSnapshot(snapshotPage({ nodes: {
    audio: [node(), node({ currentSrc: "blob:audio", controls: true, duration: 2.5 }, { src: "audio.wav" })],
    video: [node({ currentSrc: "blob:video", controls: false, duration: Infinity, videoWidth: 640, videoHeight: 360 }, { src: "video.mp4", poster: "poster.png" })],
    img: [node({ naturalWidth: 128, naturalHeight: 64 }, { src: "image.png", alt: "fixture" })],
  } }));
  assert.deepEqual(result.mediaNodes, [
    { kind: "audio", i: 1, src: "audio.wav", currentSrc: "blob:audio", controls: true, duration: 2.5 },
    { kind: "video", i: 0, src: "video.mp4", currentSrc: "blob:video", controls: false, width: 640, height: 360, duration: null, poster: "poster.png" },
    { kind: "image", i: 0, alt: "fixture", src: "image.png", width: 128, height: 64 },
  ]);
});

test("Shared page snapshot normalizes nonfinite durations while retaining zero", async () => {
  const audio = [NaN, Infinity, -Infinity, 0].map((duration) => node({ currentSrc: "blob:audio", duration }));
  const result = await app.collectPageSnapshot(snapshotPage({ nodes: { audio } }));
  assert.deepEqual(result.mediaNodes.map((entry) => entry.duration), [null, null, null, 0]);
});

test("Shared page snapshot retains current audio sources but excludes source-less images", async () => {
  const result = await app.collectPageSnapshot(snapshotPage({ nodes: {
    audio: [node({ currentSrc: "blob:audio" })],
    img: [node({ currentSrc: "image-only-current.png" }), node({}, { src: "image-with-src.png" })],
  } }));
  assert.deepEqual(result.mediaNodes.map(({ kind, src }) => ({ kind, src })), [
    { kind: "audio", src: null }, { kind: "image", src: "image-with-src.png" },
  ]);
});

test("Shared page snapshot preserves uncapped media and anchors with raw link metadata", async () => {
  const audio = Array.from({ length: 205 }, (_, i) => node({ currentSrc: `audio:${i}` }));
  const anchors = [node(), node({ textContent: " text only " }), ...Array.from({ length: 205 }, (_, i) => node({ href: `https://fixture.invalid/${i}`, textContent: ` link ${i} ` }, { href: `/${i}`, target: "_blank", download: "fixture.bin", "aria-label": "label", title: "title" }))];
  const result = await app.collectPageSnapshot(snapshotPage({ nodes: { audio, a: anchors } }));
  assert.equal(result.mediaNodes.length, 205);
  assert.equal(result.anchorNodes.length, 206);
  assert.deepEqual(result.anchorNodes[1], { i: 2, text: "link 0", href: "https://fixture.invalid/0", rawHref: "/0", ariaLabel: "label", title: "title", target: "_blank", download: "fixture.bin" });
  assert.equal(result.anchorNodes.at(-1).i, 206);
});

for (const [label, body, expected] of [["missing", null, ""], ["long", { innerText: "b".repeat(12500) }, "b".repeat(12000)]]) {
  test(`Shared page snapshot handles ${label} body text`, async () => {
    assert.equal((await app.collectPageSnapshot(snapshotPage({ body }))).pageState.bodyText, expected);
  });
}

for (const name of ["collectButtonSnapshot", "collectPageSnapshot"]) {
  test(`Shared ${name} propagates a detached-page failure unchanged`, async () => {
    const failure = new Error("fixture page detached");
    await assert.rejects(app[name]({ evaluate: async () => { throw failure; } }), (error) => error === failure);
  });
}

test("Debug real entry composes nonempty shared snapshots and diagnostics without navigation", async () => {
  const bodyText = "body ".repeat(2500);
  const page = snapshotPage({ url: "https://gemini.google.com/app/12345678", body: { innerText: bodyText }, html: "speech diagnostic", nodes: {
    [buttonSelector]: [node({ innerText: " Control " })],
    [textboxSelector]: [node({ tagName: "TEXTAREA", value: "v".repeat(350) }, { placeholder: "Prompt" })],
    img: [node({ naturalWidth: 16, naturalHeight: 8 }, { src: "fixture.png", alt: "fixture" })],
  } });
  const result = await app.runDebugOperation({ page }, { resetConversation: false, captureNetwork: "false" });
  assert.equal(result.operation, "debug");
  assert.equal(result.appPath, "/app/12345678");
  assert.equal(result.bodyText, bodyText);
  assert.deepEqual(result.buttons, [{ index: 0, text: "Control", ariaLabel: null, title: null }]);
  assert.deepEqual(result.media, [{ kind: "image", i: 0, alt: "fixture", src: "fixture.png", width: 16, height: 8 }]);
  assert.deepEqual(result.textboxes, [{ index: 0, tagName: "TEXTAREA", ariaLabel: null, placeholder: "Prompt", text: "v".repeat(300) }]);
  assert.deepEqual(result.snippets, { speech: "speech diagnostic\n" });
  assert.deepEqual([result.apiKeys, result.interestingKeys, result.networkEvents, result.rpcCaptures], [[], {}, [], []]);
});
