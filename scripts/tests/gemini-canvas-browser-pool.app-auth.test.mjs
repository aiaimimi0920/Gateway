import assert from "node:assert/strict";
import test from "node:test";
import { app, appFixture, baseUrl, browserTest, readyHtml, signedOutHtml } from "./gemini-canvas-browser-pool.app-fixtures.mjs";

test("consent detection leaves a ready app document untouched", browserTest, async (t) => {
  const { page, state } = await appFixture(t);
  assert.equal(await app.tryResolveGoogleConsent(page, 2000), false);
  assert.deepEqual(state.waits, []);
  assert.match(await page.locator("body").innerText(), /Talk to Gemini/);
});

for (const tag of ["button", "input"]) {
  test(`consent rejection resolves using a real ${tag === "button" ? "locator click" : "DOM fallback"}`, browserTest, async (t) => {
    const click = "document.body.innerHTML = '<main>Talk to Gemini New chat</main>'";
    const control = tag === "button"
      ? `<button onclick="${click}">Reject all</button>`
      : `<input type="button" value="Reject all" onclick="${click}">`;
    const { page, state } = await appFixture(t, { html: `<main>Before you continue to Google</main>${control}` });
    assert.equal(await app.tryResolveGoogleConsent(page, 2000), true);
    assert.equal(await page.locator("body").innerText(), "Talk to Gemini New chat");
    assert.deepEqual(state.waits, [800]);
  });
}

test("unresolved consent produces the bounded diagnostic payload and conflict status", browserTest, async (t) => {
  const buttons = Array.from({ length: 130 }, (_, i) => `<button>Fixture option ${i}</button>`).join("");
  const { entry } = await appFixture(t, { html: `<main>Before you continue to Google</main>${buttons}` });
  await assert.rejects(app.ensureAppPage(entry, baseUrl, 2000), (error) => {
    assert.equal(error.code, "gemini_canvas_google_consent_unresolved");
    assert.equal(error.status, 409);
    const details = JSON.parse(error.bodyText);
    assert.equal(details.buttons.length, 120);
    assert.equal(details.runtimeStatePath, "synthetic-app-profile");
    assert.equal(details.launchClonedProfile, false);
    return true;
  });
});

test("cookie recovery hydrates an isolated browser context once and reloads into the ready app", browserTest, async (t) => {
  const { entry, context, state } = await appFixture(t, {
    html: (request) => request.headers().cookie?.includes("SID=fixture-cookie") ? readyHtml : signedOutHtml,
  });
  const addCookies = context.addCookies.bind(context);
  const sync = t.mock.method(context, "addCookies", addCookies);
  await app.ensureAppPage(entry, baseUrl, 2000, { cookieHeader: "SID=fixture-cookie" });
  assert.equal(sync.mock.callCount(), 1);
  assert.deepEqual(state.routes, ["/app"]);
  assert.equal((await context.cookies(baseUrl)).find((cookie) => cookie.name === "SID")?.value, "fixture-cookie");
});

test("an already-attempted cookie recovery does not repeat and auth grace stays bounded", browserTest, async (t) => {
  const { entry, context, state } = await appFixture(t, { html: signedOutHtml });
  const sync = t.mock.method(context, "addCookies", async () => { throw new Error("unexpected cookie retry"); });
  await assert.rejects(app.ensureAppPage(entry, baseUrl, 2000, {
    cookieHeader: "SID=fixture-cookie", cookieRehydrateAttempted: true,
  }), (error) => error.code === "gemini_canvas_auth_required" && error.status === 401);
  assert.equal(sync.mock.callCount(), 0);
  assert.deepEqual(state.routes, []);
  assert.deepEqual(state.waits, [2000, 1000, 1000]);
});

test("auth grace accepts a prompt-ready app that materializes after the initial inspection", browserTest, async (t) => {
  const { entry, state } = await appFixture(t, {
    html: signedOutHtml, onWait: async (ms, page) => { if (ms === 1000) await page.setContent(readyHtml); },
  });
  await app.ensureAppPage(entry, baseUrl, 2000);
  assert.deepEqual(state.waits, [2000, 1000]);
  assert.deepEqual(state.routes, []);
});

test("auth grace reports a sign-in redirect occurring after initial page inspection", browserTest, async (t) => {
  const { entry, state } = await appFixture(t, {
    html: signedOutHtml, onWait: async (ms, page) => { if (ms === 1000) await page.goto("https://accounts.google.com/ServiceLogin"); },
  });
  await assert.rejects(app.ensureAppPage(entry, baseUrl, 2000), (error) => error.code === "gemini_canvas_auth_redirect" && error.status === 401);
  assert.deepEqual(state.waits, [2000, 1000]);
});
