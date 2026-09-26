import assert from "node:assert/strict";
import test from "node:test";
import { capturePage, fixtureApiKey, importTestableExporter } from "./export-gemini-canvas-storage-state.fixtures.mjs";

const app = await importTestableExporter();

test("Canvas exporter inspection deduplicates pages and retains page and signal identity", async () => {
  const first = capturePage(), fallback = capturePage({ url: "https://gemini.google.com/app/fallback" });
  const pages = [first, first];
  const entries = await app.inspectGeminiPages({ pages: () => pages }, fallback);
  assert.deepEqual(entries.map((entry) => entry.page), [first, fallback]);
  assert.deepEqual(entries.map((entry) => entry.url), ["https://gemini.google.com/app/fixture", "https://gemini.google.com/app/fallback"]);
  assert.equal(entries[0].pageSignal.url, entries[0].url);
  assert.equal(entries[0].pageSignal.hasConversationInput, true);
  assert.equal(entries[0].pageSignal.hasGeminiSurface, true);
  assert.equal(entries[0].pageSignal.verificationChallengeVisible, false);
  assert.deepEqual(first.calls, ["url", "evaluate-1"]);
  assert.deepEqual(pages, [first, first]);
});

test("Canvas exporter inspection collects avatar, account and login signals from actual browser callback", async () => {
  const page = capturePage({ input: false, account: true, login: true, images: [
    { alt: "Profile photo", currentSrc: "https://lh3.googleusercontent.com/fixture-avatar" },
    { alt: "unrelated", src: "https://fixture.test/ignored.png" },
  ] });
  const [entry] = await app.inspectGeminiPages({ pages: () => [page] }, page);
  assert.equal(entry.pageSignal.hasNonDefaultAvatar, true);
  assert.equal(entry.pageSignal.hasAccountMenuButton, true);
  assert.equal(entry.pageSignal.hasConversationInput, false);
  assert.equal(entry.pageSignal.loginVisible, true);
  assert.deepEqual(Array.from(entry.pageSignal.avatarSources), ["https://lh3.googleusercontent.com/fixture-avatar"]);
  assert.equal(app.isGeminiAuthenticatedSignal({ url: entry.url, cookies: [{ name: "SAPISID", domain: ".google.com" }], pageSignal: entry.pageSignal }), false);
});

test("Canvas exporter inspection attaches verification challenge decision to collected signal", async () => {
  const page = capturePage({ bodyText: "Select all images that match", title: "Verify it's you" });
  const [entry] = await app.inspectGeminiPages({ pages: () => [page] }, null);
  assert.equal(entry.pageSignal.verificationChallengeVisible, true);
  assert.equal(app.canForceCompleteGeminiCapture({ url: entry.url, cookies: [{ name: "SAPISID", domain: ".google.com" }], pageSignal: entry.pageSignal }), false);
});

for (const message of ["Execution context was destroyed", "Target page, context or browser has been closed", "Cannot find context with specified id"]) {
  test(`Canvas exporter inspection skips transient page: ${message}`, async () => {
    const failed = capturePage({ failEvaluation: 1, evaluateError: new Error(message) });
    const fallback = capturePage();
    const entries = await app.inspectGeminiPages({ pages: () => [failed] }, fallback);
    assert.deepEqual(entries.map((entry) => entry.page), [fallback]);
  });
}

for (const stage of ["url", "evaluate"]) {
  test(`Canvas exporter inspection preserves nontransient ${stage} failure identity`, async () => {
    const failure = new Error("fixture unexpected page failure");
    const page = capturePage(stage === "url" ? { urlError: failure } : { failEvaluation: 1, evaluateError: failure });
    await assert.rejects(app.inspectGeminiPages({ pages: () => [page] }, null), (error) => error === failure);
  });
}

test("Canvas exporter runtime capture prioritizes preferred page, deduplicates keys and preserves input ordering", async () => {
  const timeline = [], keyA = fixtureApiKey("A"), keyB = fixtureApiKey("B"), keyC = fixtureApiKey("C");
  const first = capturePage({ label: "first", timeline, html: keyA, runtime: { WIZ_global_data: { key: keyB } } });
  const preferred = capturePage({ label: "preferred", timeline, url: "https://gemini.google.com/app/preferred", html: keyC, runtime: { firebaseConfig: { apiKey: keyA } } });
  const pages = [first, preferred];
  const result = await app.collectGeminiRuntimeMaterial({ pages: () => pages }, first, " https://gemini.google.com/app/preferred ");
  assert.deepEqual(result, { apiKeys: [keyC, keyA, keyB] });
  assert.deepEqual(timeline.filter(([, stage]) => stage === "content"), [["preferred", "content"], ["first", "content"]]);
  assert.deepEqual(pages, [first, preferred]);
});

test("Canvas exporter runtime capture retains discovery order without preferred match", async () => {
  const first = capturePage({ html: fixtureApiKey("A") });
  const fallback = capturePage({ html: fixtureApiKey("B") });
  const result = await app.collectGeminiRuntimeMaterial({ pages: () => [first] }, fallback, "https://fixture.test/missing");
  assert.deepEqual(result, { apiKeys: [fixtureApiKey("A"), fixtureApiKey("B")] });
});

for (const stage of ["content", "runtime"]) {
  test(`Canvas exporter runtime capture tolerates transient ${stage} loss and retains other source`, async () => {
    const failure = new Error("Execution context was destroyed");
    const page = capturePage({ html: fixtureApiKey("A"), runtime: { __NEXT_DATA__: { key: fixtureApiKey("B") } },
      ...(stage === "content" ? { contentError: failure } : { failEvaluation: 2, evaluateError: failure }),
    });
    const result = await app.collectGeminiRuntimeMaterial({ pages: () => [page] }, page, null);
    assert.deepEqual(result, { apiKeys: [fixtureApiKey(stage === "content" ? "B" : "A")] });
    assert.deepEqual(page.calls, ["url", "evaluate-1", "content", "evaluate-2"]);
  });
}

for (const stage of ["content", "runtime"]) {
  test(`Canvas exporter runtime capture preserves nontransient ${stage} failure`, async () => {
    const failure = new Error("fixture artifact failure");
    const page = capturePage(stage === "content" ? { contentError: failure } : { failEvaluation: 2, evaluateError: failure });
    await assert.rejects(app.collectGeminiRuntimeMaterial({ pages: () => [page] }, null, null), (error) => error === failure);
    assert.equal(page.calls.includes("evaluate-2"), stage === "runtime");
  });
}

test("Canvas exporter runtime capture handles absent pages and transient loss of both sources", async () => {
  assert.deepEqual(await app.collectGeminiRuntimeMaterial({ pages: () => [] }, null, null), { apiKeys: [] });
  const failure = new Error("Target page, context or browser has been closed");
  const page = capturePage({ contentError: failure, failEvaluation: 2, evaluateError: failure });
  assert.deepEqual(await app.collectGeminiRuntimeMaterial({ pages: () => [page] }, null, null), { apiKeys: [] });
});

test("Canvas exporter transient classification preserves Error and string handling", () => {
  assert.equal(app.isTransientNavigationError("Cannot find context with specified id"), true);
  assert.equal(app.isTransientNavigationError(new Error("Execution context was destroyed")), true);
  assert.equal(app.isTransientNavigationError(new Error("fixture operation failed")), false);
});
