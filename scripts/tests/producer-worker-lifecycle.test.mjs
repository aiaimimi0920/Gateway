import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import test from "node:test";

const source = await readFile(new URL("../producer-browser-worker.mjs", import.meta.url), "utf8");
const stripped = source.replace(/^import[\s\S]*?;\r?\n/gm, "");
const inputUrl = new URL("../producer-browser/input.mjs", import.meta.url).href;
const fieldsUrl = new URL("../producer-browser/request-fields.mjs", import.meta.url).href;
const stdinUrl = new URL("../producer-browser/stdin.mjs", import.meta.url).href;

function runWorker(scenario, inputOverride = null) {
  const bootstrap = `
    const scenario = ${JSON.stringify(scenario)};
    const events = [];
    process.on("exit", () => process.stderr.write(JSON.stringify(events)));
    const write = process.stdout.write.bind(process.stdout);
    process.stdout.write = (...args) => { events.push("output"); return write(...args); };
    const page = {
      setDefaultNavigationTimeout() {}, waitForTimeout: async () => {},
      goto: async () => { if (scenario === "navigation-error") throw Error("fixture navigation"); },
      evaluate: async () => scenario === "empty-page" ? null : scenario === "page-error" ? { ok: false, error: { code: "fixture-page" } } : { ok: true, result: { fixture: true } },
    };
    const context = {
      pages: () => [page], addCookies: async () => {},
      close: async () => { await new Promise(setImmediate); events.push("context-close"); if (scenario === "close-error") throw Error("fixture close"); },
    };
    const browser = {
      newContext: async () => { if (scenario === "context-error") throw Error("fixture context"); return context; },
      close: async () => { await new Promise(setImmediate); events.push("browser-close"); },
    };
    const persistent = !["ephemeral", "context-error"].includes(scenario);
    const api = {
      ...await import(${JSON.stringify(inputUrl)}), ...await import(${JSON.stringify(fieldsUrl)}),
      ...await import(${JSON.stringify(stdinUrl)}),
      resolveExecutablePath: () => scenario === "missing-browser" ? null : "fixture-browser",
      resolveBrowserProfileSource: () => persistent ? { userDataDir: "/fixture-source", profileDirectory: "Default" } : null,
      cloneBrowserProfile: async () => { events.push("clone"); return "/fixture-owned-profile"; },
      chromium: {
        launchPersistentContext: async () => { events.push("launch"); if (scenario === "launch-error") throw Error("fixture launch"); return context; },
        launch: async () => { events.push("launch"); return browser; },
      },
      appendTrace: async () => {}, executeProducerPageVideoFlow: async () => page.evaluate(),
      executeProducerConversationVideoFlow: async () => ({ ok: true, result: { fixtureNode: true } }),
      rm: async (target) => { if (target !== "/fixture-owned-profile") throw Error("unexpected removal"); await new Promise(setImmediate); events.push("remove"); },
    };
    globalThis.__producerFixture = api;
    const prefix = "const {" + Object.keys(api).join(",") + "} = globalThis.__producerFixture;\\n";
    await import("data:text/javascript;base64," + Buffer.from(prefix + ${JSON.stringify(stripped)}).toString("base64"));
  `;
  const child = spawnSync(process.execPath, ["--input-type=module", "-e", bootstrap], {
    input: inputOverride ?? JSON.stringify({ baseUrl: "https://producer.invalid", authToken: "fixture-token", requestBody: scenario === "node-result" ? { clip_id: "fixture-clip" } : {} }),
    encoding: "utf8", timeout: 10000,
  });
  assert.equal(child.error, undefined);
  return { status: child.status, result: JSON.parse(child.stdout), events: JSON.parse(child.stderr) };
}

test("Producer oversized stdin is rejected before profile/browser allocation", () => {
  const { status, result, events } = runWorker("page-success", " ".repeat(16 * 1024 * 1024 + 1));
  assert.equal(status, 0);
  assert.equal(result.ok, false);
  assert.equal(result.error.status, 413);
  assert.equal(result.error.code, "producer_browser_input_too_large");
  assert.deepEqual(events, ["output"]);
});

test("Producer malformed JSON never echoes stdin credential text in its result", () => {
  const canary = "CANARY77";
  const { status, result, events } = runWorker("page-success", canary);
  assert.equal(status, 0);
  assert.equal(JSON.stringify(result).includes(canary), false);
  assert.equal(result.error.status, 400);
  assert.equal(result.error.code, "producer_browser_invalid_json");
  assert.equal(result.error.message, "Producer worker input must be valid JSON.");
  assert.deepEqual(events, ["output"]);
});

for (const scenario of ["page-success", "page-error", "empty-page", "node-result", "ephemeral", "context-error", "navigation-error", "close-error", "launch-error", "missing-browser"]) {
  test(`Producer ${scenario} outputs once after owned cleanup`, () => {
    const { status, result, events } = runWorker(scenario);
    assert.equal(status, 0, "preserve Producer structured-result exit convention");
    assert.equal(result.ok, ["page-success", "node-result", "ephemeral", "close-error"].includes(scenario));
    assert.equal(events.filter((entry) => entry === "output").length, 1);
    assert.equal(events.at(-1), "output");
    if (scenario === "missing-browser") assert.deepEqual(events, ["output"]);
    else if (scenario === "launch-error") assert.deepEqual(events, ["clone", "launch", "remove", "output"]);
    else if (scenario === "context-error") assert.deepEqual(events, ["launch", "browser-close", "output"]);
    else if (scenario === "ephemeral") assert.deepEqual(events, ["launch", "context-close", "browser-close", "output"]);
    else assert.deepEqual(events, ["clone", "launch", "context-close", "remove", "output"]);
  });
}
