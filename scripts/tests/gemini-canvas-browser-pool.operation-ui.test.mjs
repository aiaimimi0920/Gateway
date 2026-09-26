import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { locator, missingLocator, ElementFixture, domPage } from "./gemini-canvas-browser-pool.operation-ui-fixtures.mjs";

const app = await importTestableScript();

for (const [operation, action, label] of [["music", "download", "Download music"], ["music", "play", "Play"], ["video", "download", "Download video"], ["video", "play", "Play video"]]) {
  test(`operation UI ${operation} ${action} falls back past missing role controls`, async () => {
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

test("operation UI unsupported actions and text mode return before accessing the page", async () => {
  const page = new Proxy({}, { get() { throw new Error("unexpected page access"); } });
  assert.equal(await app.clickMediaActionButton(page, "image", "download", 1000), false);
  assert.equal(await app.clickMediaActionButton(page, "video", "unknown", 1000), false);
  assert.equal(await app.clickOperationMode(page, "text", 1000), false);
});

test("operation UI action click retries failed candidates with shorter caller timeouts", async () => {
  const first = locator({ async click() { throw new Error("fixture detached"); } });
  let clicks = 0;
  const second = locator({ async waitFor({ timeout }) { assert.equal(timeout, 25); }, async click({ timeout }) { assert.equal(timeout, 25); clicks += 1; } });
  const page = { getByRole: () => first, locator: () => second, async waitForTimeout() {} };
  assert.equal(await app.clickMediaActionButton(page, "music", "download", 25), true);
  assert.equal(clicks, 1);
});

test("operation UI selector retries ordered controls and reports complete absence", async () => {
  const missing = missingLocator(), calls = [];
  const opened = locator({ async waitFor(options) { calls.push(options); }, async click(options) { calls.push(options); } });
  const page = {
    getByRole(role, { name }) { return name.test("Open mode selector") ? opened : missing; },
    locator: () => missing, async waitForTimeout(ms) { calls.push(ms); },
  };
  assert.equal(await app.openOperationModeSelector(page, 90000), true);
  assert.deepEqual(calls, [{ state: "visible", timeout: 5000 }, { timeout: 10000, force: true }, 800]);
  page.getByRole = () => missing;
  assert.equal(await app.openOperationModeSelector(page, 1), false);
});

test("operation UI selected route evidence avoids unnecessary control reads", async () => {
  const page = { url: () => "https://gemini.google.com/images?mode=1", getByRole() { throw new Error("unneeded control read"); } };
  assert.equal(await app.operationModeAppearsSelected(page, { selectedButtonName: /Deselect/, routePathPattern: /\/images(?:\?|$)/ }, 1000), true);
});

test("operation UI selected link evidence survives missing button controls", async () => {
  const page = { getByRole: (role) => locator({ async isVisible({ timeout }) { assert.equal(timeout, 300); return role === "link"; } }) };
  assert.equal(await app.operationModeAppearsSelected(page, { selectedButtonName: /Deselect/ }, 1000), true);
});

test("operation UI body evidence evaluates a self-contained callback", async () => {
  const { page } = domPage({ bodyText: "Choose a style for your image" });
  assert.equal(await app.operationModeAppearsSelected(page, { modeIndicator: /Choose a style/ }, 1000), true);
});

test("operation UI modes without selection evidence require no polling", async () => {
  assert.equal(await app.operationModeAppearsSelected(null, {}, 1000), true);
});

test("operation UI expired selection deadlines return without page access", async () => {
  assert.equal(await app.operationModeAppearsSelected(null, { modeIndicator: /selected/ }, 0), false);
});

test("operation UI selection retries accessor failures within the eight-second cap", async (t) => {
  let now = 0, waits = 0;
  t.mock.method(Date, "now", () => now);
  const failed = locator({ async isVisible() { throw new Error("detached control"); } });
  const page = {
    url() { throw new Error("closed page URL"); }, getByRole: () => failed,
    async evaluate() { throw new Error("detached DOM"); },
    async waitForTimeout(ms) { assert.equal(ms, 350); now += ms; waits += 1; },
  };
  assert.equal(await app.operationModeAppearsSelected(page, { routePathPattern: /selected/, selectedButtonName: /selected/, modeIndicator: /selected/ }, 90000), false);
  assert.equal(waits, 23);
  assert.equal(now, 8050);
});

test("operation UI music style selection reports missing candidates and settles", async () => {
  const { page, waits } = domPage();
  assert.deepEqual(await app.trySelectMusicStyleCard(page, 1000), { clicked: false, reason: "style_candidate_missing", styleCount: 0 });
  assert.deepEqual(waits, [1500]);
});

for (const [attempt, expected] of [[-2, 0], [99, 1]]) {
  test(`operation UI music style selection filters candidates and clamps attempt ${attempt}`, async () => {
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

test("operation UI music style selection clicks the visible ancestor and preserves metadata", async () => {
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

test("operation UI music style evaluation failures propagate without settling", async () => {
  const failure = new Error("fixture page closed");
  const page = { async evaluate() { throw failure; }, async waitForTimeout() { throw new Error("unexpected settle"); } };
  await assert.rejects(app.trySelectMusicStyleCard(page, 1000), (error) => error === failure);
});

test("operation UI mode fallback serializes DOM clicks and requires subsequent selected evidence", async () => {
  const control = new ElementFixture({ tagName: "BUTTON", innerText: "Create image", clickable: true });
  const { page, document } = domPage({ controls: [control] }), missing = missingLocator();
  control.onClick = () => { document.body.innerText = "Choose a style for your image"; };
  page.getByRole = (role, { name }) => name.test("Tools") ? locator() : missing;
  page.locator = page.getByText = () => missing;
  assert.equal(await app.clickOperationMode(page, "image", 10), true);
  assert.deepEqual(control.events, ["pointerdown", "mousedown", "mouseup", "click"]);
});
