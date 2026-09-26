import { writeAtomicCapture as writeFile } from "./atomic-capture-file.mjs";
import path from "node:path";
import { collectPageSnapshot, collectFrameDiagnostics } from "./page-observation.mjs";
import { buildProbeSummary, extractAistudioUiSignals } from "./diagnostics.mjs";
import { buildNormalizedTargetRpcContract, isReplayReadyTargetRpcContract } from "./rpc-contract.mjs";
import { summarizeTargetRpcContracts } from "./rpc-attribution.mjs";
import { buildTargetRpcContractObjectKey, persistJsonObjectMirror } from "./runtime-storage.mjs";
import { printJsonAndSetExitCode } from "./cli-io.mjs";

async function publishProbeCapture({
  page, capture, captureDir, initialSnapshot, input, executablePath,
  runtimeState, appUrl, localProxyRequest, persistCapture,
}) {
  const finalSnapshot = await collectPageSnapshot(page, "final");
  const finalFrames = await collectFrameDiagnostics(page, "final");
  await writeFile(
    path.join(captureDir, "final-page.json"),
    `${JSON.stringify(finalSnapshot, null, 2)}\n`,
    "utf8",
  );
  await writeFile(
    path.join(captureDir, "final-frames.json"),
    `${JSON.stringify(finalFrames, null, 2)}\n`,
    "utf8",
  );
  await page.screenshot({
    path: path.join(captureDir, "final-page.png"),
    fullPage: true,
  });

  capture.finishedAt = new Date().toISOString();
  capture.finalUrl = page.url();
  capture.initialPage = {
    title: initialSnapshot.title,
    url: initialSnapshot.url,
    textboxes: initialSnapshot.textboxes.length,
    buttons: initialSnapshot.buttons.length,
    uiSignals: extractAistudioUiSignals(initialSnapshot),
  };
  capture.finalPage = {
    title: finalSnapshot.title,
    url: finalSnapshot.url,
    textboxes: finalSnapshot.textboxes.length,
    buttons: finalSnapshot.buttons.length,
    uiSignals: extractAistudioUiSignals(finalSnapshot),
  };

  const targetRpcSummary = summarizeTargetRpcContracts(capture);
  const normalizedTargetRpcContract =
    buildNormalizedTargetRpcContract(targetRpcSummary);
  const replayReadyTargetRpcContract = isReplayReadyTargetRpcContract(
    normalizedTargetRpcContract,
  );
  const targetRpcContractObjectKey = replayReadyTargetRpcContract
    ? buildTargetRpcContractObjectKey(input.runtimeStateObjectKey)
    : null;
  capture.targetRpcSummary = {
    capturedTargetRpcContract: targetRpcSummary.capturedTargetRpcContract,
    replayReadyTargetRpcContract,
    targetRpcPairCount: targetRpcSummary.targetRpcPairCount,
    codeAssistantOfflineCount: targetRpcSummary.codeAssistantOfflineCount,
    streamCodeAssistantOfflineGenerationCount:
      targetRpcSummary.streamCodeAssistantOfflineGenerationCount,
    targetRpcContractObjectKey,
  };

  await writeFile(
    path.join(captureDir, "target-rpc-summary.json"),
    `${JSON.stringify(targetRpcSummary, null, 2)}\n`,
    "utf8",
  );
  await writeFile(
    path.join(captureDir, "normalized-target-rpc-contract.json"),
    `${JSON.stringify(normalizedTargetRpcContract, null, 2)}\n`,
    "utf8",
  );
  let targetRpcContractMirrorPath = null;
  if (replayReadyTargetRpcContract && targetRpcContractObjectKey) {
    targetRpcContractMirrorPath = await persistJsonObjectMirror(
      targetRpcContractObjectKey,
      normalizedTargetRpcContract,
    );
  }
  for (const [index, pair] of targetRpcSummary.pairs.entries()) {
    const prefix = `${String(index + 1).padStart(2, "0")}-${pair.kind}`;
    await writeFile(
      path.join(captureDir, `${prefix}.json`),
      `${JSON.stringify(pair, null, 2)}\n`,
      "utf8",
    );
  }

  await persistCapture();

  const failUnlessTargetRpcCaptured = input?.failUnlessTargetRpcCaptured === true;
  const summary = buildProbeSummary({
    captureDir,
    capture,
    executablePath,
    runtimeState,
    appUrl,
    failUnlessTargetRpcCaptured,
    targetRpcSummary,
    normalizedTargetRpcContract,
    targetRpcContractObjectKey,
    targetRpcContractMirrorPath,
    localProxyRequest,
  });
  await writeFile(
    path.join(captureDir, "summary.json"),
    `${JSON.stringify(summary, null, 2)}\n`,
    "utf8",
  );
  printJsonAndSetExitCode(summary, summary.ok ? 0 : 2);
}

async function publishProbeFailure({ error, capture, captureDir }) {
  const message = error instanceof Error ? error.message : String(error);
  const status = Number(error?.status || error?.statusCode || 500);
  const code = error?.code || "aistudio_live_probe_failed";
  if (capture) {
    capture.failedAt = new Date().toISOString();
    capture.failure = {
      code,
      message,
      status,
    };
  }
  const targetRpcSummary = summarizeTargetRpcContracts(capture ?? {});
  const normalizedTargetRpcContract =
    buildNormalizedTargetRpcContract(targetRpcSummary);
  const replayReadyTargetRpcContract = isReplayReadyTargetRpcContract(
    normalizedTargetRpcContract,
  );
  const summary = {
    ok: false,
    captureDir,
    error: {
      code,
      message,
      status,
    },
    capturedTargetRpcContract: targetRpcSummary.capturedTargetRpcContract,
    replayReadyTargetRpcContract,
    browserProxyMode: capture?.browserProxyMode ?? null,
    browserProxyServer: capture?.browserProxyServer ?? null,
    browserProxyPreflight: capture?.browserProxyPreflight ?? null,
    matchedCodeAssistantOfflineCount:
      targetRpcSummary.codeAssistantOfflineCount,
    matchedStreamCodeAssistantOfflineGenerationCount:
      targetRpcSummary.streamCodeAssistantOfflineGenerationCount,
    targetRpcSummaryPath: captureDir
      ? path.join(captureDir, "target-rpc-summary.json")
      : null,
    normalizedTargetRpcContractPath: captureDir
      ? path.join(captureDir, "normalized-target-rpc-contract.json")
      : null,
  };
  if (captureDir) {
    if (capture) {
      await writeFile(
        path.join(captureDir, "capture.json"),
        `${JSON.stringify(capture, null, 2)}\n`,
        "utf8",
      ).catch(() => {});
    }
    await writeFile(
      path.join(captureDir, "target-rpc-summary.json"),
      `${JSON.stringify(targetRpcSummary, null, 2)}\n`,
      "utf8",
    ).catch(() => {});
    await writeFile(
      path.join(captureDir, "normalized-target-rpc-contract.json"),
      `${JSON.stringify(normalizedTargetRpcContract, null, 2)}\n`,
      "utf8",
    ).catch(() => {});
    await writeFile(
      path.join(captureDir, "summary.json"),
      `${JSON.stringify(summary, null, 2)}\n`,
      "utf8",
    ).catch(() => {});
  }
  printJsonAndSetExitCode(summary, 1);
}

export { publishProbeCapture, publishProbeFailure };
