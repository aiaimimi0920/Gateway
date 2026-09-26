import assert from "node:assert/strict";
import test from "node:test";
import { importTestableProgramHandle } from "./gemini-canvas-program-handle.fixtures.mjs";
import { locator, missingLocator, ElementFixture, domPage } from "./gemini-canvas-browser-pool.operation-ui-fixtures.mjs";

import { composerFixture } from "./gemini-canvas-browser-pool.composer-fixtures.mjs";

const app = await importTestableProgramHandle();

for (const [operation, action, label] of [["music", "download", "Download music"], ["music", "play", "Play"], ["video", "download", "Download video"], ["video", "play", "Play video"]]) {
  test(`standalone interaction ${operation} ${action} falls back past missing role controls`, async () => {
    const calls = [], fallback = locator({
      async waitFor(options) { calls.push(["wait", options]); },
      async click(options) { calls.push(["click", options]); },
    });
    const page = {
      getByRole(role, { name }) { assert.equal(role, "button"); assert.equal(name.test(label), true); return missingLocator(); },
      locator() { return fallback; }, async waitForTimeout(ms) { calls.push(["settle", ms]); },
    };
    assert.equal(await app.clickMediaActionButton(page, operation, action, 90000), true);
    assert.deepEqual(calls, [["wait", { state: "visible", timeout: 5000 }], ["click", { timeout: 10000, force: true }], ["settle", 1500]]);
  });
}

test("standalone interaction unsupported actions and text mode return before accessing the page", async () => {
  const page = new Proxy({}, { get() { throw new Error("unexpected page access"); } });
  assert.equal(await app.clickMediaActionButton(page, "image", "download", 1000), false);
  assert.equal(await app.clickMediaActionButton(page, "video", "unknown", 1000), false);
  assert.equal(await app.clickOperationMode(page, "text", 1000), false);
});

test("standalone interaction action click retries failed candidates with shorter caller timeouts", async () => {
  const first = locator({ async click() { throw new Error("fixture detached"); } });
  let clicks = 0;
  const second = locator({ async waitFor({ timeout }) { assert.equal(timeout, 25); }, async click({ timeout }) { assert.equal(timeout, 25); clicks += 1; } });
  const page = { getByRole: () => first, locator: () => second, async waitForTimeout() {} };
  assert.equal(await app.clickMediaActionButton(page, "music", "download", 25), true);
  assert.equal(clicks, 1);
});

test("standalone interaction music style selection reports missing candidates and settles", async () => {
  const { page, waits } = domPage();
  assert.deepEqual(await app.trySelectMusicStyleCard(page, 1000), { clicked: false, reason: "style_candidate_missing", styleCount: 0 });
  assert.deepEqual(waits, [1500]);
});

for (const [attempt, expected] of [[-2, 0], [99, 1]]) {
  test(`standalone interaction music style selection filters candidates and clamps attempt ${attempt}`, async () => {
    const images = [new ElementFixture(), new ElementFixture({ alt: "Profile photo" }), new ElementFixture({ alt: "small", naturalWidth: 299 }), new ElementFixture({ alt: "hidden", hidden: true }), new ElementFixture({ alt: "first", currentSrc: "first.png" }), new ElementFixture({ alt: "second", currentSrc: "second.png" })];
    const { page } = domPage({ images });
    const result = await app.trySelectMusicStyleCard(page, 1000, attempt);
    assert.equal(result.clicked, true);
    assert.equal(result.styleCount, 2);
    assert.equal(result.selectedIndex, expected);
    assert.equal(result.selectedAlt, expected ? "second" : "first");
    assert.equal(images[4 + expected].nativeClicks, 1);
    assert.equal(images[5 - expected].nativeClicks, 0);
  });
}

test("standalone interaction music style selection clicks the visible ancestor and preserves metadata", async () => {
  const parent = new ElementFixture({ tagName: "BUTTON", clickable: true, innerText: "x".repeat(220), attributes: { role: "button", "aria-label": "Select style" } });
  const image = new ElementFixture({ alt: "style", src: "fallback.png", parentElement: parent });
  const { page } = domPage({ images: [image] });
  const result = await app.trySelectMusicStyleCard(page, 1000);
  assert.equal(parent.nativeClicks, 1);
  assert.equal(image.nativeClicks, 0);
  assert.equal(result.selectedSrc, "fallback.png");
  assert.equal(result.clickedTag, "BUTTON");
  assert.equal(result.clickedRole, "button");
  assert.equal(result.clickedAria, "Select style");
  assert.equal(result.clickedText.length, 200);
});

