import path from "node:path";
import { normalizeString, previewText } from "./input-text.mjs";
import { buildNormalizedTargetRpcContract, isReplayReadyTargetRpcContract, isSuccessfulTargetRpcStatus } from "./rpc-contract.mjs";

function extractAistudioUiSignals(snapshot) {
  const bodyText = String(snapshot?.bodyText ?? "");
  const buttonText = (Array.isArray(snapshot?.buttons) ? snapshot.buttons : [])
    .flatMap((entry) => [entry?.text, entry?.ariaLabel, entry?.title])
    .filter((entry) => typeof entry === "string")
    .join("\n");
  const textboxText = (Array.isArray(snapshot?.textboxes) ? snapshot.textboxes : [])
    .flatMap((entry) => [entry?.placeholder, entry?.ariaLabel, entry?.role, entry?.tag])
    .filter((entry) => typeof entry === "string")
    .join("\n");
  const combinedText = `${bodyText}\n${buttonText}\n${textboxText}`;
  const modelLabels = Array.from(
    new Set(
      combinedText
        .split(/\r?\n/)
        .map((line) => line.match(/\bGemini\s+\d+(?:\.\d+)?(?:\s+[A-Za-z0-9.-]+){1,3}\b/)?.[0])
        .filter(Boolean),
    ),
  ).slice(0, 12);

  return {
    hasInternalError:
      /An internal error occurred|internal error|内部错误|发生内部错误/i.test(combinedText),
    hasCanceledStatus: /\bCanceled\b|已取消|已取消请求/i.test(combinedText),
    hasRetryAction: /\bRetry\b|重试/.test(combinedText),
    hasCookieBanner: /uses cookies|OK,\s*got it|Cookie|cookies from Google/i.test(combinedText),
    hasBuildPromptTextbox:
      /Enter a prompt to generate an app|Describe an app|Make changes/i.test(textboxText),
    hasBudgetControlPrompt:
      /控制\s*API\s*费用|create budget limit|budget limit|API cost|API 费用/i.test(
        combinedText,
      ),
    hasProBadge: /(^|\n)\s*PRO\s*(\n|$)/.test(bodyText),
    accountEmails: Array.from(
      new Set(combinedText.match(/[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}/gi) ?? []),
    ).slice(0, 8),
    modelLabels,
  };
}

function classifyAistudioAuthRecovery(capture) {
  const finalUrl = normalizeString(capture?.finalUrl);
  if (!finalUrl) {
    return {
      isAuthRecovery: false,
      kind: null,
      finalUrlHost: null,
      accountEmails: [],
    };
  }

  let parsed = null;
  try {
    parsed = new URL(finalUrl);
  } catch (_) {
    return {
      isAuthRecovery: false,
      kind: null,
      finalUrlHost: null,
      accountEmails: [],
    };
  }

  const host = parsed.hostname.toLowerCase();
  const isGoogleAccountsHost =
    host === "accounts.google.com" || host.endsWith(".accounts.google.com");
  if (!isGoogleAccountsHost) {
    return {
      isAuthRecovery: false,
      kind: null,
      finalUrlHost: host,
      accountEmails: [],
    };
  }

  const path = parsed.pathname.toLowerCase();
  let kind = "google_auth";
  if (path.includes("accountchooser")) {
    kind = "google_account_chooser";
  } else if (path.includes("signin")) {
    kind = "google_signin";
  } else if (path.includes("challenge")) {
    kind = "google_challenge";
  }

  const accountEmails = Array.isArray(capture?.finalPage?.uiSignals?.accountEmails)
    ? capture.finalPage.uiSignals.accountEmails
    : [];

  return {
    isAuthRecovery: true,
    kind,
    finalUrlHost: host,
    accountEmails,
  };
}

function summarizeLocalProxyErrors(localConnections) {
  return localConnections
    .flatMap((connection) =>
      (Array.isArray(connection?.framesReceived)
        ? connection.framesReceived
        : []
      ).map((frame) => {
        if (typeof frame !== "string") {
          return null;
        }
        let parsed = null;
        try {
          parsed = JSON.parse(frame);
        } catch (_) {
          return null;
        }
        if (parsed?.event_type !== "error") {
          return null;
        }
        return {
          status: Number.isInteger(parsed.status) ? parsed.status : null,
          requestId: normalizeString(parsed.request_id),
          message: previewText(parsed.message ?? "local proxy error", 512),
        };
      }),
    )
    .filter(Boolean)
    .slice(0, 8);
}

