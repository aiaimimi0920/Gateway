import path from "node:path";

export function createBrowserlessVideoOperation({
  DEFAULT_VIDEO_POLL_INTERVAL_MS,
  normalizeString,
  writeBuffer,
  buildVideoCreateRequestBody,
  extractVideoUriFromOperation,
  mimeToExt,
  summarizeJsonBody,
  sendJsonWithAuthAttempts,
  sendExactMinimalApiKeyOnlyJson,
  sendGetJsonWithAuthAttempts,
  sendGetBytesWithAuthAttempts,
  looksLikeQuotaOrPlanGate,
}) {
  async function probeVideoCreate(context) {
    const invokeContractRequestUrl = normalizeString(
      context.browserState?.canvasProgramInvokeContract?.requestUrl,
    );
    const browserStateVideoInvokePath = normalizeString(context.browserState?.videoInvokePath);
    const requestUrl = invokeContractRequestUrl
      ? invokeContractRequestUrl
      : browserStateVideoInvokePath
        ? buildVideoInvokeUrl(
            context.apiBaseUrl,
            browserStateVideoInvokePath,
            context.model,
          )
        : `${context.apiBaseUrl.replace(/\/+$/, "")}/models/${context.model}:predictLongRunning`;
    const requestBody = buildVideoCreateRequestBody(
      context.prompt,
      context.aspectRatio,
      context.durationSeconds,
    );
    const exactAttempts = [];
    for (let retry = 0; retry < 3; retry += 1) {
      const exactResult = await sendExactMinimalApiKeyOnlyJson(
        context,
        `video.create.exact-${String(retry + 1).padStart(2, "0")}`,
        requestUrl,
        requestBody,
        context.timeoutMs,
      );
      exactAttempts.push(exactResult);
      if (
        exactResult.response &&
        looksLikeQuotaOrPlanGate(exactResult.response.status, exactResult.response.text)
      ) {
        return {
          kind: "official_like_predict_long_running",
          requestUrl: exactResult.request.url,
          requestBody: exactResult.request.body,
          status: exactResult.response.status,
          ok: false,
          bodySummary: summarizeJsonBody(exactResult.responseJson),
          finalPollSummary: null,
          pollRecords: null,
          asset: null,
          error: "video_official_gate",
        };
      }
      const exactErrorMessage = normalizeString(exactResult.responseJson?.error?.message);
      if (!/origin doesn't match host for xd3/i.test(exactErrorMessage || "")) {
        break;
      }
      await sleep(750);
    }
    const createResult = await sendJsonWithAuthAttempts(
      context,
      "video.create",
      requestUrl,
      requestBody,
      context.timeoutMs,
    );
    const operationName = normalizeString(createResult.responseJson?.name);
    if (!operationName) {
      return {
        kind: "official_like_predict_long_running",
        requestUrl,
        requestBody,
        status: createResult.response?.status ?? null,
        ok: false,
        bodySummary: summarizeJsonBody(createResult.responseJson),
        error: "video_operation_name_missing",
      };
    }
    const operationUrl = operationName.startsWith("http://") || operationName.startsWith("https://")
      ? operationName
      : `${context.apiBaseUrl.replace(/\/+$/, "")}/${operationName.replace(/^\/+/, "")}`;

    const deadline = Date.now() + context.videoPollTimeoutMs;
    const pollRecords = [];
    let finalPollJson = createResult.responseJson;
    while (Date.now() < deadline) {
      if (finalPollJson?.done === true) {
        break;
      }
      await sleep(DEFAULT_VIDEO_POLL_INTERVAL_MS);
      const pollResult = await sendGetJsonWithAuthAttempts(
        context,
        `video.poll-${String(pollRecords.length + 1).padStart(2, "0")}`,
        operationUrl,
        Math.min(context.timeoutMs, 30_000),
      );
      finalPollJson = pollResult.responseJson;
      pollRecords.push({
        status: pollResult.response?.status ?? null,
        ok: pollResult.ok,
        bodySummary: summarizeJsonBody(finalPollJson),
      });
      if (!pollResult.ok) {
        return {
          kind: "official_like_predict_long_running",
          requestUrl,
          requestBody,
          status: createResult.response?.status ?? null,
          ok: false,
          operationUrl,
          bodySummary: summarizeJsonBody(createResult.responseJson),
          pollRecords,
          error: "video_poll_failed",
        };
      }
    }
    const videoUri = extractVideoUriFromOperation(finalPollJson);
    let asset = null;
    if (videoUri) {
      const bytesResult = await sendGetBytesWithAuthAttempts(
        context,
        "video.asset",
        videoUri,
        Math.min(context.videoPollTimeoutMs, 120_000),
      );
      if (bytesResult.ok) {
        const contentType =
          normalizeString(bytesResult.response?.headers?.["content-type"]) ?? "video/mp4";
        const ext = mimeToExt(contentType);
        const assetPath = path.join(context.outDir, `video-asset${ext}`);
        await writeBuffer(assetPath, bytesResult.response.bytes);
        asset = {
          url: videoUri,
          contentType,
          bytesLength: bytesResult.response.bytes.length,
          assetPath,
        };
      }
    }
    return {
      kind: "official_like_predict_long_running",
      requestUrl,
      requestBody,
      status: createResult.response?.status ?? null,
      ok: finalPollJson?.done === true && !finalPollJson?.error,
      operationUrl,
      bodySummary: summarizeJsonBody(createResult.responseJson),
      finalPollSummary: summarizeJsonBody(finalPollJson),
      pollRecords,
      asset,
    };
  }

  function buildVideoInvokeUrl(apiBaseUrl, videoInvokePath, model) {
    const normalizedPath = normalizeString(videoInvokePath);
    if (!normalizedPath) {
      return `${apiBaseUrl.replace(/\/+$/, "")}/models/${model}:predictLongRunning`;
    }
    if (normalizedPath.startsWith("http://") || normalizedPath.startsWith("https://")) {
      return normalizedPath;
    }
    return `${apiBaseUrl.replace(/\/+$/, "")}${normalizedPath.startsWith("/") ? normalizedPath : `/${normalizedPath}`}`;
  }

  function sleep(ms) {
    return new Promise((resolve) => setTimeout(resolve, ms));
  }

  return {
    probeVideoCreate,
    buildVideoInvokeUrl,
  };
}
