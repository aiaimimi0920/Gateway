import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "probe-aistudio-live-request.mjs",
);

test("writes diagnostic artifacts when probe fails after capture directory is known", () => {
  const captureDir = mkdtempSync(path.join(tmpdir(), "aistudio-probe-error-"));

  try {
    const result = spawnSync(process.execPath, [scriptPath], {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      input: JSON.stringify({
        runtimeStateObjectKey: "credential-runtime/missing-for-error-summary-test",
        captureDir,
        browserExecutablePath: path.join(captureDir, "missing-browser.exe"),
      }),
      encoding: "utf8",
    });

    assert.equal(result.status, 1, result.stdout || result.stderr);

    const stdout = JSON.parse(result.stdout);
    assert.equal(stdout.ok, false);
    assert.equal(stdout.error.code, "aistudio_runtime_state_unavailable");
    assert.equal(stdout.captureDir, captureDir);
    assert.equal(
      stdout.targetRpcSummaryPath,
      path.join(captureDir, "target-rpc-summary.json"),
    );
    assert.equal(stdout.capturedTargetRpcContract, false);
    assert.equal(stdout.matchedCodeAssistantOfflineCount, 0);
    assert.equal(stdout.matchedStreamCodeAssistantOfflineGenerationCount, 0);

    const summary = JSON.parse(
      readFileSync(path.join(captureDir, "summary.json"), "utf8"),
    );
    assert.deepEqual(summary, stdout);

    const targetRpcSummary = JSON.parse(
      readFileSync(path.join(captureDir, "target-rpc-summary.json"), "utf8"),
    );
    assert.equal(targetRpcSummary.capturedTargetRpcContract, false);
    assert.equal(targetRpcSummary.targetRpcPairCount, 0);
    assert.equal(targetRpcSummary.codeAssistantOfflineCount, 0);
    assert.equal(targetRpcSummary.streamCodeAssistantOfflineGenerationCount, 0);

    const normalizedTargetRpcContract = JSON.parse(
      readFileSync(path.join(captureDir, "normalized-target-rpc-contract.json"), "utf8"),
    );
    assert.equal(normalizedTargetRpcContract.capturedTargetRpcContract, false);
  } finally {
    rmSync(captureDir, { recursive: true, force: true });
  }
});

test("writes diagnostic artifacts for validation failures when capture directory is provided", () => {
  const captureDir = mkdtempSync(path.join(tmpdir(), "aistudio-probe-validation-"));

  try {
    const result = spawnSync(process.execPath, [scriptPath], {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      input: JSON.stringify({
        captureDir,
        browserExecutablePath: path.join(captureDir, "missing-browser.exe"),
      }),
      encoding: "utf8",
    });

    assert.equal(result.status, 1, result.stdout || result.stderr);

    const stdout = JSON.parse(result.stdout);
    assert.equal(stdout.ok, false);
    assert.equal(stdout.error.message, "runtimeStateObjectKey is required.");
    assert.equal(stdout.captureDir, captureDir);
    assert.equal(stdout.capturedTargetRpcContract, false);

    const summary = JSON.parse(
      readFileSync(path.join(captureDir, "summary.json"), "utf8"),
    );
    assert.deepEqual(summary, stdout);

    const targetRpcSummary = JSON.parse(
      readFileSync(path.join(captureDir, "target-rpc-summary.json"), "utf8"),
    );
    assert.equal(targetRpcSummary.capturedTargetRpcContract, false);
  } finally {
    rmSync(captureDir, { recursive: true, force: true });
  }
});

test("writes capture artifact when browser launch fails after capture initialization", () => {
  const rootDir = mkdtempSync(path.join(tmpdir(), "aistudio-probe-capture-"));
  const captureDir = path.join(rootDir, "capture");
  const objectStoreDir = path.join(rootDir, "objects");
  const runtimeStateRelativePath = path.join(
    "credential-runtime",
    "aistudio",
    "capture-after-init",
    "storage-state.json",
  );
  const runtimeStateObjectKey = runtimeStateRelativePath.replaceAll(path.sep, "/");
  const runtimeStatePath = path.join(objectStoreDir, runtimeStateRelativePath);
  const fakeBrowserPath = path.join(rootDir, "not-a-browser.exe");

  try {
    mkdirSync(path.dirname(runtimeStatePath), { recursive: true });
    writeFileSync(
      runtimeStatePath,
      JSON.stringify({ cookies: [], origins: [] }),
      "utf8",
    );
    writeFileSync(fakeBrowserPath, "not a real browser", "utf8");

    const result = spawnSync(process.execPath, [scriptPath], {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR: objectStoreDir,
      },
      input: JSON.stringify({
        runtimeStateObjectKey,
        captureDir,
        browserExecutablePath: fakeBrowserPath,
      }),
      encoding: "utf8",
    });

    assert.equal(result.status, 1, result.stdout || result.stderr);

    const stdout = JSON.parse(result.stdout);
    assert.equal(stdout.ok, false);
    assert.equal(stdout.captureDir, captureDir);
    assert.match(stdout.error.message, /not-a-browser\.exe|browser/i);

    const capture = JSON.parse(
      readFileSync(path.join(captureDir, "capture.json"), "utf8"),
    );
    assert.equal(capture.runtimeStateObjectKey, runtimeStateObjectKey);
    assert.equal(capture.runtimeStateMode, "storage_state_file");
    assert.equal(capture.executablePath, fakeBrowserPath);
  } finally {
    rmSync(rootDir, { recursive: true, force: true });
  }
});

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

