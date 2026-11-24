// Synthetic Playwright object only; the actual CLI executes without browser or platform access.
import { chromium, errors } from "playwright-core";
import process from "node:process";
const mode = process.env.LUMA_LIFECYCLE_FIXTURE;
if (mode === "early-exit" && process.argv[1]?.endsWith("lumalabs-browser-worker.mjs")) process.exit(23);
const mark = (event) => process.stderr.write(`fixture:${event}\n`);
const pause = () => new Promise((resolve) => setTimeout(resolve, 20));
const page = {
  goto: async () => {
    mark("goto");
    if (mode === "deadline") await pause();
    if (mode === "playwright-timeout") throw new errors.TimeoutError("synthetic-private timeout");
    if (mode === "goto-error") throw new Error("synthetic-private navigation");
  },
  url: () => mode === "auth" ? "https://auth.lumalabs.ai/sign-in?state=synthetic-private" :
    mode === "foreign-board" ? "https://foreign.fixture.invalid/board/realm-fixture" :
      mode === "query-board" ? "https://auth.lumalabs.ai/sign-in?return=/board/realm-fixture" :
        "https://app.lumalabs.ai/board/realm-fixture",
  evaluate: async () => {
    mark("generate");
    return mode === "operation-error" ? { ok: false, error: { code: "lumalabs_fixture_error", status: 429 } } :
      { ok: true, signedUrl: "https://asset.fixture.invalid/output.png", outputId: "output-1",
        actionId: null, resolvedActionType: "create_image_uni_1", realmId: "realm-fixture",
        mediaOperation: "image", artifactField: "image", state: "completed" };
  },
  close: async () => {},
};
const context = { addCookies: async () => {}, newPage: async () => page };
chromium.launch = async () => {
  mark("launch");
  return {
    newContext: async () => context,
    close: async () => {
      await pause(); mark("close");
      if (mode === "close-error") throw new Error("synthetic-private cleanup");
    },
  };
};
const write = process.stdout.write.bind(process.stdout);
process.stdout.write = (...args) => {
  mark("emit");
  if (process.argv[1]?.endsWith("lumalabs-browser-worker.mjs")) {
    if (mode === "nonzero") process.exitCode = 23;
    if (mode === "stdout-budget") write(" ".repeat(128 * 1024));
    if (mode === "stderr-budget") process.stderr.write("x".repeat(16 * 1024 + 1));
  }
  return write(...args);
};
