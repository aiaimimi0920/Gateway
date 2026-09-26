import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import path from "node:path";
import test from "node:test";

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "probe-aistudio-live-request.mjs",
);

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
