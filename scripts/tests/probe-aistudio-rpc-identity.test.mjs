import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import path from "node:path";
import test from "node:test";

const scriptPath = path.resolve(
  import.meta.dirname,
  "..",
  "probe-aistudio-live-request.mjs",
);

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