test("standalone interaction music style evaluation failures propagate without settling", async () => {
  const failure = new Error("fixture page closed");
  const page = { async evaluate() { throw failure; }, async waitForTimeout() { throw new Error("unexpected settle"); } };
  await assert.rejects(app.trySelectMusicStyleCard(page, 1000), (error) => error === failure);
});


test("standalone interaction share polling retains localized cues and deadline cadence", async (t) => {
  let now = 0, probes = 0;
  t.mock.method(Date, "now", () => now);
  const { page, document, waits } = domPage({ bodyText: "Continue Try Gemini Canvas" });
  const evaluate = page.evaluate;
  page.evaluate = async (...args) => { probes += 1; return evaluate(...args); };
  page.waitForTimeout = async (ms) => { waits.push(ms); now += ms; document.body.innerText = "继续"; };
  assert.equal(await app.waitForShareSurface(page, 2400), true);
  assert.equal(probes, 2);
  assert.deepEqual(waits, [1200]);
  assert.equal(await app.waitForShareSurface(null, 0), false);
});

test("standalone interaction share evaluation failures preserve error identity", async () => {
  const failure = new Error("page closed");
  await assert.rejects(app.waitForShareSurface({ evaluate: async () => { throw failure; } }, 1000), (error) => error === failure);
});

test("standalone interaction prompt detection keeps count and accessor failure behavior", async () => {
  assert.equal(await app.hasPromptTextbox({ locator: () => locator({ count: async () => 2 }) }), true);
  assert.equal(await app.hasPromptTextbox({ locator: () => missingLocator() }), false);
  assert.equal(await app.hasPromptTextbox({ locator() { throw new Error("closed"); } }), false);
});

test("standalone interaction first-visible selection preserves order and fixed timeouts", async () => {
  const calls = [];
  const first = locator({ async click() { calls.push("failed"); throw new Error("detached"); } });
  const second = locator({ async waitFor(options) { calls.push(options); }, async click(options) { calls.push(options); } });
  assert.equal(await app.clickFirstVisible([missingLocator(), first, second]), true);
  assert.deepEqual(calls, ["failed", { state: "visible", timeout: 4000 }, { timeout: 12000, force: true }]);
  assert.equal(await app.clickFirstVisible([missingLocator(), first]), false);
});

test("standalone interaction new chat retains candidate fallback and settlement", async () => {
  const calls = [], fallback = locator({ async click() { calls.push("click"); } });
  const page = { getByRole: () => missingLocator(), locator: () => fallback, async waitForTimeout(ms) { calls.push(ms); } };
  assert.equal(await app.clickNewChat(page), true);
  assert.deepEqual(calls, ["click", 1200]);
});

for (const [operation, label] of [["image", "Create image"], ["music", "Create music"], ["video", "Create video"]]) {
  test("standalone interaction simple " + operation + " mode preserves role fallback", async () => {
    const calls = [], control = locator({ async click(options) { calls.push(options); } });
    const page = {
      locator: () => missingLocator(),
      getByRole(role, { name }) { assert.equal(role, "button"); assert.equal(name.test(label), true); return control; },
      async waitForTimeout(ms) { calls.push(ms); },
    };
    assert.equal(await app.clickOperationMode(page, operation), true);
    assert.deepEqual(calls, [{ timeout: 12000, force: true }, 1500]);
  });
}

for (const kind of ["popup", "same_page", "none"]) {
  test("standalone interaction share entry preserves " + kind + " page ownership", async () => {
    const calls = [], matchers = [];
    const popup = {
      async waitForLoadState(state, options) { calls.push([state, options]); throw new Error("load event already passed"); },
      async waitForTimeout(ms) { calls.push(["popup settle", ms]); },
    };
    const control = kind === "none" ? missingLocator() : locator({ async click(options) { calls.push(["click", options]); } });
    const page = {
      locator() { return { filter({ hasText }) { matchers.push(hasText); return control; } }; },
      waitForEvent(event, options) { calls.push([event, options]); return Promise.resolve(kind === "popup" ? popup : null); },
      async waitForTimeout(ms) { calls.push(["page settle", ms]); },
    };
    const result = await app.tryFollowShareEntryPoint(page);
    assert.equal(result.kind, kind);
    assert.equal(result.page, kind === "popup" ? popup : page);
    assert.deepEqual(matchers.map((pattern, i) => pattern.test(["Try Gemini Canvas", "Continue", "Create image", "Open in new window"][i])), [true, true, true, true]);
    if (kind === "none") assert.deepEqual(calls, []);
    else {
      assert.deepEqual(calls[0], ["popup", { timeout: 6000 }]);
      assert.deepEqual(calls[1], ["click", { timeout: 12000, force: true }]);
      assert.deepEqual(calls.at(-1), [kind === "popup" ? "popup settle" : "page settle", 3000]);
    }
  });
}

