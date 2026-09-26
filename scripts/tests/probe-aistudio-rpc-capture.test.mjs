import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import test from "node:test";

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "probe-aistudio-live-request.mjs",
);

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
