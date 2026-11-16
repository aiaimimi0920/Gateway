// Explicit subprocess preload: no platform, credentials, or actual browser is contacted.
import { chromium } from "playwright-core";
import process from "node:process";
import { registerHooks } from "node:module";

registerHooks({
  resolve(specifier, context, nextResolve) {
    if (context.parentURL?.endsWith("/suno-browser/responses.mjs") &&
        specifier.endsWith("/gemini-canvas-program-handle-response-capture.mjs")) {
      return { url: new URL("./suno-native-capture-fixture.mjs", import.meta.url).href, shortCircuit: true };
    }
    return nextResolve(specifier, context);
  },
});

const mode = process.env.SUNO_LIFECYCLE_FIXTURE;
const mark = (value) => process.stderr.write(`fixture:${value}\n`);
const pause = () => new Promise((resolve) => setTimeout(resolve, 15));
let prompt, routeHandler, submitted;
const submission = new Promise(resolve => { submitted = resolve; });
const locator = {
  first() { return this; }, last() { return this; },
  count: async () => 1, waitFor: async () => {}, fill: async value => { prompt = value; },
  isEnabled: async () => true,
  click: async () => {
    if (mode === "click-error") throw new Error("private fixture click failure");
    await routeHandler({ request: () => ({ method: () => "POST", postData: () => JSON.stringify({
      mv: "page-default", generation_type: "TEXT", prompt, make_instrumental: false, metadata: { create_mode: "simple" },
    }) }), continue: async () => submitted(), abort: async () => {}, fallback: async () => {} });
  },
  innerText: async () => "Create song",
};
const page = {
  url: () => "https://suno.fixture.invalid/create",
  title: async () => mode === "challenge" ? "Just a moment" : "Create",
  evaluate: async () => false, keyboard: { press: async () => {} },
  waitForTimeout: async () => {}, waitForLoadState: async () => {},
  waitForFunction: async () => {}, goto: async () => {}, locator: () => locator,
  route: async (_, handler) => { routeHandler = handler; },
  unroute: async (_, handler) => { if (routeHandler === handler) routeHandler = null; },
  waitForResponse: async () => { await submission; return ({
    status: () => 200,
    text: async () => JSON.stringify({ clips: [{ id: "fixture", status: "complete", audio_url: "https://asset.invalid/audio" }] }),
  }); },
};
const context = { pages: () => [page], newPage: async () => page, addCookies: async () => {} };
const browser = {
  contexts: () => [context], newContext: async () => context,
  close: async () => {
    await pause(); mark("close");
    if (mode === "close-error") throw new Error("private fixture close failure");
  },
};
chromium.launch = async () => browser;
chromium.connectOverCDP = async () => {
  if (mode === "connect-error") throw new Error("private fixture connect failure");
  return browser;
};
globalThis.fetch = async (url) => {
  if (String(url).endsWith("/release")) {
    await pause(); mark("release");
    if (mode === "release-error") return { ok: false, status: 503 };
    return { ok: true, status: 200 };
  }
  return {
    ok: true, status: 200,
    text: async () => JSON.stringify({ data: { session: {
      session_id: "fixture", attach: { endpoint: "ws://fixture.invalid", page_url: page.url() },
    } } }),
  };
};
const write = process.stdout.write.bind(process.stdout);
process.stdout.write = (...args) => { mark("emit"); return write(...args); };
