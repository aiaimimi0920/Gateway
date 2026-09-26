import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import test from "node:test";

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "probe-aistudio-live-request.mjs",
);

test("applies AI Studio remix modal before prompting", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { bestEffortApplyRemixModal } = module;

    assert.equal(typeof bestEffortApplyRemixModal, "function");

    const calls = [];
    const remixHeader = {
      first() {
        return this;
      },
      async isVisible() {
        calls.push("remix-header-visible");
        return true;
      },
    };
    const applyButton = {
      first() {
        return this;
      },
      async isVisible() {
        calls.push("apply-visible");
        return true;
      },
      async click() {
        calls.push("apply-click");
      },
    };
    const page = {
      getByText(pattern) {
        assert.match("Remix AIStudioToAPI-V1.2.2", pattern);
        return remixHeader;
      },
      getByRole(role, options) {
        assert.equal(role, "button");
        assert.match("Apply", options.name);
        return applyButton;
      },
      async waitForTimeout(ms) {
        calls.push("wait:" + ms);
      },
    };
    const capture = { autoActions: [] };

    const applied = await bestEffortApplyRemixModal(page, capture);

    assert.equal(applied, true);
    assert.deepEqual(calls, [
      "remix-header-visible",
      "apply-visible",
      "apply-click",
      "wait:2000",
    ]);
    assert.deepEqual(capture.autoActions, [
      { action: "apply-remix-modal", ok: true },
    ]);
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});

test("dismisses AI Studio non-destructive overlays before prompting", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { bestEffortDismissAistudioOverlays } = module;

    assert.equal(typeof bestEffortDismissAistudioOverlays, "function");

    const calls = [];
    const makeLocator = (label, visible = false) => ({
      first() {
        return this;
      },
      async isVisible() {
        calls.push(label + ":visible");
        return visible;
      },
      async click() {
        calls.push(label + ":click");
      },
    });
    const locators = new Map([
      ["OK, got it", makeLocator("cookie-ok", true)],
      ["Skip", makeLocator("skip", true)],
      ["Create budget", makeLocator("create-budget", true)],
    ]);
    const page = {
      getByRole(role, options) {
        assert.equal(role, "button");
        const name = options.name;
        if (name.test("OK, got it")) return locators.get("OK, got it");
        if (name.test("Skip")) return locators.get("Skip");
        if (name.test("Create budget")) return locators.get("Create budget");
        return makeLocator("missing:" + String(name), false);
      },
      locator() {
        return makeLocator("backdrop", false);
      },
      keyboard: {
        async press(key) {
          calls.push("key:" + key);
        },
      },
      async waitForTimeout(ms) {
        calls.push("wait:" + ms);
      },
    };
    const capture = { autoActions: [] };

    const dismissed = await bestEffortDismissAistudioOverlays(page, capture);

    assert.equal(dismissed, true);
    assert.ok(calls.includes("cookie-ok:click"));
    assert.ok(calls.includes("skip:click"));
    assert.ok(!calls.includes("create-budget:click"));
    assert.deepEqual(
      capture.autoActions.filter((entry) => entry.ok).map((entry) => entry.action),
      ["dismiss-cookie-ok", "dismiss-onboarding-skip"],
    );
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});

test("selects Google account chooser row only for an explicit email", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { bestEffortSelectGoogleAccount } = module;

    assert.equal(typeof bestEffortSelectGoogleAccount, "function");

    const calls = [];
    const locator = {
      first() {
        return this;
      },
      async isVisible() {
        calls.push("visible");
        return true;
      },
      async click() {
        calls.push("click");
      },
    };
    const page = {
      url() {
        return "https://accounts.google.com/v3/signin/accountchooser";
      },
      getByText(text, options) {
        calls.push("getByText:" + text + ":" + Boolean(options?.exact));
        return locator;
      },
      async waitForTimeout(ms) {
        calls.push("wait:" + ms);
      },
    };
    const capture = { autoActions: [] };

    const selected = await bestEffortSelectGoogleAccount(
      page,
      capture,
      "slimemetawallet@gmail.com",
    );

    assert.equal(selected, true);
    assert.deepEqual(calls, [
      "getByText:slimemetawallet@gmail.com:true",
      "visible",
      "click",
      "wait:3000",
    ]);
    assert.deepEqual(capture.autoActions, [
      {
        action: "select-google-account",
        ok: true,
        email: "slimemetawallet@gmail.com",
      },
    ]);
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});

