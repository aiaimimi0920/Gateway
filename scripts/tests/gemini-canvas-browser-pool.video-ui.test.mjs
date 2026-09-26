import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { domPage, ElementFixture, locator } from "./gemini-canvas-browser-pool.operation-ui-fixtures.mjs";

const app = await importTestableScript();

function videoPage({ images = [], controls = [], actionVisible = false, clickFailure = false, attributeFailure = false, attributes = {}, sendFailure = false } = {}) {
  const result = domPage({ images, controls });
  const calls = [];
  const action = locator({
    last() { return this; },
    async waitFor(options) {
      calls.push(["action-wait", options]);
      if (!actionVisible) throw new Error("fixture action missing");
    },
    async getAttribute(name) {
      if (attributeFailure) throw new Error("fixture attribute detached");
      return attributes[name] ?? null;
    },
    async click(options) {
      calls.push(["action-click", options]);
      if (clickFailure) throw new Error("fixture action click failed");
    },
  });
  const send = locator({
    last() { return this; },
    async waitFor(options) { calls.push(["send-wait", options]); },
    async click(options) {
      calls.push(["send-click", options]);
      if (sendFailure) throw new Error("fixture send failed");
    },
  });
  result.page.locator = (selector) => selector.startsWith("button[aria-label") || selector === "button:has(svg), button:has(i)" ? send : action;
  result.page.getByRole = () => send;
  return { ...result, calls };
}

test("video UI template detector accepts localized chooser cues and rejects unrelated text", () => {
  for (const text of ["挑选一个模板", "开始制作你的视频", "CHOOSE A TEMPLATE", "Start creating your video"]) {
    assert.equal(app.bodyTextSuggestsVideoTemplateSelection(text), true, text);
  }
  for (const text of [null, "", "video is ready", "Please send a prompt"]) {
    assert.equal(app.bodyTextSuggestsVideoTemplateSelection(text), false);
  }
});

test("video UI create detection uses its short locator visibility cap", async () => {
  const { page, calls } = videoPage({ actionVisible: true });
  assert.equal(await app.hasVideoCreateAction(page, 90000), true);
  assert.deepEqual(calls, [["action-wait", { state: "visible", timeout: 800 }]]);
});

test("video UI create detection serializes DOM fallback and ignores hidden or unrelated controls", async () => {
  const controls = [new ElementFixture({ textContent: "Create video", hidden: true }), new ElementFixture({ textContent: "Help" })];
  const { page } = videoPage({ controls });
  assert.equal(await app.hasVideoCreateAction(page, 25), false);
  controls.push(new ElementFixture({ attributes: { "aria-label": "Create video" } }));
  assert.equal(await app.hasVideoCreateAction(page, 25), true);
});

test("video UI create detection returns false when both locator and DOM access fail", async () => {
  const { page } = videoPage();
  page.evaluate = async () => { throw new Error("fixture page closed"); };
  assert.equal(await app.hasVideoCreateAction(page, 25), false);
});

test("video UI create click preserves timeout caps and force behavior", async () => {
  const { page, calls } = videoPage({ actionVisible: true });
  assert.equal(await app.tryClickVideoCreateAction(page, 90000), true);
  assert.deepEqual(calls, [["action-wait", { state: "visible", timeout: 1500 }], ["action-click", { timeout: 5000, force: true }]]);
});

test("video UI create click tolerates detached optional metadata", async () => {
  const { page, calls } = videoPage({ actionVisible: true, attributeFailure: true });
  assert.equal(await app.tryClickVideoCreateAction(page, 25), true);
  assert.deepEqual(calls.at(-1), ["action-click", { timeout: 25, force: true }]);
});

test("video UI create click rejects deselect controls in locator and serialized DOM", async () => {
  const control = new ElementFixture({ textContent: "Create video", clickable: true, attributes: { title: "Deselect" } });
  const { page, calls } = videoPage({ controls: [control], actionVisible: true, attributes: { "aria-label": "Deselect Create video" } });
  assert.equal(await app.tryClickVideoCreateAction(page, 25), false);
  assert.equal(calls.some(([kind]) => kind === "action-click"), false);
  assert.equal(control.nativeClicks, 0);
});

test("video UI failed locator click dispatches ordered events on the clickable ancestor", async () => {
  const parent = new ElementFixture({ tagName: "BUTTON", clickable: true });
  const child = new ElementFixture({ textContent: "Create video", parentElement: parent });
  const { page } = videoPage({ controls: [child], actionVisible: true, clickFailure: true });
  assert.equal(await app.tryClickVideoCreateAction(page, 25), true);
  assert.deepEqual(parent.events, ["pointerdown", "mousedown", "mouseup", "click", "click"]);
  assert.equal(parent.nativeClicks, 1);
  assert.equal(child.nativeClicks, 0);
});

