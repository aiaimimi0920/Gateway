import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import test from "node:test";

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "probe-aistudio-live-request.mjs",
);

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
