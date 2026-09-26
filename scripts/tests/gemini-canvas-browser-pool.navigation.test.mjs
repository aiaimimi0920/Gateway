import assert from "node:assert/strict";
import test from "node:test";
import { app, appFixture, baseUrl, browserTest, signedOutHtml } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

test("program URL resolution preserves explicit, app-path and conversation precedence", () => {
  assert.equal(app.resolveProgramPageUrl(baseUrl, { canvasProgramUrl: `${baseUrl}/app/deadbeef`, appPath: "/app/12345678" }), `${baseUrl}/app/deadbeef`);
  assert.equal(app.resolveProgramPageUrl(baseUrl, { programUrl: "invalid", appPath: "/app/12345678", conversationId: "c_abcdef12" }), `${baseUrl}/app/12345678`);
  assert.equal(app.resolveProgramPageUrl(baseUrl, { conversationId: "c_abcdef12", pageUrl: `${baseUrl}/app/12345678` }), `${baseUrl}/app/abcdef12`);
  assert.equal(app.resolveProgramPageUrl(baseUrl, { pageUrl: `${baseUrl}/app/12345678` }), `${baseUrl}/app/12345678`);
  assert.equal(app.resolveProgramPageUrl(baseUrl, { pageUrl: `${baseUrl}/share/fixture` }), null);
});

test("program navigation with no target does not access a page", async () => {
  assert.equal(await app.ensureProgramPage({}, baseUrl, null, 100), false);
});

test("program and share navigation report a closed real page", browserTest, async (t) => {
  const { page, entry } = await appFixture(t);
  await page.close();
  await assert.rejects(app.ensureProgramPage(entry, baseUrl, `${baseUrl}/app/deadbeef`, 100), { code: "gemini_canvas_page_closed", status: 500 });
  await assert.rejects(app.ensureSharePage(entry, baseUrl, "fixture", 100), { code: "gemini_canvas_page_closed", status: 500 });
});

test("program navigation preserves an existing same-path draft despite query and trailing slash differences", browserTest, async (t) => {
  const { page, entry, state } = await appFixture(t, { initialUrl: `${baseUrl}/app/deadbeef/?authuser=2` });
  await page.locator("textarea").fill("synthetic unsent draft");
  assert.equal(await app.ensureProgramPage(entry, baseUrl, `${baseUrl}/app/deadbeef?authuser=3`, 100), true);
  assert.equal(await page.locator("textarea").inputValue(), "synthetic unsent draft");
  assert.deepEqual(state.routes, []);
});

test("program navigation loads a different concrete document in the same owned page", browserTest, async (t) => {
  const { page, entry, state } = await appFixture(t);
  assert.equal(await app.ensureProgramPage(entry, baseUrl, `${baseUrl}/app/deadbeef`, 2000), true);
  assert.equal(page.url(), `${baseUrl}/app/deadbeef`);
  assert.equal(entry.page, page);
  assert.deepEqual(state.routes, ["/app/deadbeef"]);
});

test("program navigation rejects a sign-in document with its program-specific auth code", browserTest, async (t) => {
  const url = "https://accounts.google.com/signin";
  const { entry } = await appFixture(t, { initialUrl: url, html: signedOutHtml });
  await assert.rejects(app.ensureProgramPage(entry, baseUrl, url, 100), { code: "gemini_canvas_program_auth_redirect", status: 401 });
});

test("share navigation requires an identifier before any document navigation", browserTest, async (t) => {
  const { entry, state } = await appFixture(t);
  await assert.rejects(app.ensureSharePage(entry, baseUrl, " ", 100), { code: "gemini_canvas_missing_share_id", status: 400 });
  assert.deepEqual(state.routes, []);
});

for (const configuredBase of [`${baseUrl}/u/3/app`, `${baseUrl}?authuser=3`]) {
  test(`share navigation retains account scope from ${configuredBase}`, browserTest, async (t) => {
    const { entry, page } = await appFixture(t, { html: "<main>Try Gemini Canvas</main>" });
    await app.ensureSharePage(entry, configuredBase, "fixture", 2000);
    const url = new URL(page.url());
    assert.equal(url.pathname, "/share/fixture");
    assert.equal(url.searchParams.get("authuser"), "3");
    assert.equal(entry.page, page);
  });
}

for (const ready of [false, true]) {
  test(`share navigation ${ready ? "accepts a ready surface with incidental sign-in text" : "rejects a sign-in-only surface"}`, browserTest, async (t) => {
    const { entry } = await appFixture(t, { html: `<main>Sign in ${ready ? "Try Gemini Canvas" : "to continue"}</main>` });
    const result = app.ensureSharePage(entry, baseUrl, "fixture", 100);
    if (ready) await result;
    else await assert.rejects(result, { code: "gemini_canvas_auth_required", status: 401 });
  });
}
