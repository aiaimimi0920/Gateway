import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const buttonSelector = 'button,[role="button"],a[role="button"]';
const textboxSelector = '[role="textbox"], textarea, [contenteditable="true"], input';
const node = (fields = {}, attributes = {}) => ({ innerText: "", tagName: "DIV", ...fields, getAttribute: (name) => attributes[name] ?? null });

async function collect({ nodes = {}, body = { innerText: "" }, historyState = null, url = "https://fixture.invalid/page", title = "Fixture" } = {}) {
  return await app.collectProgramHandleSnapshot({
    evaluate: async (callback) => {
      // Recreate Playwright's closure boundary rather than inheriting host imports.
      const snapshot = vm.runInNewContext(`(${callback.toString()})()`, {
        document: { body, title, querySelectorAll: (selector) => nodes[selector] ?? [] },
        history: { state: historyState }, location: { href: url },
      }, { timeout: 1000 });
      return structuredClone(snapshot);
    },
  });
}

test("program snapshot evaluates a self-contained browser closure with an empty DOM", async () => {
  assert.deepEqual(await collect(), {
    url: "https://fixture.invalid/page", title: "Fixture", bodyText: "", historyState: "null",
    buttons: [], anchors: [], mediaNodes: [], textboxes: [],
    handleHints: { appPaths: [], conversationIds: [], responseIds: [], sharePaths: [] },
  });
});

test("program snapshot filters empty buttons and anchors before applying their limits", async () => {
  const buttons = [node(), node({}, { "aria-label": " label " }), ...Array.from({ length: 205 }, (_, i) => node({ innerText: ` button ${i} ` }))];
  const anchors = [node(), ...Array.from({ length: 205 }, (_, i) => node({ innerText: ` link ${i} `, href: `https://fixture.invalid/${i}` }, { target: "_blank", download: "fixture.bin" }))];
  const snapshot = await collect({ nodes: { [buttonSelector]: buttons, "a[href]": anchors } });
  assert.equal(snapshot.buttons.length, 200);
  assert.deepEqual(snapshot.buttons[0], { index: 1, text: "", ariaLabel: " label ", title: null });
  assert.equal(snapshot.buttons.at(-1).text, "button 198");
  assert.equal(snapshot.anchors.length, 200);
  assert.deepEqual(snapshot.anchors[0], { index: 1, text: "link 0", href: "https://fixture.invalid/0", ariaLabel: null, title: null, target: "_blank", download: "fixture.bin" });
  assert.equal(snapshot.anchors.at(-1).index, 200);
});

test("program snapshot retains media kind metadata and normalizes nonfinite durations", async () => {
  const snapshot = await collect({ nodes: {
    audio: [node(), node({ currentSrc: "blob:audio", controls: true, duration: 2.5 }, { src: "track.wav" })],
    video: [node({ currentSrc: "blob:video", controls: false, duration: Infinity, videoWidth: 640, videoHeight: 360 }, { src: "clip.mp4", poster: "poster.png" })],
    img: [node({ currentSrc: "image.png", naturalWidth: 128, naturalHeight: 64 }, { alt: "fixture" })],
  } });
  assert.deepEqual(snapshot.mediaNodes, [
    { kind: "audio", index: 1, src: "track.wav", currentSrc: "blob:audio", controls: true, duration: 2.5 },
    { kind: "video", index: 0, src: "clip.mp4", currentSrc: "blob:video", controls: false, duration: null, width: 640, height: 360, poster: "poster.png" },
    { kind: "image", index: 0, src: null, currentSrc: "image.png", alt: "fixture", width: 128, height: 64 },
  ]);
});

test("program snapshot applies the shared media cap after filtering in kind order", async () => {
  const audio = [node(), ...Array.from({ length: 119 }, (_, i) => node({ currentSrc: `audio:${i}` }))];
  const snapshot = await collect({ nodes: { audio, video: [node({ currentSrc: "video:first" }), node({ currentSrc: "video:second" })], img: [node({ currentSrc: "image:last" })] } });
  assert.equal(snapshot.mediaNodes.length, 120);
  assert.equal(snapshot.mediaNodes[0].index, 1);
  assert.equal(snapshot.mediaNodes[118].currentSrc, "audio:118");
  assert.equal(snapshot.mediaNodes[119].currentSrc, "video:first");
});

test("program snapshot caps textboxes while retaining input descriptors", async () => {
  const textboxes = Array.from({ length: 83 }, () => node({ tagName: "TEXTAREA" }, { role: "textbox", placeholder: "Prompt", "aria-label": "Compose" }));
  const snapshot = await collect({ nodes: { [textboxSelector]: textboxes } });
  assert.equal(snapshot.textboxes.length, 80);
  assert.deepEqual(snapshot.textboxes.at(-1), { index: 79, tag: "TEXTAREA", role: "textbox", placeholder: "Prompt", ariaLabel: "Compose" });
});

test("program snapshot bounds body and serialized history text", async () => {
  const body = { innerText: "b".repeat(12500) }, historyState = { data: "h".repeat(4500) };
  const snapshot = await collect({ body, historyState });
  assert.equal(snapshot.bodyText, body.innerText.slice(0, 12000));
  assert.equal(snapshot.historyState, JSON.stringify(historyState).slice(0, 4000));
});

test("program snapshot tolerates missing body and unserializable history", async () => {
  const historyState = {}; historyState.self = historyState;
  const snapshot = await collect({ body: null, historyState });
  assert.equal(snapshot.bodyText, "");
  assert.equal(snapshot.historyState, null);
});

test("program snapshot derives host-side hints from URL history body and anchor fields", async () => {
  const snapshot = await collect({
    url: "https://gemini.google.com/app/11111111", title: "c_deadbeef",
    historyState: { id: "c_22222222" }, body: { innerText: "c_33333333 r_44444444" },
    nodes: { [buttonSelector]: [node({ innerText: "c_deadbeef" })], "a[href]": [node({ href: "https://gemini.google.com/app/55555555", innerText: "/share/abcdefgh" }, { "aria-label": "c_66666666", title: "r_77777777" })] },
  });
  assert.deepEqual(snapshot.handleHints, {
    appPaths: ["/app/11111111", "/app/55555555", "/app/22222222", "/app/33333333", "/app/66666666"],
    conversationIds: ["c_22222222", "c_33333333", "c_66666666"],
    responseIds: ["r_44444444", "r_77777777"], sharePaths: ["/share/abcdefgh"],
  });
});

test("program snapshot propagates page evaluation failure without masking it", async () => {
  const failure = new Error("fixture page detached");
  await assert.rejects(app.collectProgramHandleSnapshot({ evaluate: async () => { throw failure; } }), (error) => error === failure);
});
