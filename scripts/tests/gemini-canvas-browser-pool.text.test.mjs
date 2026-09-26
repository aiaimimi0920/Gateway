import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";
import { textHarness, textPage, textSnapshot } from "./gemini-canvas-browser-pool.text-fixtures.mjs";

const app = await importTestableScript();
const stages = (h, name) => h.calls.filter(([stage]) => stage === name);

for (const marker of ["reply with exactly:", "return exactly:", "output exactly:", "respond with exactly:"]) {
  test(`Text exact-answer directive recognizes ${marker}`, () => {
    assert.equal(app.extractExactAnswerDirective(`Task. ${marker.toUpperCase()} \n \"Paris\"\nIgnore later lines`), "Paris");
  });
}

test("Text exact-answer directive preserves marker priority and skips empty quoted lines", () => {
  assert.equal(app.extractExactAnswerDirective("return exactly: Second\nreply with exactly: ``\n`First`"), "First");
  for (const prompt of [null, " ", "reply with exactly: ", "No directive"]) assert.equal(app.extractExactAnswerDirective(prompt), null);
});

test("Text tool-history augmentation appends the exact answer for both caller-result markers", () => {
  for (const history of ["Caller-provided result for `weather`: sunny", "The caller has already executed every required external tool."]) {
    const prompt = `${history}\nReturn exactly: \"Paris\"`;
    assert.equal(app.augmentToolHistoryPrompt(prompt), `${prompt}\n\nRequired exact final answer:\nParis\n\nReturn exactly that text and nothing else.`);
  }
});

test("Text tool-history augmentation preserves ordinary absent and already augmented prompts", () => {
  for (const prompt of ["", "return exactly: Paris", "Caller-provided result for weather: sunny\nreturn exactly: Paris", "Caller-provided result for `weather`: sunny", "Caller-provided result for `weather`: sunny\nreturn exactly: Paris\nRequired exact final answer: Paris"]) {
    assert.equal(app.augmentToolHistoryPrompt(prompt), prompt);
  }
});

test("Text real snapshot serializes selectors filters blank nodes and caps body text", async () => {
  const page = textPage(() => textSnapshot("  Primary  ", {
    primaryTexts: ["", "  Primary  ", "  "], fallbackTexts: ["  Alternate  ", ""],
    bodyText: "x".repeat(13000), buttons: [{ text: "Cancel", disabled: false }, { aria: "Send", disabled: true }],
  }));
  assert.deepEqual(await app.collectTextSnapshot(page), {
    url: "https://fixture.invalid/result", bodyText: "x".repeat(12000),
    primaryTexts: ["Primary"], fallbackTexts: ["Alternate"], sendDisabled: true,
  });
});

test("Text real snapshot tolerates missing body and send button", async () => {
  const result = await app.collectTextSnapshot(textPage(() => textSnapshot("", { bodyText: null, buttons: [] })));
  assert.deepEqual([result.bodyText, result.primaryTexts, result.sendDisabled], ["", [], false]);
});

test("Text real entry rejects invalid prompts before page access", async () => {
  for (const prompt of [undefined, " ", 12]) {
    await assert.rejects(app.runTextOperation(null, { prompt }), (error) => {
      assert.deepEqual([error.status, error.code], [400, "gemini_canvas_invalid_text_prompt"]);
      return true;
    });
  }
});

test("Text execution captures before reset takes a baseline then submits and stabilizes", async () => {
  const h = textHarness(app), result = await h.run();
  const order = h.calls.map(([stage]) => stage).filter((stage) => stage !== "log");
  assert.deepEqual(order, ["resolve", "capture", "reset", "snapshot", "submit", "snapshot", "wait", "snapshot", "handle", "stop"]);
  assert.deepEqual(stages(h, "reset")[0].slice(2), ["https://gemini.google.com", 3000, "https://fixture.invalid/preferred"]);
  assert.deepEqual(stages(h, "submit")[0].slice(2), ["fixture prompt", 3000]);
  assert.equal(stages(h, "capture")[0][2], "text");
  assert.deepEqual(result, {
    operation: "text", pageUrl: "https://fixture.invalid/result", bodyText: "fixture prompt",
    appPath: "/app/fixture", conversationId: "fixture-conversation", text: "A complete fixture answer.", media: [],
    networkEvents: h.state.events, rpcCaptures: h.state.rpcCaptures,
  });
  assert.equal(stages(h, "handle")[0][4], h.state);
});

test("Text execution submits augmented prompt and passes explicit timeout and base URL", async () => {
  const prompt = "Caller-provided result for `place`: Paris\nReply with exactly: Paris";
  const effectivePrompt = `${prompt}\n\nRequired exact final answer:\nParis\n\nReturn exactly that text and nothing else.`;
  const h = textHarness(app, { snapshots: [textSnapshot(""), textSnapshot("Paris", { bodyText: effectivePrompt })] });
  assert.equal((await h.run({ prompt, timeoutMs: 8000, baseUrl: "https://fixture.invalid/base" })).text, "Paris");
  assert.deepEqual(stages(h, "submit")[0].slice(2), [effectivePrompt, 8000]);
  assert.deepEqual(stages(h, "reset")[0].slice(2), ["https://fixture.invalid/base", 8000, "https://fixture.invalid/preferred"]);
  assert.equal(stages(h, "handle")[0][1], "https://fixture.invalid/base");
});