test("video UI missing template still settles and uses send when create controls are absent", async () => {
  const { page, waits, calls } = videoPage();
  assert.deepEqual(await app.trySelectVideoTemplateCard(page, 25), {
    clicked: false, reason: "template_candidate_missing", templateCount: 0,
    videoCreateVisible: false, videoCreateClicked: false, sendClicked: true,
  });
  assert.deepEqual(waits, [1500]);
  assert.deepEqual(calls.slice(-2), [["send-wait", { state: "visible", timeout: 25 }], ["send-click", { timeout: 25, force: true }]]);
});

for (const [attempt, selectedIndex, selectedSource] of [[-4, 0, "image"], [99, 1, "text"]]) {
  test(`video UI template filters candidates and clamps attempt ${attempt}`, async () => {
    const image = new ElementFixture({ alt: "video generation template", currentSrc: "template.png", clickable: true });
    const text = new ElementFixture({ tagName: "BUTTON", innerText: "Cosmos", clickable: true });
    const { page } = videoPage({
      images: [new ElementFixture({ alt: "unrelated" }), new ElementFixture({ alt: "video generation template", hidden: true }), image],
      controls: [new ElementFixture({ innerText: "Cosmos extra" }), new ElementFixture({ innerText: "Cosmos", hidden: true }), text],
    });
    const result = await app.trySelectVideoTemplateCard(page, 25, attempt);
    assert.equal(result.clicked, true);
    assert.equal(result.templateCount, 2);
    assert.equal(result.selectedIndex, selectedIndex);
    assert.equal(result.selectedSource, selectedSource);
    assert.equal(result.selectedLabel, selectedIndex ? "Cosmos" : "video generation template");
    assert.equal(image.nativeClicks, selectedIndex ? 0 : 1);
    assert.equal(text.nativeClicks, selectedIndex ? 1 : 0);
  });
}

test("video UI template clicks ancestor and preserves bounded selection metadata", async () => {
  const parent = new ElementFixture({ tagName: "BUTTON", clickable: true, innerText: "x".repeat(220), attributes: { role: "button", "aria-label": "Select template" } });
  const image = new ElementFixture({ alt: "video generation template", src: "fallback.png", parentElement: parent });
  const { page } = videoPage({ images: [image] });
  const result = await app.trySelectVideoTemplateCard(page, 25);
  assert.deepEqual([result.selectedSrc, result.clickedTag, result.clickedRole, result.clickedAria, result.clickedText.length], ["fallback.png", "BUTTON", "button", "Select template", 200]);
  assert.equal(parent.nativeClicks, 1);
  assert.equal(image.nativeClicks, 0);
  assert.deepEqual(parent.events, ["click", "pointerdown", "mousedown", "mouseup", "click"]);
});

test("video UI visible create action suppresses send even when clicking fails", async () => {
  const { page, calls } = videoPage({ actionVisible: true, clickFailure: true });
  const result = await app.trySelectVideoTemplateCard(page, 25);
  assert.equal(result.videoCreateVisible, true);
  assert.equal(result.videoCreateClicked, false);
  assert.equal(result.sendClicked, false);
  assert.equal(calls.some(([kind]) => kind === "send-click"), false);
});

test("video UI successful create action suppresses send fallback", async () => {
  const { page, calls } = videoPage({ actionVisible: true });
  const result = await app.trySelectVideoTemplateCard(page, 90000);
  assert.equal(result.videoCreateVisible, true);
  assert.equal(result.videoCreateClicked, true);
  assert.equal(result.sendClicked, false);
  assert.equal(calls.some(([kind]) => kind === "send-click"), false);
});

test("video UI exhausted send fallback reports false", async () => {
  const { page, calls } = videoPage({ sendFailure: true });
  assert.equal((await app.trySelectVideoTemplateCard(page, 25)).sendClicked, false);
  assert.equal(calls.filter(([kind]) => kind === "send-click").length, 3);
});

test("video UI template evaluation failure propagates before settling or follow-up actions", async () => {
  const { page, waits, calls } = videoPage();
  const failure = new Error("fixture page detached");
  page.evaluate = async () => { throw failure; };
  await assert.rejects(app.trySelectVideoTemplateCard(page, 25), (error) => error === failure);
  assert.deepEqual(waits, []);
  assert.deepEqual(calls, []);
});
