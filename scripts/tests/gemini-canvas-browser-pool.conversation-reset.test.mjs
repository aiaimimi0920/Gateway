import assert from "node:assert/strict";
import test from "node:test";
import { app, appFixture, baseUrl, browserTest, readyHtml } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

function resetFixture(options = {}) {
  const state = { url: `${baseUrl}/app/old`, body: "Talk to Gemini", present: [], failures: {}, events: [], ...options };
  const selector = (key) => ({
    first() { return this; },
    async count() { return state.present.includes(key) ? 1 : 0; },
    async waitFor(config) {
      state.events.push(["visible", key, config]);
      if (!state.present.includes(key) || state.failures[key] === "visible") throw new Error("not visible");
    },
    async click(config) {
      state.events.push(["click", key, config]);
      if (state.failures[key] === "click") throw new Error("click failed");
    },
  });
  const page = {
    url: () => state.url,
    async evaluate() {
      state.events.push(["evaluate"]);
      if (state.evaluateError) throw state.evaluateError;
      return state.body;
    },
    async goto(url, config) {
      state.events.push(["goto", url, config]);
      if (state.gotoError) throw state.gotoError;
      state.url = url;
    },
    async waitForTimeout(ms) { state.events.push(["wait", ms]); },
    getByRole(role, { name }) {
      for (const [label, key] of [["New chat", "new-chat"], ["Not now", "not-now"], ["Got it", "close"], ["Skip", "skip"], ["Gemini", "gemini"]]) {
        if (name.test(label)) return selector(`${role}:${key}`);
      }
      throw new Error(`Unexpected role selector: ${role} ${name}`);
    },
    locator(css) {
      assert.equal(css, 'button[aria-label*="发起新对话"], button[aria-label*="New chat"]');
      return selector("aria:new-chat");
    },
  };
  return { page, state, events: (type) => state.events.filter((event) => event[0] === type) };
}

for (const key of ["button:new-chat", "aria:new-chat", "link:new-chat"]) {
  test(`new chat selects ${key} with visibility and click deadlines`, async () => {
    const { page, events } = resetFixture({ present: [key] });
    assert.equal(await app.clickNewChat(page), true);
    assert.deepEqual(events("visible"), [["visible", key, { state: "visible", timeout: 4000 }]]);
    assert.deepEqual(events("click"), [["click", key, { timeout: 12000, force: true }]]);
    assert.deepEqual(events("wait"), [["wait", 1200]]);
  });
}

for (const failure of ["visible", "click"]) {
  test(`new chat continues after the first candidate ${failure} failure`, async () => {
    const { page, events } = resetFixture({
      present: ["button:new-chat", "link:new-chat"], failures: { "button:new-chat": failure },
    });
    assert.equal(await app.clickNewChat(page), true);
    assert.equal(events("click").at(-1)[1], "link:new-chat");
    assert.deepEqual(events("wait"), [["wait", 1200]]);
  });
}

test("new chat without candidates returns false without waiting", async () => {
  const { page, state } = resetFixture();
  assert.equal(await app.clickNewChat(page), false);
  assert.deepEqual(state.events, []);
});

for (const preferred of [`${baseUrl}/app/program`, `${baseUrl}/canvas`, `${baseUrl}/canvas/program`]) {
  test(`reset preserves program context at ${preferred}`, async () => {
    const { page, events } = resetFixture({ present: ["button:new-chat"] });
    assert.equal(await app.resetConversation(page, baseUrl, 2500, preferred), undefined);
    assert.deepEqual(events("goto"), [["goto", preferred, { waitUntil: "domcontentloaded", timeout: 2500 }]]);
    assert.deepEqual(events("click"), []);
    assert.equal(events("visible").length, 4);
    assert.deepEqual(events("wait"), [["wait", 1200]]);
  });
}

test("ordinary reset normalizes the entry URL and cleans interstitials before and after new chat", async () => {
  const { page, events } = resetFixture({ present: ["button:new-chat", "button:skip"] });
  await app.resetConversation(page, `${baseUrl}///`, 15000, `  ${baseUrl}/app  `);
  assert.deepEqual(events("goto"), [["goto", `${baseUrl}/app`, { waitUntil: "domcontentloaded", timeout: 15000 }]]);
  assert.deepEqual(events("click"), [
    ["click", "button:skip", { timeout: 6000, force: true }],
    ["click", "button:new-chat", { timeout: 10000, force: true }],
    ["click", "button:skip", { timeout: 6000, force: true }],
  ]);
  assert.deepEqual(events("wait"), [["wait", 1200], ["wait", 800], ["wait", 1200], ["wait", 800]]);
});