function buildProbeSummary({
  capture,
  captureDir,
  executablePath,
  runtimeState,
  appUrl,
  failUnlessTargetRpcCaptured,
  targetRpcSummary,
  normalizedTargetRpcContract,
  targetRpcContractObjectKey,
  targetRpcContractMirrorPath,
  localProxyRequest,
}) {
  const requests = Array.isArray(capture?.requests) ? capture.requests : [];
  const responses = Array.isArray(capture?.responses) ? capture.responses : [];
  const websockets = Array.isArray(capture?.websockets) ? capture.websockets : [];
  const localConnections = Array.isArray(capture?.localWebSocket?.connections)
    ? capture.localWebSocket.connections
    : [];
  const hasCapturedTraffic =
    requests.length > 0 || websockets.length > 0 || localConnections.length > 0;
  const normalizedContract =
    normalizedTargetRpcContract ??
    buildNormalizedTargetRpcContract(targetRpcSummary);
  const replayReadyTargetRpcContract = isReplayReadyTargetRpcContract(
    normalizedContract,
  );
  const targetRpcResponseStatuses = {
    codeAssistantOffline:
      normalizedContract?.codeAssistantOffline?.responseStatus ?? null,
    streamCodeAssistantOfflineGeneration:
      normalizedContract?.streamCodeAssistantOfflineGeneration?.responseStatus ?? null,
  };
  const targetRpcFailure =
    [
      {
        kind: "codeAssistantOffline",
        status: targetRpcResponseStatuses.codeAssistantOffline,
        bodyPreview:
          normalizedContract?.codeAssistantOffline?.responseBodyPreview ?? null,
      },
      {
        kind: "streamCodeAssistantOfflineGeneration",
        status: targetRpcResponseStatuses.streamCodeAssistantOfflineGeneration,
        bodyPreview:
          normalizedContract?.streamCodeAssistantOfflineGeneration
            ?.responseBodyPreview ?? null,
      },
    ].find(
      (entry) =>
        Number.isInteger(entry.status) && !isSuccessfulTargetRpcStatus(entry.status),
    ) ?? null;
  if (targetRpcFailure?.bodyPreview) {
    targetRpcFailure.bodyPreview = previewText(targetRpcFailure.bodyPreview, 512);
  }
  const authRecoveryState = classifyAistudioAuthRecovery(capture);
  const authRecoveryBlocksOk =
    authRecoveryState.isAuthRecovery &&
    !targetRpcSummary.capturedTargetRpcContract;
  const localProxyErrors = summarizeLocalProxyErrors(localConnections);

  return {
    ok: failUnlessTargetRpcCaptured
      ? replayReadyTargetRpcContract
      : hasCapturedTraffic && !authRecoveryBlocksOk,
    captureDir,
    executablePath,
    runtimeStateMode: runtimeState.mode,
    runtimeStatePath: runtimeState.absolutePath,
    appUrl,
    browserProxyMode: capture?.browserProxyMode ?? null,
    browserProxyServer: capture?.browserProxyServer ?? null,
    browserProxyPreflight: capture?.browserProxyPreflight ?? null,
    finalUrl: capture?.finalUrl ?? null,
    matchedRequestCount: requests.length,
    matchedResponseCount: responses.length,
    matchedWebSocketCount: websockets.length,
    matchedLocalWebSocketConnections: localConnections.length,
    capturedTargetRpcContract: targetRpcSummary.capturedTargetRpcContract,
    replayReadyTargetRpcContract,
    matchedCodeAssistantOfflineCount:
      targetRpcSummary.codeAssistantOfflineCount,
    matchedStreamCodeAssistantOfflineGenerationCount:
      targetRpcSummary.streamCodeAssistantOfflineGenerationCount,
    targetRpcModelPath: normalizedContract?.modelPath ?? null,
    targetRpcResponseStatuses,
    targetRpcFailure,
    authRecoveryState,
    pageUiSignals: capture?.finalPage?.uiSignals ?? null,
    targetRpcSummaryPath: captureDir
      ? path.join(captureDir, "target-rpc-summary.json")
      : null,
    normalizedTargetRpcContractPath: captureDir
      ? path.join(captureDir, "normalized-target-rpc-contract.json")
      : null,
    targetRpcContractObjectKey,
    targetRpcContractMirrorPath,
    localProxyRequestEnabled: Boolean(localProxyRequest),
    localProxySentEventTypes:
      localConnections.flatMap((entry) => entry.sentEventTypes ?? []),
    localProxyReceivedEventTypes:
      localConnections.flatMap((entry) => entry.receivedEventTypes ?? []),
    localProxyErrors,
    firstMatchedUrl: requests[0]?.url ?? websockets[0]?.url ?? null,
    runAppRedirectLocation:
      responses.find((entry) =>
        typeof entry.headers?.location === "string" &&
        entry.headers.location.includes("run.app"),
      )?.headers?.location ?? null,
    note:
      replayReadyTargetRpcContract
        ? "Captured target AI Studio MakerSuite RPC contract. Inspect target-rpc-summary.json and numbered pair files."
        : authRecoveryBlocksOk
          ? "Reached Google auth recovery/account chooser before capturing target AI Studio MakerSuite RPCs. Refresh storage-state or select the account in a visible browser run."
        : targetRpcSummary.capturedTargetRpcContract
          ? "Captured target AI Studio MakerSuite RPC pairs, but normalized contract is missing replay-critical appId / opaque token / URL material or 2xx response status."
        : requests.length > 0 || websockets.length > 0
          ? "Captured AI Studio traffic, but not the target CodeAssistantOffline / StreamCodeAssistantOfflineGeneration contract yet."
          : "No matching AI Studio traffic was captured before timeout. If login or prompt submission was required, rerun the probe and perform the action in the visible browser window.",
  };
}

export { buildProbeSummary, extractAistudioUiSignals };