test("extracts AI Studio UI diagnostic signals from page snapshots", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { extractAistudioUiSignals } = module;

    assert.equal(typeof extractAistudioUiSignals, "function");

    const signals = extractAistudioUiSignals({
      bodyText: [
        "slimemetawallet@gmail.com",
        "PRO",
        "控制 API 费用",
        "create budget limit",
        "Gemini 3.5 Flash",
        "Canceled",
        "An internal error occurred.",
        "Retry",
        "OK, got it",
      ].join("\\n"),
      buttons: [
        { text: "Retry" },
        { text: "OK, got it" },
      ],
      textboxes: [
        { ariaLabel: "Enter a prompt to generate an app" },
      ],
    });

    assert.equal(signals.hasInternalError, true);
    assert.equal(signals.hasCanceledStatus, true);
    assert.equal(signals.hasRetryAction, true);
    assert.equal(signals.hasCookieBanner, true);
    assert.equal(signals.hasBuildPromptTextbox, true);
    assert.equal(signals.hasBudgetControlPrompt, true);
    assert.equal(signals.hasProBadge, true);
    assert.deepEqual(signals.modelLabels, ["Gemini 3.5 Flash"]);
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

test("probe summary includes final AI Studio UI diagnostic signals", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { buildProbeSummary } = module;

    const summary = buildProbeSummary({
      captureDir: "capture",
      capture: {
        requests: [{ url: "https://aistudio.google.com/" }],
        responses: [],
        websockets: [],
        finalUrl: "https://aistudio.google.com/apps/new",
        finalPage: {
          uiSignals: {
            hasInternalError: true,
            hasBudgetControlPrompt: true,
          },
        },
      },
      executablePath: "browser",
      runtimeState: { mode: "storage_state_file", absolutePath: "state.json" },
      appUrl: "https://aistudio.google.com/apps",
      failUnlessTargetRpcCaptured: false,
      targetRpcSummary: {
        capturedTargetRpcContract: false,
        targetRpcPairCount: 0,
        codeAssistantOfflineCount: 0,
        streamCodeAssistantOfflineGenerationCount: 0,
        pairs: [],
      },
      normalizedTargetRpcContract: null,
      targetRpcContractObjectKey: null,
      targetRpcContractMirrorPath: null,
      localProxyRequest: null,
    });

    assert.deepEqual(summary.pageUiSignals, {
      hasInternalError: true,
      hasBudgetControlPrompt: true,
    });
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

test("probe summary marks Google account chooser as auth recovery instead of ok capture", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { buildProbeSummary } = module;

    const summary = buildProbeSummary({
      captureDir: "capture",
      capture: {
        requests: [
          {
            url: "https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger",
          },
        ],
        responses: [],
        websockets: [],
        finalUrl:
          "https://accounts.google.com/v3/signin/accountchooser?continue=https%3A%2F%2Faistudio.google.com%2Fapps",
        finalPage: {
          uiSignals: {
            accountEmails: ["slimemetawallet@gmail.com"],
          },
        },
      },
      executablePath: "browser",
      runtimeState: { mode: "storage_state_file", absolutePath: "state.json" },
      appUrl: "https://aistudio.google.com/apps",
      failUnlessTargetRpcCaptured: false,
      targetRpcSummary: {
        capturedTargetRpcContract: false,
        targetRpcPairCount: 0,
        codeAssistantOfflineCount: 0,
        streamCodeAssistantOfflineGenerationCount: 0,
        pairs: [],
      },
      normalizedTargetRpcContract: null,
      targetRpcContractObjectKey: null,
      targetRpcContractMirrorPath: null,
      localProxyRequest: null,
    });

    assert.equal(summary.ok, false);
    assert.deepEqual(summary.authRecoveryState, {
      isAuthRecovery: true,
      kind: "google_account_chooser",
      finalUrlHost: "accounts.google.com",
      accountEmails: ["slimemetawallet@gmail.com"],
    });
    assert.match(summary.note, /Google auth recovery/);
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

test("probe summary surfaces local proxy error frames", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { buildProbeSummary } = module;

    const summary = buildProbeSummary({
      captureDir: "capture",
      capture: {
        requests: [{ url: "https://aistudio.google.com/" }],
        responses: [],
        websockets: [],
        localWebSocket: {
          connections: [
            {
              receivedEventTypes: ["error"],
              framesReceived: [
                JSON.stringify({
                  event_type: "error",
                  message: "Proxy browser error: Google API returned error: 403 PERMISSION_DENIED",
                  status: 403,
                  request_id: "req_1",
                }),
              ],
            },
          ],
        },
        finalUrl: "https://aistudio.google.com/apps/example",
      },
      executablePath: "browser",
      runtimeState: { mode: "storage_state_file", absolutePath: "state.json" },
      appUrl: "https://aistudio.google.com/apps/example",
      failUnlessTargetRpcCaptured: false,
      targetRpcSummary: {
        capturedTargetRpcContract: false,
        targetRpcPairCount: 0,
        codeAssistantOfflineCount: 0,
        streamCodeAssistantOfflineGenerationCount: 0,
        pairs: [],
      },
      normalizedTargetRpcContract: null,
      targetRpcContractObjectKey: null,
      targetRpcContractMirrorPath: null,
      localProxyRequest: { path: "/v1beta/models/gemini-3-flash-preview:generateContent" },
    });

    assert.deepEqual(summary.localProxyErrors, [
      {
        status: 403,
        requestId: "req_1",
        message: "Proxy browser error: Google API returned error: 403 PERMISSION_DENIED",
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

test("resolves explicit AI Studio browser proxy direct mode without inheriting system proxy", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { resolveBrowserProxySettings } = module;

    assert.equal(typeof resolveBrowserProxySettings, "function");

    const direct = resolveBrowserProxySettings({ browserProxyUrl: "direct" });
    assert.equal(direct.mode, "direct");
    assert.equal(direct.launchProxy, null);
    assert.deepEqual(direct.launchArgs, ["--no-proxy-server"]);

    const disabled = resolveBrowserProxySettings({ browserProxyUrl: "none" });
    assert.equal(disabled, null);
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

test("resolves explicit AI Studio browser proxy URL and bypass list", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { resolveBrowserProxySettings } = module;

    const proxy = resolveBrowserProxySettings({
      browserProxyUrl: "http://user:pass@127.0.0.1:7890",
      browserProxyBypass: "localhost,127.0.0.1",
    });

    assert.equal(proxy.mode, "proxy");
    assert.deepEqual(proxy.launchArgs, []);
    assert.deepEqual(proxy.launchProxy, {
      server: "http://127.0.0.1:7890",
      username: "user",
      password: "pass",
      bypass: "localhost,127.0.0.1",
    });
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

test("builds Chromium proxy launch options without proxy null in direct mode", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildBrowserProxyLaunchOptions,
      resolveBrowserProxySettings,
    } = module;

    assert.equal(typeof buildBrowserProxyLaunchOptions, "function");

    const options = buildBrowserProxyLaunchOptions(
      resolveBrowserProxySettings({ browserProxyUrl: "direct" }),
    );
    assert.deepEqual(options, { args: ["--no-proxy-server"] });
    assert.equal(Object.hasOwn(options, "proxy"), false);
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

test("detects local browser proxy endpoint for preflight diagnostics", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import net from "node:net";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      assertBrowserProxyReachable,
      resolveBrowserProxySettings,
    } = module;

    assert.equal(typeof assertBrowserProxyReachable, "function");

    const server = net.createServer((socket) => socket.destroy());
    await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
    const port = server.address().port;
    try {
      const result = await assertBrowserProxyReachable(
        resolveBrowserProxySettings({ browserProxyUrl: "http://127.0.0.1:" + port }),
        1000,
      );
      assert.equal(result.ok, true);
      assert.equal(result.host, "127.0.0.1");
      assert.equal(result.port, port);
    } finally {
      await new Promise((resolve) => server.close(resolve));
    }

    await assert.rejects(
      () =>
        assertBrowserProxyReachable(
          resolveBrowserProxySettings({ browserProxyUrl: "http://127.0.0.1:1" }),
          200,
        ),
      /Browser proxy endpoint is not reachable/,
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

test("writes browser proxy diagnostics when launch-time probe fails", () => {
  const captureDir = mkdtempSync(path.join(tmpdir(), "aistudio-probe-proxy-diag-"));
  const objectStoreDir = mkdtempSync(path.join(tmpdir(), "aistudio-probe-proxy-objects-"));
  const runtimeStateRelativePath = path.join(
    "credential-runtime",
    "aistudio",
    "proxy-diag",
    "storage-state.json",
  );
  const runtimeStateObjectKey = runtimeStateRelativePath.replaceAll(path.sep, "/");
  const runtimeStatePath = path.join(objectStoreDir, runtimeStateRelativePath);
  const fakeBrowserPath = path.join(captureDir, "missing-browser.exe");

  try {
    mkdirSync(path.dirname(runtimeStatePath), { recursive: true });
    writeFileSync(
      runtimeStatePath,
      JSON.stringify({ cookies: [], origins: [] }),
      "utf8",
    );
    writeFileSync(fakeBrowserPath, "not a real browser", "utf8");

    const result = spawnSync(process.execPath, [scriptPath], {
      cwd: path.resolve(import.meta.dirname, "..", "..", ".."),
      env: {
        ...process.env,
        AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR: objectStoreDir,
      },
      input: JSON.stringify({
        runtimeStateObjectKey,
        captureDir,
        browserExecutablePath: fakeBrowserPath,
        browserProxyUrl: "http://127.0.0.1:42344",
      }),
      encoding: "utf8",
    });

    assert.equal(result.status, 1, result.stdout || result.stderr);
    const stdout = JSON.parse(result.stdout);
    assert.equal(stdout.browserProxyMode, "proxy");
    assert.equal(stdout.browserProxyServer, "http://127.0.0.1:42344");
  } finally {
    rmSync(captureDir, { recursive: true, force: true });
    rmSync(objectStoreDir, { recursive: true, force: true });
  }
});

test("local WebSocket capture server close destroys open sockets", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import net from "node:net";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { createLocalWebSocketCaptureServer } = module;

    assert.equal(typeof createLocalWebSocketCaptureServer, "function");

    async function reservePort() {
      const server = net.createServer();
      await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
      const port = server.address().port;
      await new Promise((resolve) => server.close(resolve));
      return port;
    }

    const capture = {};
    const persistCapture = async () => {};
    const port = await reservePort();
    const server = await createLocalWebSocketCaptureServer(port, capture, persistCapture);
    const client = net.createConnection({ host: "127.0.0.1", port });
    await new Promise((resolve) => client.once("connect", resolve));

    const closed = await Promise.race([
      server.close().then(() => "closed"),
      new Promise((resolve) => setTimeout(() => resolve("timeout"), 1000)),
    ]);
    const clientClosed = await Promise.race([
      new Promise((resolve) => client.once("close", () => resolve("closed"))),
      new Promise((resolve) => setTimeout(() => resolve("timeout"), 1000)),
    ]);

    assert.equal(closed, "closed");
    assert.equal(clientClosed, "closed");
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

test("requires both AI Studio target RPCs before marking contract captured", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    assert.equal(typeof summarizeTargetRpcContracts, "function");
    assert.equal(typeof buildNormalizedTargetRpcContract, "function");

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";

    const codeOnlySummary = summarizeTargetRpcContracts({
      requests: [{ id: "code-request", method: "POST", url: codeAssistantUrl }],
      responses: [{ requestId: "code-request", url: codeAssistantUrl, status: 200 }],
    });
    assert.equal(codeOnlySummary.targetRpcPairCount, 1);
    assert.equal(codeOnlySummary.codeAssistantOfflineCount, 1);
    assert.equal(codeOnlySummary.streamCodeAssistantOfflineGenerationCount, 0);
    assert.equal(codeOnlySummary.capturedTargetRpcContract, false);
    assert.equal(
      buildNormalizedTargetRpcContract(codeOnlySummary).capturedTargetRpcContract,
      false,
    );

    const requestOnlySummary = summarizeTargetRpcContracts({
      requests: [
        { id: "code-request", method: "POST", url: codeAssistantUrl },
        { id: "stream-request", method: "POST", url: streamUrl },
      ],
      responses: [],
    });
    assert.equal(requestOnlySummary.codeAssistantOfflineCount, 1);
    assert.equal(requestOnlySummary.streamCodeAssistantOfflineGenerationCount, 1);
    assert.equal(requestOnlySummary.capturedTargetRpcContract, false);
    assert.equal(
      buildNormalizedTargetRpcContract(requestOnlySummary).capturedTargetRpcContract,
      false,
    );

    const bothSummary = summarizeTargetRpcContracts({
      requests: [
        { id: "code-request", method: "POST", url: codeAssistantUrl },
        { id: "stream-request", method: "POST", url: streamUrl },
      ],
      responses: [
        { requestId: "code-request", url: codeAssistantUrl, status: 200 },
        { requestId: "stream-request", url: streamUrl, status: 200 },
      ],
    });
    assert.equal(bothSummary.codeAssistantOfflineCount, 1);
    assert.equal(bothSummary.streamCodeAssistantOfflineGenerationCount, 1);
    assert.equal(bothSummary.capturedTargetRpcContract, true);
    assert.equal(
      buildNormalizedTargetRpcContract(bothSummary).capturedTargetRpcContract,
      true,
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

test("normalized target RPC contract prefers complete pairs over earlier partial pairs", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: "complete code request",
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: "complete stream request",
        },
      ],
      responses: [
        {
          requestId: "missing-code-request",
          url: codeAssistantUrl,
          status: 503,
          bodyPreview: "orphan code response",
        },
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: "[\\"generation-1\\",\\"opaque-token\\"]",
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    assert.equal(targetRpcSummary.capturedTargetRpcContract, true);

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(
      normalized.codeAssistantOffline.requestBodyPreview,
      "complete code request",
    );
    assert.equal(normalized.codeAssistantOffline.responseStatus, 200);
    assert.equal(
      normalized.streamCodeAssistantOfflineGeneration.requestBodyPreview,
      "complete stream request",
    );
    assert.equal(normalized.streamCodeAssistantOfflineGeneration.responseStatus, 200);
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

test("normalized target RPC contract preserves preferred headers case-insensitively", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const appId = "11111111-1111-4111-8111-111111111111";
    const generationId = "generation-1";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          headers: {
            "Content-Type": "application/json+protobuf",
            "Origin": "https://aistudio.google.com",
            "X-Goog-Api-Key": "AIzaCaptured",
            "X-Goog-AuthUser": "0",
          },
          postDataPreview: JSON.stringify([
            [[[[[null, "Reply with OK"]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            "models/gemini-3-flash-preview",
            null,
            null,
            null,
            appId
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          headers: {
            "Content-Type": "application/json+protobuf",
            "X-Browser-Validation": "validation-token",
            "Sec-Fetch-Site": "same-site",
          },
          postDataPreview: JSON.stringify([generationId, null, null, appId]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          headers: {
            "Content-Type": "application/json",
          },
          bodyPreview: JSON.stringify([generationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          headers: {
            "Content-Type": "text/plain; charset=utf-8",
          },
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.replayReadyTargetRpcContract, true);
    assert.equal(isReplayReadyTargetRpcContract(normalized), true);
    assert.deepEqual(normalized.codeAssistantOffline.requestHeaders, {
      "content-type": "application/json+protobuf",
      origin: "https://aistudio.google.com",
      "x-goog-api-key": "AIzaCaptured",
      "x-goog-authuser": "0",
    });
    assert.equal(
      normalized.streamCodeAssistantOfflineGeneration.requestHeaders["content-type"],
      "application/json+protobuf",
    );
    assert.equal(
      normalized.streamCodeAssistantOfflineGeneration.requestHeaders["x-browser-validation"],
      "validation-token",
    );
    assert.equal(
      normalized.streamCodeAssistantOfflineGeneration.requestHeaders["sec-fetch-site"],
      "same-site",
    );
    assert.equal(normalized.codeAssistantOffline.responseHeaders["content-type"], "application/json");
    assert.equal(
      normalized.streamCodeAssistantOfflineGeneration.responseHeaders["content-type"],
      "text/plain; charset=utf-8",
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

test("target RPC response attribution preserves repeated request order for identical URLs", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { createResponseRequestAttributor } = module;

    assert.equal(typeof createResponseRequestAttributor, "function");

    const url = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const firstRequest = { method: () => "POST", url: () => url };
    const secondRequest = { method: () => "POST", url: () => url };
    const attributor = createResponseRequestAttributor();

    attributor.trackRequest(firstRequest, "first-request-id");
    attributor.trackRequest(secondRequest, "second-request-id");

    assert.equal(
      attributor.resolveResponse({
        request: () => firstRequest,
        url: () => url,
      }),
      "first-request-id",
    );
    assert.equal(
      attributor.resolveResponse({
        request: () => secondRequest,
        url: () => url,
      }),
      "second-request-id",
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

test("capture request id factory is monotonic without capture array length", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { createCaptureRequestIdFactory } = module;

    assert.equal(typeof createCaptureRequestIdFactory, "function");

    const nextRequestId = createCaptureRequestIdFactory(() => 1234567890);
    const firstId = nextRequestId();
    const secondId = nextRequestId();

    assert.equal(firstId, "1234567890-1");
    assert.equal(secondId, "1234567890-2");
    assert.notEqual(firstId, secondId);
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

test("normalized target RPC contract prefers successful complete pairs over earlier rejected pairs", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const appId = "11111111-1111-4111-8111-111111111111";

    const failedRequestBody = JSON.stringify([
      [[[[[null, "Reply with OK"]], "user"]]],
      "failed-opaque",
      null,
      null,
      null,
      2,
      null,
      "models/gemini-3-flash-preview",
      null,
      null,
      null,
      appId
    ]);
    const successfulRequestBody = JSON.stringify([
      [[[[[null, "Reply with OK"]], "user"]]],
      "successful-opaque",
      null,
      null,
      null,
      2,
      null,
      "models/gemini-3-flash-preview",
      null,
      null,
      null,
      appId
    ]);

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request-failed",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: failedRequestBody,
        },
        {
          id: "code-request-successful",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: successfulRequestBody,
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: "[\\"generation-1\\",null,null,\\"" + appId + "\\"]",
        },
      ],
      responses: [
        {
          requestId: "code-request-failed",
          url: codeAssistantUrl,
          status: 403,
          bodyPreview: "[\\"forbidden\\"]",
        },
        {
          requestId: "code-request-successful",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: "[\\"generation-1\\"]",
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    assert.equal(targetRpcSummary.capturedTargetRpcContract, true);
    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.codeAssistantOffline.responseStatus, 200);
    assert.equal(normalized.codeAssistantOpaqueToken, "successful-opaque");
    assert.equal(normalized.replayReadyTargetRpcContract, true);
    assert.equal(isReplayReadyTargetRpcContract(normalized), true);
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

test("normalized target RPC contract extracts appId from request slots instead of generation UUIDs", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const generationId = "22222222-2222-4222-8222-222222222222";
    const appId = "11111111-1111-4111-8111-111111111111";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: JSON.stringify([
            [[[[[null, "Reply with OK"]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            "models/gemini-3-flash-preview"
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: JSON.stringify([generationId, null, null, appId]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: JSON.stringify([generationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.generationId, generationId);
    assert.equal(normalized.appId, appId);
    assert.notEqual(normalized.appId, generationId);
    assert.equal(normalized.replayReadyTargetRpcContract, true);
    assert.equal(isReplayReadyTargetRpcContract(normalized), true);
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

test("normalized target RPC contract rejects mismatched generation id chain", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const appId = "11111111-1111-4111-8111-111111111111";
    const codeAssistantGenerationId = "generation-from-code-response";
    const streamGenerationId = "generation-from-different-request";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: JSON.stringify([
            [[[[[null, "Reply with OK"]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            "models/gemini-3-flash-preview",
            null,
            null,
            null,
            appId
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: JSON.stringify([streamGenerationId, null, null, appId]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: JSON.stringify([codeAssistantGenerationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.generationId, null);
    assert.equal(normalized.replayReadyTargetRpcContract, false);
    assert.equal(isReplayReadyTargetRpcContract(normalized), false);
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

test("normalized target RPC contract rejects mismatched appId request slots", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const codeAssistantAppId = "11111111-1111-4111-8111-111111111111";
    const streamAppId = "55555555-5555-4555-8555-555555555555";
    const generationId = "generation-1";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: JSON.stringify([
            [[[[[null, "Reply with OK"]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            "models/gemini-3-flash-preview",
            null,
            null,
            null,
            codeAssistantAppId,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            codeAssistantAppId
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: JSON.stringify([generationId, null, null, streamAppId]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: JSON.stringify([generationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.appId, null);
    assert.equal(normalized.replayReadyTargetRpcContract, false);
    assert.equal(isReplayReadyTargetRpcContract(normalized), false);
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

test("normalized target RPC contract rejects mismatched CodeAssistant appId slots", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const codeAssistantAppId = "11111111-1111-4111-8111-111111111111";
    const otherCodeAssistantAppId = "66666666-6666-4666-8666-666666666666";
    const generationId = "generation-1";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: JSON.stringify([
            [[[[[null, "Reply with OK"]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            "models/gemini-3-flash-preview",
            null,
            null,
            null,
            codeAssistantAppId,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            null,
            otherCodeAssistantAppId
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: JSON.stringify([generationId, null, null, codeAssistantAppId]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: JSON.stringify([generationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.appId, null);
    assert.equal(normalized.replayReadyTargetRpcContract, false);
    assert.equal(isReplayReadyTargetRpcContract(normalized), false);
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

test("normalized target RPC contract extracts modelPath from request slot instead of prompt text", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const appId = "11111111-1111-4111-8111-111111111111";
    const generationId = "generation-1";
    const actualModelPath = "models/gemini-3-flash-preview";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: JSON.stringify([
            [[[[[null, "Please compare models/not-the-runtime-model with the default."]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            actualModelPath,
            null,
            null,
            null,
            appId
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: JSON.stringify([generationId, null, null, appId]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: JSON.stringify([generationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.modelPath, actualModelPath);
    assert.notEqual(normalized.modelPath, "models/not-the-runtime-model");
    assert.equal(normalized.replayReadyTargetRpcContract, true);
    assert.equal(isReplayReadyTargetRpcContract(normalized), true);
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

test("normalized target RPC contract does not infer modelPath from prompt or stream text", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const appId = "11111111-1111-4111-8111-111111111111";
    const generationId = "generation-1";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: JSON.stringify([
            [[[[[null, "Please compare models/not-the-runtime-model with the default."]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            null,
            null,
            null,
            null,
            appId
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: JSON.stringify([generationId, null, null, appId]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: JSON.stringify([generationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text mentions models/also-not-runtime\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.modelPath, null);
    assert.equal(normalized.replayReadyTargetRpcContract, true);
    assert.equal(isReplayReadyTargetRpcContract(normalized), true);
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

test("normalized target RPC contract does not treat prompt UUIDs as appId", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const promptUuid = "33333333-3333-4333-8333-333333333333";
    const generationId = "generation-1";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: JSON.stringify([
            [[[[[null, "Summarize id " + promptUuid]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            "models/gemini-3-flash-preview"
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: JSON.stringify([generationId, null, null, null]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: JSON.stringify([generationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final text\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.appId, null);
    assert.equal(normalized.replayReadyTargetRpcContract, false);
    assert.equal(isReplayReadyTargetRpcContract(normalized), false);
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

test("normalized target RPC contract does not treat stream response UUIDs as appId", () => {
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const streamResponseUuid = "44444444-4444-4444-8444-444444444444";
    const generationId = "generation-1";

    const targetRpcSummary = summarizeTargetRpcContracts({
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          postDataPreview: JSON.stringify([
            [[[[[null, "Summarize the response id."]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            "models/gemini-3-flash-preview"
          ]),
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          postDataPreview: JSON.stringify([generationId, null, null, null]),
        },
      ],
      responses: [
        {
          requestId: "code-request",
          url: codeAssistantUrl,
          status: 200,
          bodyPreview: JSON.stringify([generationId]),
        },
        {
          requestId: "stream-request",
          url: streamUrl,
          status: 200,
          bodyPreview: "[null,\\"final id " + streamResponseUuid + "\\"]",
        },
      ],
    });

    const normalized = buildNormalizedTargetRpcContract(targetRpcSummary);
    assert.equal(normalized.capturedTargetRpcContract, true);
    assert.equal(normalized.appId, null);
    assert.equal(normalized.replayReadyTargetRpcContract, false);
    assert.equal(isReplayReadyTargetRpcContract(normalized), false);
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

test("complete target RPC pairs are not validation-ok until replay-critical fields are present", () => {
  const captureDir = path.join(tmpdir(), "aistudio-probe-replay-ready");
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const {
      buildNormalizedTargetRpcContract,
      buildProbeSummary,
      isReplayReadyTargetRpcContract,
      summarizeTargetRpcContracts,
    } = module;

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const streamUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";
    const incompleteCapture = {
      finalUrl: "https://ai.studio/apps/example",
      requests: [
        {
          id: "code-request",
          method: "POST",
          url: codeAssistantUrl,
          headers: { "x-goog-api-key": "AIzaSyCaptured" },
          postDataPreview: "[[\\"Reply with OK\\"]]",
        },
        {
          id: "stream-request",
          method: "POST",
          url: streamUrl,
          headers: { "x-goog-api-key": "AIzaSyCaptured" },
          postDataPreview: "[\\"generation-1\\",null,null,null]",
        },
      ],
      responses: [
        { requestId: "code-request", url: codeAssistantUrl, status: 200, bodyPreview: "[\\"generation-1\\"]" },
        { requestId: "stream-request", url: streamUrl, status: 200, bodyPreview: "[null,\\"final text\\"]" },
      ],
      websockets: [],
      localWebSocket: { connections: [] },
    };

    const incompleteSummary = summarizeTargetRpcContracts(incompleteCapture);
    assert.equal(incompleteSummary.capturedTargetRpcContract, true);
    const incompleteContract = buildNormalizedTargetRpcContract(incompleteSummary);
    assert.equal(incompleteContract.capturedTargetRpcContract, true);
    assert.equal(incompleteContract.appId, null);
    assert.equal(incompleteContract.codeAssistantOpaqueToken, null);
    assert.equal(isReplayReadyTargetRpcContract(incompleteContract), false);

    const incompleteProbeSummary = buildProbeSummary({
      capture: incompleteCapture,
      captureDir: ${JSON.stringify(captureDir)},
      executablePath: "C:/browser.exe",
      runtimeState: {
        mode: "storage_state_file",
        absolutePath: "C:/runtime/storage-state.json",
      },
      appUrl: "https://ai.studio/apps/example",
      failUnlessTargetRpcCaptured: true,
      targetRpcSummary: incompleteSummary,
      normalizedTargetRpcContract: incompleteContract,
      targetRpcContractObjectKey: null,
      targetRpcContractMirrorPath: null,
      localProxyRequest: null,
    });
    assert.equal(incompleteProbeSummary.capturedTargetRpcContract, true);
    assert.equal(incompleteProbeSummary.replayReadyTargetRpcContract, false);
    assert.equal(incompleteProbeSummary.ok, false);

    const replayReadyCapture = {
      ...incompleteCapture,
      requests: [
        {
          ...incompleteCapture.requests[0],
          postDataPreview: JSON.stringify([
            [[[[[null, "Reply with OK"]], "user"]]],
            "opaque-token",
            null,
            null,
            null,
            2,
            null,
            "models/gemini-3-flash-preview",
            null,
            null,
            null,
            "11111111-1111-4111-8111-111111111111"
          ]),
        },
        {
          ...incompleteCapture.requests[1],
          postDataPreview: "[\\"generation-1\\",null,null,\\"11111111-1111-4111-8111-111111111111\\"]",
        },
      ],
    };
    const replayReadySummary = summarizeTargetRpcContracts(replayReadyCapture);
    const replayReadyContract = buildNormalizedTargetRpcContract(replayReadySummary);
    assert.equal(replayReadySummary.capturedTargetRpcContract, true);
    assert.equal(isReplayReadyTargetRpcContract(replayReadyContract), true);

    const replayReadyProbeSummary = buildProbeSummary({
      capture: replayReadyCapture,
      captureDir: ${JSON.stringify(captureDir)},
      executablePath: "C:/browser.exe",
      runtimeState: {
        mode: "storage_state_file",
        absolutePath: "C:/runtime/storage-state.json",
      },
      appUrl: "https://ai.studio/apps/example",
      failUnlessTargetRpcCaptured: true,
      targetRpcSummary: replayReadySummary,
      normalizedTargetRpcContract: replayReadyContract,
      targetRpcContractObjectKey: "credential-runtime/aistudio/demo/aistudio-target-rpc-contract.json",
      targetRpcContractMirrorPath: "C:/mirror/aistudio-target-rpc-contract.json",
      localProxyRequest: null,
    });
    assert.equal(replayReadyProbeSummary.capturedTargetRpcContract, true);
    assert.equal(replayReadyProbeSummary.replayReadyTargetRpcContract, true);
    assert.equal(replayReadyProbeSummary.ok, true);

    const rejectedCapture = {
      ...replayReadyCapture,
      responses: [
        {
          ...replayReadyCapture.responses[0],
          status: 403,
          bodyPreview: '[7,"The caller does not have permission"]',
        },
        { ...replayReadyCapture.responses[1], status: 200 },
      ],
    };
    const rejectedSummary = summarizeTargetRpcContracts(rejectedCapture);
    const rejectedContract = buildNormalizedTargetRpcContract(rejectedSummary);
    assert.equal(rejectedSummary.capturedTargetRpcContract, true);
    assert.equal(isReplayReadyTargetRpcContract(rejectedContract), false);
    const rejectedProbeSummary = buildProbeSummary({
      capture: rejectedCapture,
      captureDir: ${JSON.stringify(captureDir)},
      executablePath: "C:/browser.exe",
      runtimeState: {
        mode: "storage_state_file",
        absolutePath: "C:/runtime/storage-state.json",
      },
      appUrl: "https://ai.studio/apps/example",
      failUnlessTargetRpcCaptured: true,
      targetRpcSummary: rejectedSummary,
      normalizedTargetRpcContract: rejectedContract,
      targetRpcContractObjectKey: null,
      targetRpcContractMirrorPath: null,
      localProxyRequest: null,
    });
    assert.equal(rejectedProbeSummary.capturedTargetRpcContract, true);
    assert.equal(rejectedProbeSummary.replayReadyTargetRpcContract, false);
    assert.equal(rejectedProbeSummary.ok, false);
    assert.match(rejectedProbeSummary.note, /2xx|status/i);
    assert.equal(
      rejectedProbeSummary.targetRpcModelPath,
      "models/gemini-3-flash-preview",
    );
    assert.deepEqual(rejectedProbeSummary.targetRpcResponseStatuses, {
      codeAssistantOffline: 403,
      streamCodeAssistantOfflineGeneration: 200,
    });
    assert.deepEqual(rejectedProbeSummary.targetRpcFailure, {
      kind: "codeAssistantOffline",
      status: 403,
      bodyPreview: '[7,"The caller does not have permission"]',
    });
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

test("summary includes diagnostic artifact paths even before target contract is complete", () => {
  const captureDir = path.join(tmpdir(), "aistudio-probe-summary-paths");
  const childCode = `
    import assert from "node:assert/strict";
    import { pathToFileURL } from "node:url";

    const module = await import(pathToFileURL(${JSON.stringify(scriptPath)}).href);
    const { buildProbeSummary, summarizeTargetRpcContracts } = module;

    assert.equal(typeof buildProbeSummary, "function");
    assert.equal(typeof summarizeTargetRpcContracts, "function");

    const codeAssistantUrl = "https://clients6.google.com/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
    const capture = {
      finalUrl: "https://ai.studio/apps/example",
      requests: [{ id: "code-request", method: "POST", url: codeAssistantUrl }],
      responses: [{ requestId: "code-request", url: codeAssistantUrl, status: 200 }],
      websockets: [],
      localWebSocket: { connections: [] },
    };
    const targetRpcSummary = summarizeTargetRpcContracts(capture);
    const summary = buildProbeSummary({
      capture,
      captureDir: ${JSON.stringify(captureDir)},
      executablePath: "C:/browser.exe",
      runtimeState: {
        mode: "storage_state_file",
        absolutePath: "C:/runtime/storage-state.json",
      },
      appUrl: "https://ai.studio/apps/example",
      failUnlessTargetRpcCaptured: true,
      targetRpcSummary,
      targetRpcContractObjectKey: null,
      targetRpcContractMirrorPath: null,
      localProxyRequest: null,
    });

    assert.equal(summary.ok, false);
    assert.equal(summary.capturedTargetRpcContract, false);
    assert.equal(summary.matchedCodeAssistantOfflineCount, 1);
    assert.equal(summary.matchedStreamCodeAssistantOfflineGenerationCount, 0);
    assert.equal(
      summary.targetRpcSummaryPath,
      ${JSON.stringify(path.join(captureDir, "target-rpc-summary.json"))},
    );
    assert.equal(
      summary.normalizedTargetRpcContractPath,
      ${JSON.stringify(path.join(captureDir, "normalized-target-rpc-contract.json"))},
    );
    assert.equal(summary.targetRpcContractObjectKey, null);
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