test("reset falls back to the Gemini button after a new-chat click failure", async () => {
  const { page, events } = resetFixture({
    present: ["button:new-chat", "button:gemini"], failures: { "button:new-chat": "click" },
  });
  await app.resetConversation(page, `${baseUrl}///`, 750);
  assert.equal(events("goto")[0][1], `${baseUrl}/app`);
  assert.deepEqual(events("click"), [
    ["click", "button:new-chat", { timeout: 750, force: true }],
    ["click", "button:gemini", { timeout: 750, force: true }],
  ]);
  assert.equal(events("visible").some((event) => event[1].includes("new-chat")), false);
});

test("ready attached program skips navigation and leaves the conversation untouched", async () => {
  const { page, events } = resetFixture({ present: ["button:new-chat"] });
  await app.resetConversation(page, baseUrl, 500, `${baseUrl}/app/program`, { skipInitialNavigationWhenAppSurfaceReady: true });
  assert.deepEqual(events("goto"), []);
  assert.deepEqual(events("click"), []);
  assert.deepEqual(events("wait"), []);
});

test("ready attached ordinary app still requests new chat when navigation is skipped", async () => {
  const { page, events } = resetFixture({ present: ["link:gemini"] });
  await app.resetConversation(page, baseUrl, 500, null, { skipInitialNavigationWhenAppSurfaceReady: true });
  assert.deepEqual(events("goto"), []);
  assert.deepEqual(events("click"), [["click", "link:gemini", { timeout: 500, force: true }]]);
  assert.deepEqual(events("wait"), [["wait", 1200]]);
});

for (const [label, options, skip] of [
  ["signed-out body", { body: "New chat Sign in Meet Gemini, your personal AI assistant" }, true],
  ["non-app path", { url: `${baseUrl}/share/program` }, true],
  ["different origin", { url: "https://fixture.invalid/app" }, true],
  ["unrecognized body", { body: "Loading" }, true],
  ["body evaluation rejection", { evaluateError: new Error("detached") }, true],
  ["truthy non-boolean option", {}, "true"],
]) {
  test(`reset navigates instead of reusing ${label}`, async () => {
    const { page, events } = resetFixture(options);
    await app.resetConversation(page, baseUrl, 500, null, { skipInitialNavigationWhenAppSurfaceReady: skip });
    assert.equal(events("goto").length, 1);
    assert.equal(events("evaluate").length, skip === true ? 1 : 0);
  });
}

test("reset propagates initial navigation failure before any waits or cleanup", async () => {
  const error = new Error("navigation failed");
  const { page, state } = resetFixture({ gotoError: error });
  await assert.rejects(app.resetConversation(page, baseUrl, 500), (actual) => actual === error);
  assert.deepEqual(state.events, [["goto", `${baseUrl}/app`, { waitUntil: "domcontentloaded", timeout: 500 }]]);
});

for (const timeout of [350, 15000]) {
  test(`interstitial dismissal keeps order and deadline caps at ${timeout} ms`, async () => {
    const keys = ["button:not-now", "link:not-now", "button:close", "button:skip"];
    const { page, events } = resetFixture({ present: keys, failures: { "link:not-now": "click" } });
    await app.dismissGeminiAppInterstitials(page, timeout);
    assert.deepEqual(events("visible"), keys.map((key) => ["visible", key, { state: "visible", timeout: Math.min(timeout, 2000) }]));
    assert.deepEqual(events("click"), keys.map((key) => ["click", key, { timeout: Math.min(timeout, 6000), force: true }]));
    assert.deepEqual(events("wait"), [["wait", 800], ["wait", 800], ["wait", 800]]);
  });
}

test("offline browser new-chat selector clicks the Chinese button", browserTest, async (t) => {
  const { page, state } = await appFixture(t, { html: '<button onclick="this.dataset.clicked=1">发起新对话</button>' });
  assert.equal(await app.clickNewChat(page), true);
  assert.equal(await page.locator("button").getAttribute("data-clicked"), "1");
  assert.deepEqual(state.waits, [1200]);
  assert.deepEqual(state.routes, []);
});

test("offline browser reset reuses the program and retains its draft", browserTest, async (t) => {
  const { page, state } = await appFixture(t, { initialUrl: `${baseUrl}/app/program`, html: readyHtml });
  await page.locator("#draft").fill("retained reset draft");
  await app.resetConversation(page, baseUrl, 2000, `${baseUrl}/app/program`, { skipInitialNavigationWhenAppSurfaceReady: true });
  assert.equal(await page.locator("#draft").inputValue(), "retained reset draft");
  assert.deepEqual(state.routes, []);
  assert.deepEqual(state.waits, []);
});
