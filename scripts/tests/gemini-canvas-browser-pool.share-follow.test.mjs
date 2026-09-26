import assert from "node:assert/strict";
import test from "node:test";
import { app, appFixture, baseUrl, browserTest, readyHtml } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

test("share entry following returns a real popup while leaving adoption to its caller", browserTest, async (t) => {
  const { context, page, entry } = await appFixture(t, {
    initialUrl: `${baseUrl}/share/fixture`,
    html: (request) => new URL(request.url()).pathname.startsWith("/share/")
      ? '<button onclick="window.open(\'/app/deadbeef\')">Continue</button>' : readyHtml,
  });
  const result = await app.tryFollowShareEntryPoint(page);
  assert.equal(result.kind, "popup");
  assert.notEqual(result.page, page);
  assert.equal(result.page.url(), `${baseUrl}/app/deadbeef`);
  assert.equal(entry.page, page);
  assert.equal(page.isClosed(), false);
  assert.equal(context.pages().length, 2);
  await result.page.close();
});

test("share entry following returns the same real page after local document navigation", browserTest, async (t) => {
  const { page, entry } = await appFixture(t, {
    initialUrl: `${baseUrl}/share/fixture`,
    html: (request) => new URL(request.url()).pathname.startsWith("/share/")
      ? '<button onclick="location.href=\'/app/deadbeef\'">Continue</button>' : readyHtml,
  });
  const result = await app.tryFollowShareEntryPoint(page);
  assert.deepEqual(result, { kind: "same_page", page });
  assert.equal(entry.page, page);
  assert.equal(page.url(), `${baseUrl}/app/deadbeef`);
});

test("share entry failure preserves the real page and bounds its diagnostics", browserTest, async (t) => {
  const buttons = Array.from({ length: 100 }, (_, i) => `<button>Unrelated ${i}</button>`).join("");
  const { page, entry } = await appFixture(t, { initialUrl: `${baseUrl}/share/fixture`, html: `<main>${"x".repeat(2000)}</main>${buttons}` });
  const logs = [];
  t.mock.method(console, "log", (...parts) => logs.push(parts));
  assert.deepEqual(await app.tryFollowShareEntryPoint(page), { kind: "none", page });
  assert.equal(entry.page, page);
  assert.equal(page.isClosed(), false);
  const details = JSON.parse(logs.find((parts) => parts[1] === "share entry point follow failed")[2]);
  assert.equal(details.bodyPreview.length, 1200);
  assert.equal(details.buttons.length, 80);
  assert.deepEqual(details.attemptedKinds, []);
});