test("auto prompt retries until the AI Studio prompt textbox becomes visible", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { bestEffortAutoPrompt } = module;

    assert.equal(typeof bestEffortAutoPrompt, "function");

    const calls = [];
    let promptVisibilityChecks = 0;
    const hiddenLocator = {
      first() { return this; },
      async isVisible() { return false; },
      async click() { throw new Error("hidden click"); },
      async fill() { throw new Error("hidden fill"); },
    };
    const promptLocator = {
      first() { return this; },
      async isVisible() {
        promptVisibilityChecks += 1;
        calls.push("prompt-visible:" + promptVisibilityChecks);
        return promptVisibilityChecks >= 2;
      },
      async click() { calls.push("prompt-click"); },
      async fill(value) { calls.push("prompt-fill:" + value); },
    };
    const buildLocator = {
      first() { return this; },
      async isVisible() { calls.push("build-visible"); return true; },
      async click() { calls.push("build-click"); },
    };
    const page = {
      locator(selector) {
        if (selector === 'textarea[placeholder*="Make changes"]') return promptLocator;
        if (selector === "button:has-text('Build')") return buildLocator;
        return hiddenLocator;
      },
      getByRole(role, options) {
        assert.equal(role, "button");
        assert.match(String(options.name), /Build/);
        return buildLocator;
      },
      keyboard: {
        async press(key) { calls.push("key:" + key); },
      },
      async waitForTimeout(ms) { calls.push("wait:" + ms); },
    };
    const capture = { autoActions: [] };

    const prompted = await bestEffortAutoPrompt(
      page,
      "Reply with exactly OK.",
      capture,
      { maxAttempts: 2, pollMs: 10 },
    );

    assert.equal(prompted, true);
    assert.deepEqual(capture.autoActions, [
      {
        action: "auto-prompt",
        ok: true,
        promptPreview: "Reply with exactly OK.",
        submission: "build-button-role",
        attempts: 2,
      },
    ]);
    assert.ok(calls.includes("wait:10"));
    assert.ok(calls.includes("prompt-fill:Reply with exactly OK."));
    assert.ok(calls.includes("build-click"));
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});

test("auto prompt polling retry gate requires prompt text, no submission, and retry interval", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { shouldRetryAutoPromptDuringPolling } = module;

    assert.equal(typeof shouldRetryAutoPromptDuringPolling, "function");
    assert.equal(
      shouldRetryAutoPromptDuringPolling({
        normalizedAutoPrompt: "Build an app",
        autoPromptSubmitted: false,
        nowMs: 10_000,
        lastAutoPromptAttemptAtMs: 4_999,
      }),
      true,
    );
    assert.equal(
      shouldRetryAutoPromptDuringPolling({
        normalizedAutoPrompt: "Build an app",
        autoPromptSubmitted: false,
        nowMs: 10_000,
        lastAutoPromptAttemptAtMs: 5_001,
      }),
      false,
    );
    assert.equal(
      shouldRetryAutoPromptDuringPolling({
        normalizedAutoPrompt: "",
        autoPromptSubmitted: false,
        nowMs: 10_000,
        lastAutoPromptAttemptAtMs: 0,
      }),
      false,
    );
    assert.equal(
      shouldRetryAutoPromptDuringPolling({
        normalizedAutoPrompt: "Build an app",
        autoPromptSubmitted: true,
        nowMs: 10_000,
        lastAutoPromptAttemptAtMs: 0,
      }),
      false,
    );
  `;

  const result = spawnSync(
    process.execPath,
    ["--input-type=module", "-e", childCode],
    {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AISTUDIO_PROBE_SUPPRESS_MAIN: "1",
      },
      encoding: "utf8",
    },
  );

  assert.equal(result.status, 0, result.stdout || result.stderr);
});
