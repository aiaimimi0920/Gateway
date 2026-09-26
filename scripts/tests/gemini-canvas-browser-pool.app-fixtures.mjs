import assert from "node:assert/strict";
import { after, before } from "node:test";
import { chromium } from "playwright-core";
import { resolveExecutablePath } from "../gemini-canvas-browser-pool-executable.mjs";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

export const app = await importTestableScript();
export const baseUrl = "https://gemini.google.com";
export const readyHtml = '<main>New chat Talk to Gemini Sign in</main><textarea role="textbox" aria-label="Gemini" id="draft"></textarea>';
export const signedOutHtml = "<main>Sign in Meet Gemini, your personal AI assistant</main>";
const executablePath = resolveExecutablePath(process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH);
export const browserTest = { skip: executablePath ? false : "No local Chromium-compatible browser installed", timeout: 60_000 };
let browser;

before(async () => {
  if (executablePath) browser = await chromium.launch({ executablePath, headless: true });
});
after(async () => {
  if (!browser) return;
  const remaining = browser.contexts().length;
  await browser.close();
  assert.equal(browser.isConnected(), false);
  assert.equal(remaining, 0, "Every fixture must close its isolated browser context");
});

export async function appFixture(t, options = {}) {
  const context = await browser.newContext({ serviceWorkers: "block" });
  t.after(() => context.close());
  await context.setOffline(true);
  const state = { html: readyHtml, routes: [], waits: [], active: false, ...options };
  // Fulfill every page request locally; never contact a provider or account.
  await context.route("**/*", async (route) => {
    const request = route.request(), url = new URL(request.url());
    state.routes.push(url.pathname);
    if (state.active && state.redirect && url.pathname === "/app") {
      // HTTP redirects bypass subsequent route handlers; use a new document navigation.
      await route.fulfill({ contentType: "text/html", body: `<script>location.replace(${JSON.stringify(state.redirect)})</script>` });
      return;
    }
    const body = typeof state.html === "function" ? await state.html(request) : state.html;
    await route.fulfill({ status: 200, contentType: "text/html; charset=utf-8", body });
  });
  const page = await context.newPage();
  await page.goto(state.initialUrl ?? `${baseUrl}/app`);
  state.routes.length = 0;
  state.active = true;
  let now = Date.now();
  t.mock.method(Date, "now", () => now);
  t.mock.method(page, "waitForTimeout", async (ms) => {
    state.waits.push(ms);
    now += ms;
    await state.onWait?.(ms, page);
  });
  t.mock.method(console, "log", () => {});
  const entry = { context, page, runtimeStatePath: "synthetic-app-profile", runtimeStateMode: "storage_state_file" };
  return { context, page, entry, state };
}
