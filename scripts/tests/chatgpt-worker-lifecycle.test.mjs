import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import test from "node:test";

const entryUrl = new URL("../chatgpt-web-session-worker.mjs", import.meta.url);
const source = await readFile(entryUrl, "utf8");
const stripped = source.replace(/^import[\s\S]*?;\r?\n/gm, "");
const configuration = new URL("../chatgpt-web-session/configuration.mjs", import.meta.url).href;
const errors = new URL("../chatgpt-web-session/errors.mjs", import.meta.url).href;

function runWorker(scenario, input = "{}") {
  const bootstrap = `
    const scenario = ${JSON.stringify(scenario)};
    const events = [];
    process.on("exit", () => process.stderr.write(JSON.stringify(events)));
    const write = process.stdout.write.bind(process.stdout);
    process.stdout.write = (...args) => { events.push("output"); return write(...args); };
    const configuration = await import(${JSON.stringify(configuration)});
    const errors = await import(${JSON.stringify(errors)});
    const page = {
      setExtraHTTPHeaders: async () => {}, content: async () => "fixture",
      evaluate: async () => "fixture-agent", url: () => "https://chatgpt.com/",
    };
    const browser = { close: async () => { await Promise.resolve(); events.push("browser-close"); } };
    const context = {
      browser: () => browser, pages: () => [page],
      close: async () => { await Promise.resolve(); events.push("context-close"); if (scenario === "close-error") throw Error("fixture close"); },
    };
    const api = {
      ...configuration, ...errors,
      chromium: { launchPersistentContext: async () => { events.push("launch"); if (scenario === "launch-error") throw Error("fixture launch"); return context; } },
      resolveExecutablePath: () => scenario === "missing-browser" ? null : "fixture-browser",
      resolveMailboxContext: () => ({}),
      resolveProfileSource: () => ({ mode: "fresh", profileDirectory: "Default", browser: "fixture" }),
      createFreshBrowserProfile: async () => { events.push("profile"); return "/fixture-owned-profile"; },
      primeBrowserWithImportedCookies: async () => {},
      navigateWithChallengeSettle: async () => { if (scenario === "navigate-error") throw Error("fixture navigation"); },
      runChatGptBrowserProbe: async () => ({ ok: true, status: 200, sessionAccessToken: "synthetic" }),
      collectCookieHeader: async () => "session=synthetic",
      extractBootstrapArtifacts: () => ({ powSources: [] }),
      findCookieValue: () => null, decodeJwtExpIso: () => null,
      safePageUrl: () => "https://chatgpt.com/", detectBrowserChallenge: () => false,
      DEFAULT_POW_SCRIPT: "fixture-script",
      maybeWriteCredentialFile: async () => { events.push("persist"); if (scenario === "persist-error") throw Error("fixture persist"); return null; },
      rm: async (target) => { if (target !== "/fixture-owned-profile") throw Error("unexpected removal"); events.push("remove"); },
    };
    globalThis.__workerFixture = api;
    const prefix = "const {" + Object.keys(api).join(",") + "} = globalThis.__workerFixture;\\n";
    await import("data:text/javascript;base64," + Buffer.from(prefix + ${JSON.stringify(stripped)}).toString("base64"));
  `;
  const child = spawnSync(process.execPath, ["--input-type=module", "-e", bootstrap], {
    input, encoding: "utf8", timeout: 10_000,
  });
  assert.equal(child.error, undefined);
  return { status: child.status, payload: JSON.parse(child.stdout), events: JSON.parse(child.stderr) };
}

for (const scenario of ["success", "close-error", "navigate-error", "persist-error", "launch-error", "missing-browser"]) {
  test(`worker ${scenario} emits its only result after owned cleanup`, () => {
    const result = runWorker(scenario);
    const success = scenario === "success" || scenario === "close-error";
    assert.equal(result.status, success ? 0 : 1);
    assert.equal(result.payload.ok, success);
    assert.equal(result.events.filter((event) => event === "output").length, 1);
    assert.equal(result.events.at(-1), "output");
    if (scenario === "missing-browser") assert.deepEqual(result.events, ["output"]);
    else if (scenario === "launch-error") assert.deepEqual(result.events, ["profile", "launch", "remove", "output"]);
    else assert.deepEqual(result.events.slice(-4), ["context-close", "browser-close", "remove", "output"]);
  });
}

test("oversized input exits with structured error before browser/profile allocation", () => {
  const result = runWorker("success", " ".repeat(16 * 1024 * 1024 + 1));
  assert.equal(result.status, 1);
  assert.equal(result.payload.ok, false);
  assert.equal(result.payload.error.code, "chatgpt_web_input_too_large");
  assert.deepEqual(result.events, ["output"]);
});
