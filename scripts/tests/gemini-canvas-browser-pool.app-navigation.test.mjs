import assert from "node:assert/strict";
import test from "node:test";
import { app, appFixture, baseUrl, browserTest, readyHtml, signedOutHtml } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

test("app preparation reports a closed page before navigating", browserTest, async (t) => {
  const { entry, page, state } = await appFixture(t);
  await page.close();
  await assert.rejects(app.ensureAppPage(entry, baseUrl, 2000), (error) => error.code === "gemini_canvas_page_closed" && error.status === 500);
  assert.deepEqual(state.routes, []);
});

test("app preparation preserves a ready page with matching origin and path", browserTest, async (t) => {
  const { entry, page, state } = await appFixture(t, { initialUrl: `${baseUrl}/app/?fixture=1` });
  await page.locator("#draft").fill("unsaved fixture draft");
  await app.ensureAppPage(entry, baseUrl, 2000);
  assert.deepEqual(state.routes, []);
  assert.equal(await page.locator("#draft").inputValue(), "unsaved fixture draft");
  assert.deepEqual(state.waits, [2000]);
});

for (const [initialUrl, configuredBase, expected] of [
  [`${baseUrl}/share/fixture?authuser=3`, `${baseUrl}/u/2`, `${baseUrl}/u/3/app`],
  ["about:blank", `${baseUrl}/u/2/`, `${baseUrl}/u/2/app`],
]) {
  test(`app navigation retains the selected account slot for ${initialUrl}`, browserTest, async (t) => {
    const { entry, page, state } = await appFixture(t, { initialUrl });
    await app.ensureAppPage(entry, configuredBase, 2000);
    assert.equal(page.url(), expected);
    assert.deepEqual(state.routes, [new URL(expected).pathname]);
  });
}

test("attached ready conversation avoids a hard reload only when reuse is requested", browserTest, async (t) => {
  const { entry, page, state } = await appFixture(t, { initialUrl: `${baseUrl}/app/fixture-conversation` });
  await page.locator("#draft").fill("retained fixture draft");
  await app.ensureAppPage(entry, baseUrl, 2000, { skipInitialNavigationWhenAppSurfaceReady: true });
  assert.deepEqual(state.routes, []);
  assert.equal(await page.locator("#draft").inputValue(), "retained fixture draft");
  await app.ensureAppPage(entry, baseUrl, 2000);
  assert.equal(page.url(), `${baseUrl}/app`);
  assert.deepEqual(state.routes, ["/app"]);
});

test("app navigation reports a real document redirect to the sign-in origin", browserTest, async (t) => {
  const { entry, page, state } = await appFixture(t, {
    initialUrl: `${baseUrl}/share/fixture`, redirect: "https://accounts.google.com/ServiceLogin", html: signedOutHtml,
  });
  await assert.rejects(app.ensureAppPage(entry, baseUrl, 2000), (error) => error.code === "gemini_canvas_auth_redirect" && error.status === 401);
  assert.equal(page.url(), "https://accounts.google.com/ServiceLogin");
  assert.deepEqual(state.routes, ["/app", "/ServiceLogin"]);
});

test("attached page discovery skips closed and signed-out documents and returns a surviving app", browserTest, async (t) => {
  const { entry, context, page } = await appFixture(t, { html: signedOutHtml });
  const closed = await context.newPage();
  await closed.close();
  const ready = await context.newPage();
  await ready.goto(`${baseUrl}/app/fixture`);
  await ready.setContent(readyHtml);
  entry.attachedCdp = true;
  assert.equal(await app.findAttachedGeminiAppPage(entry, baseUrl), ready);
  assert.notEqual(ready, page);
  entry.attachedCdp = false;
  assert.equal(await app.findAttachedGeminiAppPage(entry, baseUrl), null);
});