test("standalone interaction share entry retries a failed click without adopting a page", async () => {
  let candidate = 0, armed = 0;
  const page = {
    locator() {
      const index = candidate++;
      return locator({ async click() { if (!index) throw new Error("detached"); } });
    },
    waitForEvent() { armed += 1; return Promise.resolve(null); },
    async waitForTimeout() {},
  };
  assert.deepEqual(await app.tryFollowShareEntryPoint(page), { kind: "same_page", page });
  assert.equal(armed, 2);
});

function standaloneComposer(options) {
  const f = composerFixture(options);
  for (const button of f.buttons.values()) button.count = async () => 1;
  f.page.getByRole = (role, { name }) => {
    assert.equal(role, "button"); assert.equal(name.test("Send"), true); assert.equal(name.test("Submit"), false);
    return f.buttons.get("role");
  };
  return f;
}

for (const kind of ["input", "textarea"]) {
  test("standalone interaction " + kind + " dispatches native input before change", async () => {
    const { page, node, calls } = standaloneComposer({ kind });
    await app.submitPrompt(page, "fixture prompt");
    assert.equal(node.value, "fixture prompt");
    assert.deepEqual(node.events, [
      { type: "input", bubbles: true, cancelable: true, data: "fixture prompt", inputType: "insertText" },
      { type: "change", bubbles: true },
    ]);
    assert.deepEqual(calls, [
      ["textbox", { state: "visible", timeout: 30000 }], ["focus"], ["settle", 300],
      ["wait", "role", { state: "visible", timeout: 4000 }], ["click", "role", { timeout: 12000, force: true }],
    ]);
  });
}

test("standalone interaction contenteditable does not force keyboard retyping", async () => {
  const { page, node, calls } = standaloneComposer({ kind: "contenteditable" });
  await app.submitPrompt(page, "fixture prompt");
  assert.equal(node.textContent, "fixture prompt");
  assert.equal(node.innerHTML, "old");
  assert.equal(node.events.length, 1);
  assert.equal(calls.some(([kind]) => kind === "press" || kind === "type"), false);
});

test("standalone interaction fulfilled unsupported input callback retains legacy no-retype behavior", async () => {
  const { page, node, calls } = standaloneComposer({ kind: "unsupported" });
  await app.submitPrompt(page, "fixture prompt");
  assert.deepEqual(node.events, []);
  assert.equal(calls.some(([kind]) => kind === "press" || kind === "type"), false);
});

test("standalone interaction failed population uses platform selection and twelve-millisecond typing", async () => {
  const modifier = process.platform === "win32" ? "Control" : "Meta";
  const { page, calls } = standaloneComposer({ evaluateFailure: new Error("detached"), failedKey: modifier + "+A" });
  await app.submitPrompt(page, "fixture prompt");
  assert.deepEqual(calls.filter(([kind]) => kind === "press" || kind === "type"), [
    ["press", modifier + "+A"], ["press", "Backspace"], ["type", "fixture prompt", { delay: 12 }],
  ]);
});

for (const failedButtons of [["role"], ["role", "named"]]) {
  test("standalone interaction send fallback after " + failedButtons.join(" and "), async () => {
    const { page, calls } = standaloneComposer({ failedButtons });
    await app.submitPrompt(page, "fixture prompt");
    assert.deepEqual(calls.filter(([kind]) => kind === "click").map(([, key]) => key), ["role", "named"]);
    assert.deepEqual(calls.filter(([kind]) => kind === "press"), failedButtons.length === 2 ? [["press", "Enter"]] : []);
  });
}

test("standalone interaction textbox visibility failure stops before focus", async () => {
  const failure = new Error("textbox unavailable"), { page, calls } = standaloneComposer({ visibilityFailure: failure });
  await assert.rejects(app.submitPrompt(page, "fixture prompt"), (error) => error === failure);
  assert.deepEqual(calls, [["textbox", { state: "visible", timeout: 30000 }]]);
});
