import assert from "node:assert/strict";
import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";

export async function importTestableProgramHandle({ outDir = null, operation = "text" } = {}) {
  const root = path.resolve(import.meta.dirname, "..");
  const source = await fs.readFile(path.join(root, "probe-gemini-canvas-program-handle.mjs"), "utf8");
  const executionExtracted = source.includes("await executeProgramHandleProbe(");
  const metadataReused = !source.includes("function trimProgramRpcCaptureText(");
  const startMarker = metadataReused ? "const INTERESTING_PROGRAM_RPC_IDS =" : "const PROGRAM_RPC_CAPTURE_LIMIT =";
  const endMarker = executionExtracted ? "await executeProgramHandleProbe(" : "const context = await chromium.launchPersistentContext(";
  assert.equal(source.split(startMarker).length, 2);
  assert.equal(source.split(endMarker).length, 2);
  const start = source.indexOf(startMarker), end = source.indexOf(endMarker);
  assert.ok(end > start);
  const parserStart = source.indexOf("const APP_PATH_REGEX =");
  const imports = [...source.matchAll(/^import \{[^}]*\} from "(\.\/(?:gemini-canvas-program-handle-[^"]+|gemini-canvas-browser-pool-(?:invoke-merge|action|rpc-candidates|media-policy|proxy-discovery|invoke|capture-metadata))\.mjs)";$/gm)].map((m) => m[0]);
  const mediaExtracted = !source.includes("function inferMimeTypeFromUrl(");
  const contractsReused = !source.includes("function mergeActionContract(");
  const invokeReused = !source.includes("function buildCanvasProgramInvokeContract(");
  const pagesExtracted = !source.includes("async function collectSnapshot(");
  const networkCaptureExtracted = !source.includes("function startNetworkCapture(");
  assert.equal(imports.length, (parserStart < 0 ? 1 : 0) + (mediaExtracted ? 2 : 0) + (contractsReused ? 2 : 0) + (invokeReused ? 3 : 0) + (pagesExtracted ? 2 : 0) + (metadataReused ? 1 : 0) + (executionExtracted ? 1 : 0) + (networkCaptureExtracted ? 1 : 0));
  const prefix = parserStart < 0 ? "" : source.slice(parserStart, start);
  // Exclude executable/profile discovery, mkdir, launch and the entire live run.
  const declarations = prefix + source.slice(start, end);
  const names = [
    "normalizeString", "startNetworkCapture", "strongestHandle", "hasTransportHints", "textPreview",
    "readRpcIdFromUrl", "readSourcePathFromUrl", "classifyProgramRpcCapture",
    "trimProgramRpcCaptureText", "pushProgramRpcCapture", "extractRequestCookieHeader",
    "captureCookieHeaderFromContext", "classifyHandlePairSurface",
    "collectSnapshot", "waitForShareSurface", "hasPromptTextbox", "clickFirstVisible",
    "tryFollowShareEntryPoint", "clickNewChat", "clickOperationMode", "clickMediaActionButton",
    "trySelectMusicStyleCard", "submitPrompt",
    "invokeContractIndicatesConcreteProgress", "collectCanvasProxyContractTexts",
    "deriveCanvasProxyWsInvokeCandidate", "extractCanvasProxyWsUrlFromText", "extractCanvasProxyTargetDomainFromText",
    "mergeActionContract", "extractCanvasProgramActionContractFromText", "extractQuotedScalar",
    "extractNumberScalar", "extractDurationSecondsFromBodyText", "inferInvokeUiState",
    "deriveProgramRpcInvokeCandidate", "extractModelHintFromRpcText", "buildCanvasProgramInvokeContract",
    "inferMimeTypeFromUrl", "isBlobLikeUrl", "isAudioLikeMimeType", "isAudioLikeUrl", "isVideoLikeUrl",
    "pushUniqueMediaUrl", "scorePlayerReadyTargetCandidate", "summarizePlayerReadyTargetCandidate",
    "pushPlayerReadyTargetCandidate", "collectRpcBodyDownloadCandidates", "collectPlayerReadyTargetCandidates",
    "mergeInvokeContract",
    "uniqueStrings", "extractAppPathsFromText", "extractConversationIdsFromText",
    "extractResponseIdsFromText", "extractSharePathsFromText", "extractHandleHintsFromText",
    "sanitizeTransportHintUrl", "deriveInvokeBaseUrlFromRequestUrl", "deriveVideoInvokePathFromRequestUrl",
    "deriveMusicWsUrlFromRequestUrl", "extractTransportHintsFromUrl", "mergeTransportHints",
    "buildHandlePair", "normalizeHandleId", "extractHandlePairsFromText", "mergeHints",
    "dedupeHandlePairs", "mergeHandlePairs", "extractAppPath", "selectProgramConversationPair",
    "selectLatestResponsePair", "hasCanvasProxyProgramCandidate", "concreteAppPathFromUrl",
    "selectCanonicalProgramPair", "deriveShareId", "deriveConversationIdForAppPath",
  ];
  const temporary = path.join(root, ".gemini-canvas-program-handle-testable-" + process.pid + "-" + Date.now() + ".mjs");
  const nativePrefix = 'import { writeFileSync } from "node:fs";\nimport path from "node:path";\nconst outDir = ' + JSON.stringify(outDir) + ";\nconst operation = " + JSON.stringify(operation) + ";\n";
  await fs.writeFile(temporary, nativePrefix + imports.join("\n") + "\n" + declarations + "\nexport { " + names.join(", ") + " };\n", "utf8");
  try { return await import(pathToFileURL(temporary).href); }
  finally { await fs.rm(temporary, { force: true }); }
}
