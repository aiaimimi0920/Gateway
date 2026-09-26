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