test("Text execution prefers primary responses and filters transient status lines", async () => {
  const h = textHarness(app, { snapshots: [textSnapshot("old"), textSnapshot("Thinking\nPrimary answer", { fallbackTexts: ["Wrong fallback"] })] });
  assert.equal((await h.run()).text, "Primary answer");
});

test("Text execution compares alternate responses against the alternate baseline", async () => {
  const h = textHarness(app, { snapshots: [textSnapshot("", { fallbackTexts: ["Old"] }), textSnapshot("", { fallbackTexts: ["New alternate"] })] });
  assert.equal((await h.run()).text, "New alternate");
});

for (const [reason, baseline, current] of [
  ["missing prompt anchor", textSnapshot(""), textSnapshot("A new answer", { bodyText: "unrelated conversation" })],
  ["unchanged baseline", textSnapshot("Existing answer"), textSnapshot("Existing answer")],
  ["generic welcome", textSnapshot(""), textSnapshot("Hello! How can I help you today?")],
  ["transient status only", textSnapshot(""), textSnapshot("Thinking\nPreparing response")],
  ["empty response", textSnapshot(""), textSnapshot("")],
]) {
  test(`Text timeout rejects ${reason} and retains final body and capture state`, async () => {
    const h = textHarness(app, { snapshots: [baseline, current] });
    await assert.rejects(h.run(), (error) => {
      assert.deepEqual([error.status, error.code, error.bodyText], [504, "gemini_canvas_text_timeout", current.bodyText]);
      assert.equal(error.captureState, h.state);
      return true;
    });
    assert.deepEqual(stages(h, "wait"), [["wait", 1500], ["wait", 1500]]);
    assert.equal(stages(h, "stop").length, 1);
  });
}

test("Text greeting permits welcome responses and repeated text is new after count growth", async () => {
  const answer = "Hello! How can I help you today?";
  const h = textHarness(app, { snapshots: [textSnapshot(answer), textSnapshot(answer, { bodyText: "hi", primaryTexts: [answer, answer] })] });
  assert.equal((await h.run({ prompt: "hi" })).text, answer);
  assert.equal(h.now, 1500);
});

test("Text changed candidate restarts stable-hit counting", async () => {
  const h = textHarness(app, { snapshots: [textSnapshot(""), textSnapshot("First complete response."), textSnapshot("Second complete response.")] });
  assert.equal((await h.run({ timeoutMs: 6000 })).text, "Second complete response.");
  assert.equal(h.now, 3000);
  assert.equal(stages(h, "snapshot").length, 4);
});

for (const [reason, answer, sendDisabled, bodyText] of [
  ["disabled send", "OK", true, "fixture prompt Generating"],
  ["substantive answer", "A sufficiently long final answer.", false, "fixture prompt Generating"],
  ["tool payload", "<invoke x>", false, "fixture prompt Generating"],
  ["completed generation", "OK", false, "fixture prompt"],
]) {
  test(`Text stable completion accepts ${reason} only after two hits`, async () => {
    const h = textHarness(app, { snapshots: [textSnapshot(""), textSnapshot(answer, { bodyText, sendDisabled })] });
    assert.equal((await h.run()).text, answer);
    assert.equal(h.now, 1500);
    assert.equal(stages(h, "snapshot").length, 3);
    assert.equal(stages(h, "stop").length, 1);
  });
}

test("Text deadline fallback retains the latest new short candidate while generation continues", async () => {
  const h = textHarness(app, { snapshots: [textSnapshot(""), textSnapshot("A", { bodyText: "fixture prompt Generating" }), textSnapshot("B", { bodyText: "fixture prompt Generating" })] });
  assert.equal((await h.run()).text, "B");
  assert.equal(h.now, 3000);
  assert.equal(stages(h, "snapshot").length, 3);
  assert.equal(stages(h, "stop").length, 1);
});

for (const [failureAt, failureCall, expectedStops] of [["capture", 1, 0], ["reset", 1, 1], ["snapshot", 1, 1], ["snapshot", 2, 1], ["submit", 1, 1], ["wait", 1, 1], ["handle", 1, 1]]) {
  test(`Text ${failureAt} failure on call ${failureCall} preserves error and capture ownership`, async () => {
    const failure = new Error("fixture failure"), h = textHarness(app, { failureAt, failureCall, failure });
    await assert.rejects(h.run(), (error) => error === failure);
    assert.equal(stages(h, "stop").length, expectedStops);
  });
}
