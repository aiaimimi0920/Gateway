import { appendTrace } from "./trace.mjs";
import { browserContextFetch, browserContextReadSse, nodeSideFetch } from "./transport.mjs";
import { normalizeString, readNumberFields } from "./request-fields.mjs";
import { extractConversationIdFromStream, parseMaybeJson, summarizeConversationStream, collectMediaUrls } from "./conversation-stream.mjs";
import { buildProducerProposalPrompt, chooseProducerConfirmPrompt } from "./video-prompts.mjs";

const STATUS_POLL_INTERVAL_MS = 5000;

async function delay(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

export async function executeProducerConversationVideoFlow(args) {
  const {
    page,
    baseUrl,
    authToken,
    cookieHeader,
    clipId,
    creativePrompt,
    requestBody,
    acceptAsyncJob = true,
    timeoutMs,
    initialReferer,
  } = args;
  let stage = "bootstrap";
  await appendTrace("video-stage", { stage });

  const commonHeaders = {
    accept: "application/json, text/plain, */*",
    authorization: `Bearer ${authToken}`,
    "content-type": "application/json",
    origin: baseUrl,
  };

  const postConversation = async (conversationId, prompt, referer) => {
    const response = await browserContextFetch(page, `${baseUrl}/__api/conversation`, {
      method: "POST",
      credentials: "include",
      headers: {
        ...commonHeaders,
        referer,
      },
      body: JSON.stringify({
        ...(conversationId ? { conversation_id: conversationId } : {}),
        parts: [{ content: prompt, part_kind: "user-prompt" }],
        client_context: {
          current_song_id: clipId,
          song_queue: [{ id: clipId }],
          selected_model: "producer:standard",
          lyrics_id_map: {},
          ghostwriter_version: "standard",
        },
        model_name: "producer:standard",
        mode: "standard",
      }),
    });
    if (!response.ok) {
      return {
        ok: false,
        error: {
          status: response.status,
          code: "producer_browser_conversation_failed",
          message: "Producer conversation request failed.",
          body: response.text,
        },
      };
    }

    let bodyJson;
    try {
      bodyJson = JSON.parse(response.text);
    } catch {
      return {
        ok: false,
        error: {
          status: 500,
          code: "producer_browser_invalid_conversation_response",
          message: "Producer conversation response was not valid JSON.",
          body: response.text,
        },
      };
    }

    const jobId = normalizeString(bodyJson.job_id ?? bodyJson.jobId);
    if (!jobId) {
      return {
        ok: false,
        error: {
          status: 500,
          code: "producer_browser_missing_job_id",
          message: "Producer conversation response did not include a job_id.",
          body: response.text,
        },
      };
    }

    const stream = await browserContextReadSse(
      page,
      `${baseUrl}/__api/messages/${jobId}/stream?last_id=0`,
      {
        method: "GET",
        credentials: "include",
        headers: {
          accept: "text/event-stream",
          authorization: `Bearer ${authToken}`,
          origin: baseUrl,
          referer,
        },
      },
      Math.min(timeoutMs, 90_000),
    );
    if (!stream.ok) {
      return {
        ok: false,
        error: {
          status: stream.status,
          code: "producer_browser_stream_failed",
          message: "Producer message stream request failed.",
          body: stream.text,
        },
      };
    }

    return {
      ok: true,
      jobId,
      streamText: stream.text,
      summary: summarizeConversationStream(stream.text),
    };
  };

  const bootstrap = await postConversation(
    null,
    `Let's make a music video with the song ${baseUrl}/song/${clipId}`,
    initialReferer,
  );
  if (!bootstrap.ok) {
    return bootstrap;
  }

  const conversationId = extractConversationIdFromStream(bootstrap.streamText);
  if (!conversationId) {
    return {
      ok: false,
      error: {
        status: 500,
        code: "producer_browser_missing_conversation_id",
        message: "Producer conversation bootstrap completed without returning a conversation_id.",
        body: bootstrap.streamText,
      },
    };
  }

  const sessionReferer = `${baseUrl}/session/${conversationId}`;
  const proposalPrompt = buildProducerProposalPrompt(requestBody, creativePrompt);
  stage = "creative";
  await appendTrace("video-stage", { stage, conversationId });
  const creative = await postConversation(conversationId, proposalPrompt, sessionReferer);
  if (!creative.ok) {
    return creative;
  }

  const proposalToolSeen = creative.summary.toolReturns.some(
    (entry) => entry.toolName === "video__propose_music_video",
  );
  const proposalInputs =
    creative.summary.toolCalls.find((entry) => entry.toolName === "video__propose_music_video")
      ?.args?.inputs ?? null;
  let createTool = creative.summary.toolReturns.find(
    (entry) => entry.toolName === "video__create_music_video",
  );
  let confirm = null;
  if (!createTool) {
    stage = "confirm";
    await appendTrace("video-stage", { stage, conversationId });
    const confirmPrompt = chooseProducerConfirmPrompt(
      requestBody,
      creative.summary.suggestions,
      creative.summary.messageTexts,
      proposalInputs,
    );
    confirm = await postConversation(conversationId, confirmPrompt, sessionReferer);
    if (!confirm.ok) {
      return confirm;
    }
    createTool = confirm.summary.toolReturns.find(
      (entry) => entry.toolName === "video__create_music_video",
    );
  }

  const proposalRetrySeen = (confirm?.summary.retryPrompts ?? []).some((entry) =>
    /video__propose_music_video/i.test(entry),
  );
  if (!proposalToolSeen && proposalRetrySeen) {
    return {
      ok: false,
      error: {
        status: 500,
        code: "producer_browser_missing_video_proposal",
        message:
          "Producer video flow did not complete the required video__propose_music_video step before confirmation.",
        body: JSON.stringify(
          {
            creativeSummary: creative.summary,
            confirmSummary: confirm?.summary ?? null,
          },
          null,
          2,
        ),
      },
    };
  }

  const videoJobId = normalizeString(createTool?.content?.job_id ?? createTool?.content?.jobId);
  if (!videoJobId) {
    return {
      ok: false,
      error: {
        status: 500,
        code: "producer_browser_missing_video_job_id",
        message:
          "Producer conversation completed without returning a music-video job_id.",
        body: JSON.stringify(
          {
            creativeSummary: creative.summary,
            confirmSummary: confirm?.summary ?? null,
          },
          null,
          2,
        ),
      },
    };
  }

  const fetchVideoStatus = async (requestTimeoutMs) => {
    const status = await nodeSideFetch(
      `${baseUrl}/__api/music-video/${videoJobId}/status`,
      {
        method: "GET",
        headers: {
          accept: "application/json, text/plain, */*",
          authorization: `Bearer ${authToken}`,
          ...(cookieHeader ? { cookie: cookieHeader } : {}),
          origin: baseUrl,
          referer: sessionReferer,
        },
      },
      requestTimeoutMs,
    );
    if (!status.ok) {
      return {
        ok: false,
        error: {
          status: status.status,
          code: "producer_browser_status_failed",
          message: "Producer music-video status request failed.",
          body: status.text,
        },
      };
    }

    const payload = parseMaybeJson(status.text);
    const normalizedStatus =
      typeof payload?.status === "string"
        ? payload.status.toLowerCase()
        : typeof payload?.state?.status === "string"
          ? payload.state.status.toLowerCase()
          : null;
    return {
      ok: true,
      payload,
      normalizedStatus,
    };
  };

  let statusPayload = null;
  let finalStatus = null;
  stage = "status-poll";
  await appendTrace("video-stage", {
    stage,
    conversationId,
    jobId: videoJobId,
    acceptAsyncJob,
  });

  if (acceptAsyncJob) {
    const initialStatus = await fetchVideoStatus(Math.min(timeoutMs, 60_000));
    if (!initialStatus.ok) {
      return initialStatus;
    }
    statusPayload = initialStatus.payload;
    finalStatus = initialStatus.normalizedStatus;
    if (["failed", "error", "cancelled", "canceled"].includes(finalStatus)) {
      return {
        ok: false,
        error: {
          status: 502,
          code: "producer_browser_video_failed",
          message: `Producer video job entered terminal status '${finalStatus}'.`,
          body: JSON.stringify(statusPayload),
        },
      };
    }
  } else {
    const deadline = Date.now() + timeoutMs;
    while (Date.now() < deadline) {
      const currentStatus = await fetchVideoStatus(Math.min(timeoutMs, 60_000));
      if (!currentStatus.ok) {
        return currentStatus;
      }

      statusPayload = currentStatus.payload;
      finalStatus = currentStatus.normalizedStatus;
      if (finalStatus === "completed") {
        break;
      }
      if (["failed", "error", "cancelled", "canceled"].includes(finalStatus)) {
        return {
          ok: false,
          error: {
            status: 502,
            code: "producer_browser_video_failed",
            message: `Producer video job entered terminal status '${finalStatus}'.`,
            body: JSON.stringify(statusPayload),
          },
        };
      }
      await delay(STATUS_POLL_INTERVAL_MS);
    }
  }

  const urls = collectMediaUrls(statusPayload, []);
  const finalUrl =
    urls.find((entry) => entry.includes(`/music-video/${videoJobId}/`)) ??
    urls.find((entry) => entry.includes("/music-video/")) ??
    urls[0] ??
    null;
  const isCompleted = finalStatus === "completed" && Boolean(finalUrl);

  return {
    ok: true,
    status: 200,
    result: {
      object: "video.generation",
      provider: "producer.ai",
      created: Math.floor(Date.now() / 1000),
      completed: isCompleted,
      accepted: true,
      model: requestBody?.model ?? "producer:music-video",
      prompt: creativePrompt,
      clip_id: clipId,
      conversation_id: conversationId,
      bootstrap_job_id: bootstrap.jobId,
      creative_job_id: creative.jobId,
      confirmation_job_id: confirm?.jobId ?? null,
      job_id: videoJobId,
      progress_url: `${baseUrl}/session/${conversationId}`,
      status_path: `/__api/music-video/${videoJobId}/status`,
      detail_path: `/__api/music-video/get/${videoJobId}`,
      library_path: "/library/videos",
      data: [
        {
          kind: "video",
          url: finalUrl,
          mime_type: "video/mp4",
          preview_url: statusPayload?.preview?.video ?? null,
          aspect_ratio:
            normalizeString(requestBody?.aspect_ratio) ??
            normalizeString(requestBody?.aspectRatio) ??
            null,
          resolution: normalizeString(requestBody?.resolution) ?? null,
          duration_seconds:
            readNumberFields(requestBody, ["duration_s", "durationSeconds", "duration"]) ?? null,
        },
      ],
      state: finalStatus ?? "accepted",
      status: statusPayload,
      creative_stream: {
        suggestions: creative.summary.suggestions,
        message_texts: creative.summary.messageTexts,
        tool_names: creative.summary.toolReturns.map((entry) => entry.toolName),
      },
      confirmation_stream: confirm
        ? {
            suggestions: confirm.summary.suggestions,
            message_texts: confirm.summary.messageTexts,
            tool_names: confirm.summary.toolReturns.map((entry) => entry.toolName),
          }
        : null,
    },
  };
}
