import { performance } from "node:perf_hooks";
import {
  readStringFields,
  readNumberFields,
  buildCreativePrompt,
  buildClientContext,
  findToolReturn,
  readToolJobId,
} from "./page-request-input.mjs";
import { sendProducerPageConversation } from "./page-conversation.mjs";
import { pollProducerPageVideoStatus } from "./page-status.mjs";

export const executeProducerPageVideoFlow = async (page, {
  authToken,
  baseUrl,
  origin,
  referer,
  requestBody,
  model,
  timeoutMs,
}) => {
  const defaultConfirmPrompt = "Create the video";

  const totalDeadline = performance.now() + timeoutMs;
  const timeoutResult = () => ({
    ok: false,
    error: {
      status: 504,
      code: "producer_browser_video_timeout",
      message: "Timed out while waiting for Producer video generation to complete.",
    },
  });
  const evaluatePhase = async (callback, args) => {
    const remaining = totalDeadline - performance.now();
    if (remaining <= 0) return timeoutResult();
    const result = await page.evaluate(callback, { ...args, timeoutMs: remaining });
    // A late phase result must not publish success or admit another submission.
    return performance.now() >= totalDeadline ? timeoutResult() : result;
  };
  const normalizedModel =
    typeof model === "string" && model.trim() ? model.trim() : "producer:music-video";
  const clipId = readStringFields(requestBody, [
    "clip_id",
    "clipId",
    "song_id",
    "songId",
  ]);
  if (!clipId) {
    return {
      ok: false,
      error: {
        status: 400,
        code: "missing_video_clip_id",
        message:
          "Producer browser worker requires clip_id (or song_id/songId) for video generation.",
      },
    };
  }

  const creativePrompt = buildCreativePrompt(requestBody);
  const clientContext = buildClientContext(requestBody, clipId, normalizedModel);
  const commonHeaders = {
    accept: "application/json, text/plain, */*",
    authorization: `Bearer ${authToken}`,
    "content-type": "application/json",
    origin,
    referer,
  };

  const bootstrapResult = await evaluatePhase(sendProducerPageConversation, {
    baseUrl,
    headers: commonHeaders,
    prompt: `Let's make a music video with the song ${baseUrl.replace(/\/+$/, "")}/song/${clipId}`,
    clientContext,
    modelName: "producer:standard",
  });
  if (!bootstrapResult.ok) {
    return bootstrapResult;
  }

  const conversationId = bootstrapResult.stream.conversationId;
  if (!conversationId) {
    return {
      ok: false,
      error: {
        status: 500,
        code: "producer_browser_missing_conversation_id",
        message:
          "Producer conversation bootstrap completed without returning a conversation_id.",
        body: bootstrapResult.stream.rawText,
      },
    };
  }

  const creativeResult = await evaluatePhase(sendProducerPageConversation, {
    baseUrl,
    headers: commonHeaders,
    prompt: creativePrompt,
    conversationId,
    clientContext,
    modelName: "producer:standard",
  });
  if (!creativeResult.ok) {
    return creativeResult;
  }

  let createToolReturn = findToolReturn(
    creativeResult.stream.toolReturns,
    "video__create_music_video",
  );

  let confirmationResult = null;
  if (!createToolReturn) {
    confirmationResult = await evaluatePhase(sendProducerPageConversation, {
      baseUrl,
      headers: commonHeaders,
      prompt:
        readStringFields(requestBody, [
          "confirm_prompt",
          "confirmPrompt",
        ]) ?? defaultConfirmPrompt,
      conversationId,
      clientContext,
      modelName: "producer:standard",
    });
    if (!confirmationResult.ok) {
      return confirmationResult;
    }
    createToolReturn = findToolReturn(
      confirmationResult.stream.toolReturns,
      "video__create_music_video",
    );
  }

  const videoJobId = readToolJobId(createToolReturn);
  if (!videoJobId) {
    return {
      ok: false,
      error: {
        status: 500,
        code: "producer_browser_missing_video_job_id",
        message:
          "Producer conversation completed without returning a music-video job_id.",
        body: JSON.stringify(
          confirmationResult?.stream ?? creativeResult.stream,
          null,
          2,
        ),
      },
    };
  }

  const statusResult = await evaluatePhase(pollProducerPageVideoStatus, {
    baseUrl,
    headers: commonHeaders,
    jobId: videoJobId,
  });
  if (!statusResult.ok) {
    return statusResult;
  }

  const primaryUrl = statusResult.result.videoUrl;
  if (!primaryUrl) {
    return {
      ok: false,
      error: {
        status: 500,
        code: "producer_browser_missing_video_url",
        message:
          "Producer video job completed but no downloadable video URL was found in the status payload.",
        body: JSON.stringify(statusResult.result.statusPayload, null, 2),
      },
    };
  }

  return {
    ok: true,
    status: 200,
    result: {
      object: "video.generation",
      provider: "producer.ai",
      created: Math.floor(Date.now() / 1000),
      completed: true,
      model: normalizedModel,
      prompt: creativePrompt,
      clip_id: clipId,
      conversation_id: conversationId,
      bootstrap_job_id: bootstrapResult.jobId,
      creative_job_id: creativeResult.jobId,
      confirmation_job_id: confirmationResult?.jobId ?? null,
      job_id: videoJobId,
      progress_url: `${baseUrl.replace(/\/+$/, "")}/session/${conversationId}`,
      status_path: `/__api/music-video/${videoJobId}/status`,
      detail_path: `/__api/music-video/get/${videoJobId}`,
      library_path: "/library/videos",
      data: [
        {
          kind: "video",
          url: primaryUrl,
          mime_type: "video/mp4",
          preview_url: statusResult.result.previewUrl,
          aspect_ratio:
            readStringFields(requestBody, ["aspect_ratio", "aspectRatio"]) ?? null,
          resolution: readStringFields(requestBody, ["resolution"]) ?? null,
          duration_seconds:
            readNumberFields(requestBody, [
              "duration_s",
              "durationSeconds",
              "duration",
            ]) ?? null,
        },
      ],
      status: statusResult.result.statusPayload,
      creative_stream: {
        suggestions: creativeResult.stream.suggestions,
        message_texts: creativeResult.stream.messageTexts,
        tool_names: creativeResult.stream.toolReturns.map((entry) => entry.toolName),
      },
    },
  };
};
