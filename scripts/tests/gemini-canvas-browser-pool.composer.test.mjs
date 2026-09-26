import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { composerFixture } from "./gemini-canvas-browser-pool.composer-fixtures.mjs";

const app = await importTestableScript();
const modifier = process.platform === "win32" ? "Control" : "Meta";
const prompt = "fixture prompt\nsecond line";

for (const kind of ["input", "textarea"]) {
  test(`composer ${kind} writes native value and dispatches input before change`, async () => {
    const { page, node, calls } = composerFixture({ kind });
    await app.submitPrompt(page, prompt, 90000);
    assert.equal(node.value, prompt);
    assert.deepEqual(node.events, [
      { type: "input", bubbles: true, cancelable: true, data: prompt, inputType: "insertText" },
      { type: "change", bubbles: true },
    ]);
    assert.deepEqual(calls, [
      ["textbox", { state: "visible", timeout: 30000 }], ["focus"], ["settle", 300],
      ["wait", "role", { state: "visible", timeout: 4000 }], ["click", "role", { timeout: 10000, force: true }],
    ]);
  });
}

test("composer contenteditable populates DOM and then performs keyboard typing", async () => {
  const { page, node, calls } = composerFixture({ kind: "contenteditable" });
  await app.submitPrompt(page, prompt, 25);
  assert.equal(node.innerHTML, "");
  assert.equal(node.textContent, prompt);
  assert.equal(node.events.length, 1);
  assert.equal(node.events[0].type, "input");
  assert.deepEqual(calls.filter(([kind]) => kind === "press" || kind === "type"), [["press", `${modifier}+A`], ["press", "Backspace"], ["type", prompt, { delay: 14 }]]);
});

test("composer evaluate failures fall back to keyboard even if selection clearing fails", async () => {
  const { page, calls } = composerFixture({ evaluateFailure: new Error("detached DOM"), failedKey: `${modifier}+A` });
  await app.submitPrompt(page, prompt, 25);
  assert.deepEqual(calls.filter(([kind]) => kind === "press" || kind === "type"), [["press", `${modifier}+A`], ["press", "Backspace"], ["type", prompt, { delay: 14 }]]);
  assert.deepEqual(calls.at(-1), ["click", "role", { timeout: 25, force: true }]);
});

for (const [failedButtons, expected] of [[["role"], "named"], [["role", "named"], "icon"]]) {
  test(`composer retries failed send controls until the ${expected} candidate succeeds`, async () => {
    const { page, calls } = composerFixture({ failedButtons });
    await app.submitPrompt(page, prompt, 25);
    assert.deepEqual(calls.filter(([kind]) => kind === "click").map(([, key]) => key), [...failedButtons, expected]);
    assert.equal(calls.some(([kind]) => kind === "press"), false);
  });
}

test("composer send candidates retain role named and last-icon priority", () => {
  const { page, buttons } = composerFixture();
  assert.deepEqual(app.buildSendButtonCandidates(page), [buttons.get("role"), buttons.get("named"), buttons.get("icon")]);
});

test("composer exhausted controls try modifier Enter and then plain Enter despite shortcut failure", async () => {
  const { page, calls } = composerFixture({ failedButtons: ["role", "named", "icon"], failedKey: `${modifier}+Enter` });
  await app.submitPrompt(page, prompt, 25);
  assert.deepEqual(calls.slice(-3), [["press", `${modifier}+Enter`], ["settle", 200], ["press", "Enter"]]);
});

test("composer textbox visibility failure propagates before focus or send", async () => {
  const failure = new Error("textbox unavailable"), { page, calls } = composerFixture({ visibilityFailure: failure });
  await assert.rejects(app.submitPrompt(page, prompt, 25), (error) => error === failure);
  assert.deepEqual(calls, [["textbox", { state: "visible", timeout: 25 }]]);
});

test("composer independent send retry preserves its shorter timeout caps", async () => {
  const { page, calls } = composerFixture({ failedButtons: ["role"] });
  assert.equal(await app.tryClickSendButton(page, 90000), true);
  assert.deepEqual(calls, [["wait", "role", { state: "visible", timeout: 1500 }], ["click", "role", { timeout: 5000, force: true }], ["wait", "named", { state: "visible", timeout: 1500 }], ["click", "named", { timeout: 5000, force: true }]]);
});

test("composer independent send retry reports exhausted controls without keyboard fallback", async () => {
  const { page, calls } = composerFixture({ failedButtons: ["role", "named", "icon"] });
  assert.equal(await app.tryClickSendButton(page, 25), false);
  assert.deepEqual(calls.filter(([kind]) => kind === "click").map(([, key]) => key), ["role", "named", "icon"]);
  assert.equal(calls.some(([kind]) => kind === "press" || kind === "type"), false);
});
