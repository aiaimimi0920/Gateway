import assert from "node:assert/strict";
import test from "node:test";
import { submitPrompt } from "../gemini-canvas-browser-pool-composer.mjs";

for (const visible of [true, false]) {
  test(`single media send ${visible ? "propagates an ambiguous click failure" : "does not use an unknown icon or keyboard fallback"}`, async () => {
    let clicks = 0, keypresses = 0;
    const failure = new Error("fixture click outcome unknown");
    const editor = { first() { return this; }, async waitFor() {}, async focus() {}, async fill() {},
      async evaluate() { return { populated: true, editorKind: "native_input" }; } };
    const button = { first() { return this; }, last() { return this; },
      async isVisible() { return visible; }, async isEnabled() { return true; },
      async click() { clicks += 1; throw failure; } };
    const page = { locator: selector => selector.startsWith("[role=") ? editor : button,
      getByRole: () => button, async waitForTimeout() {}, keyboard: { async press() { keypresses += 1; } } };
    await assert.rejects(submitPrompt(page, "fixture", 1000, { singleSubmit: true }),
      visible ? error => error === failure : { code: "gemini_web_media_submit_unavailable" });
    assert.equal(clicks, visible ? 1 : 0);
    assert.equal(keypresses, 0);
  });
}

test("single media send fills through the locator and waits for the reactive send control", async (t) => {
  let now = 0, filledAt = null, clicks = 0;
  t.mock.method(Date, "now", () => now);
  const editor = { first() { return this; }, async waitFor() {}, async focus() {},
    async fill(value) { assert.equal(value, "literal music prompt"); filledAt = now; },
    async evaluate() { throw new Error("must not mutate the reactive editor DOM"); } };
  const button = { first() { return this; }, last() { return this; },
    async isVisible() { return filledAt !== null; }, async isEnabled() { return now - filledAt >= 750; },
    async click() { assert.equal(now >= 750, true); clicks += 1; } };
  const page = { locator: selector => selector.startsWith("[role=") ? editor : button,
    getByRole: () => button, async waitForTimeout(ms) { now += ms; },
    keyboard: { async press() { throw new Error("must not fall back to keyboard submission"); } } };
  await submitPrompt(page, "literal music prompt", 2000, { singleSubmit: true });
  assert.equal(clicks, 1);
});

test("single media send propagates a failed fill without trying to submit", async () => {
  const failure = new Error("editor detached during fill");
  const editor = { first() { return this; }, async waitFor() {}, async focus() {},
    async fill() { throw failure; }, async evaluate() { return { populated: true, editorKind: "native_input" }; } };
  const page = { locator: () => editor, getByRole() { throw new Error("must not inspect send controls after failed fill"); },
    async waitForTimeout() {} };
  await assert.rejects(submitPrompt(page, "literal", 1000, { singleSubmit: true }), error => error === failure);
});
