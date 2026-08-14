import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { copyFile, mkdtemp, mkdir, readdir, rm, stat } from "node:fs/promises";
import os from "node:os";
import path from "node:path";

const DEFAULT_TIMEOUT_MS = 20 * 60 * 1000;
const DEFAULT_LOCALE = "zh-CN";
const DEFAULT_CONFIRM_PROMPT = "Create the video";
const DEFAULT_VIDEO_PROMPT = "Create a cinematic music video for this song.";
const STATUS_POLL_INTERVAL_MS = 5000;
const DEFAULT_PRODUCER_SUPABASE_URL = "https://sb.flowmusic.app";
const DEFAULT_PRODUCER_SUPABASE_ANON_KEY =
  "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImVkbmpjY3FjbWJ4ZWF4YmlkaW5yIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzE1NjEwNjQsImV4cCI6MjA4NzEzNzA2NH0.XCXSuL7Th1xHecfRrP0vAOFmKwJxwBqVFLu06SxtVzg";

const WINDOWS_BROWSER_PATHS = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];
const MACOS_BROWSER_PATHS = [
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
];
const LINUX_BROWSER_PATHS = [
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
];
const WINDOWS_CHROME_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Google",
  "Chrome",
  "User Data",
);
const WINDOWS_EDGE_USER_DATA_DIR = path.join(
  process.env.LOCALAPPDATA ?? "",
  "Microsoft",
  "Edge",
  "User Data",
);
const PROFILE_FILES_TO_COPY = ["Preferences", "Secure Preferences"];
const PROFILE_DIRS_TO_COPY = [
  "Network",
  "Local Storage",
  "Session Storage",
  "IndexedDB",
  "WebStorage",
  "Service Worker",
];
const TRACE_PATH = normalizeTracePath(process.env.PRODUCER_BROWSER_TRACE_PATH);

function normalizeTracePath(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

async function appendTrace(event, details = {}) {
  if (!TRACE_PATH) {
    return;
  }
  const line = JSON.stringify({
    ts: new Date().toISOString(),
    event,
    ...details,
  });
  await mkdir(path.dirname(TRACE_PATH), { recursive: true }).catch(() => undefined);
  await import("node:fs/promises").then(({ appendFile }) =>
    appendFile(TRACE_PATH, `${line}\n`, "utf8").catch(() => undefined),
  );
}

async function main() {
  let currentStage = "init";
  try {
    await appendTrace("stage", { stage: currentStage });
    currentStage = "read-input";
    await appendTrace("stage", { stage: currentStage });
    const raw = await readStdin();
    const input = JSON.parse(raw);
    validateInput(input);

    currentStage = "resolve-browser";
    await appendTrace("stage", { stage: currentStage });
    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ?? process.env.PRODUCER_BROWSER_EXECUTABLE_PATH ?? null,
    );
    if (!executablePath) {
      throw Object.assign(
        new Error(
          "Unable to locate a Chromium-compatible browser. Set PRODUCER_BROWSER_EXECUTABLE_PATH.",
        ),
        { status: 500, code: "producer_browser_not_found" },
      );
    }

    currentStage = "resolve-session";
    await appendTrace("stage", { stage: currentStage });
    const timeoutMs = normalizeTimeoutMs(input.timeoutMs);
    const cookieHeader = normalizeString(input.cookieHeader);
    const supabaseSession = extractSupabaseSession(cookieHeader);
    const refreshedSession = supabaseSession?.refreshToken
      ? await refreshSupabaseAccessToken(supabaseSession.refreshToken)
      : null;
    const authToken =
      refreshedSession?.accessToken ??
      supabaseSession?.accessToken ??
      normalizeString(input.authToken);
    if (!authToken) {
      throw Object.assign(
        new Error(
          "Producer browser worker requires a bearer token or sb-sb-auth-token cookies carrying a current access token.",
        ),
        { status: 401, code: "producer_browser_missing_auth_token" },
      );
    }

    currentStage = "prepare-browser";
    await appendTrace("stage", { stage: currentStage });
    const baseUrl = normalizeBaseUrl(input.baseUrl);
      const referer =
        normalizeString(input.referer) ?? `${baseUrl.replace(/\/+$/, "")}/library/videos`;
    const origin = normalizeString(input.origin) ?? baseUrl.replace(/\/+$/, "");
    const sourceProfile = resolveBrowserProfileSource(input);
    const clonedProfileRoot = sourceProfile
      ? await cloneBrowserProfile(sourceProfile.userDataDir, sourceProfile.profileDirectory)
      : null;
    let browser = null;
    let context = null;

    try {
      currentStage = "launch-browser";
      await appendTrace("stage", { stage: currentStage });
      const locale = normalizeLocale(input.acceptLanguage) ?? DEFAULT_LOCALE;
      if (clonedProfileRoot) {
        context = await chromium.launchPersistentContext(clonedProfileRoot, {
          executablePath,
          headless: parseBoolean(process.env.PRODUCER_BROWSER_HEADLESS, true),
          locale,
          userAgent: normalizeString(input.userAgent) ?? undefined,
          args: [
            "--disable-blink-features=AutomationControlled",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--no-default-browser-check",
            `--profile-directory=${sourceProfile.profileDirectory}`,
          ],
        });
      } else {
        browser = await chromium.launch({
          executablePath,
          headless: parseBoolean(process.env.PRODUCER_BROWSER_HEADLESS, true),
          args: [
            "--disable-blink-features=AutomationControlled",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--no-default-browser-check",
          ],
        });
        context = await browser.newContext({
          locale,
          userAgent: normalizeString(input.userAgent) ?? undefined,
        });
      }
      if (cookieHeader) {
        await context.addCookies(parseCookieHeader(cookieHeader, baseUrl));
      }

      currentStage = "open-page";
      const navigationTargets = buildProducerNavigationTargets(baseUrl, referer);
      await appendTrace("stage", {
        stage: currentStage,
        referer,
        navigationTargets,
      });
      const page = context.pages()[0] ?? (await context.newPage());
      page.setDefaultNavigationTimeout(Math.min(timeoutMs, 90_000));
      await openProducerLandingPage(page, navigationTargets, timeoutMs);
      await page.waitForTimeout(800);

      const clipId =
        normalizeString(input.requestBody?.clip_id) ??
        normalizeString(input.requestBody?.clipId) ??
        normalizeString(input.requestBody?.song_id) ??
        normalizeString(input.requestBody?.songId);
      if (clipId) {
        currentStage = "video-flow";
        await appendTrace("stage", { stage: currentStage, clipId });
        const creativePrompt =
          normalizeString(input.requestBody?.user_message) ??
          normalizeString(input.requestBody?.userMessage) ??
          normalizeString(input.requestBody?.prompt) ??
          normalizeString(input.requestBody?.input) ??
          DEFAULT_VIDEO_PROMPT;
        const nodeWorkerResult = await executeProducerConversationVideoFlow({
          page,
          baseUrl,
          authToken,
          cookieHeader,
          clipId,
          creativePrompt,
          requestBody: input.requestBody ?? {},
          acceptAsyncJob: input.acceptAsyncJob !== false,
          timeoutMs,
          initialReferer: referer,
        });
        await printJsonAndExit(nodeWorkerResult);
        return;
      }

      currentStage = "page-evaluate";
      await appendTrace("stage", { stage: currentStage });
      const workerResult = await page.evaluate(
        async ({
          authToken,
          baseUrl,
          origin,
          referer,
          requestBody,
          model,
          timeoutMs,
        }) => {
          const defaultConfirmPrompt = "Create the video";
          const defaultVideoPrompt = "Create a cinematic music video for this song.";

          const normalizeText = (value) =>
            typeof value === "string" && value.trim() ? value.trim() : null;

          const readStringFields = (value, fields) => {
            if (!value || typeof value !== "object") {
              return null;
            }
            for (const field of fields) {
              const direct = normalizeText(value[field]);
              if (direct) {
                return direct;
              }
              const nested =
                value.args && typeof value.args === "object"
                  ? normalizeText(value.args[field])
                  : null;
              if (nested) {
                return nested;
              }
            }
            return null;
          };

          const readNumberFields = (value, fields) => {
            if (!value || typeof value !== "object") {
              return null;
            }
            for (const field of fields) {
              const direct = value[field];
              if (typeof direct === "number" && Number.isFinite(direct)) {
                return direct;
              }
              if (typeof direct === "string" && direct.trim()) {
                const parsed = Number(direct);
                if (Number.isFinite(parsed)) {
                  return parsed;
                }
              }
              const nested = value.args && typeof value.args === "object" ? value.args[field] : null;
              if (typeof nested === "number" && Number.isFinite(nested)) {
                return nested;
              }
              if (typeof nested === "string" && nested.trim()) {
                const parsed = Number(nested);
                if (Number.isFinite(parsed)) {
                  return parsed;
                }
              }
            }
            return null;
          };

          const buildClientContext = (value, clipId, modelName) => {
            const provided = value?.client_context;
            if (provided && typeof provided === "object" && !Array.isArray(provided)) {
              return {
                current_song_id: clipId,
                song_queue: Array.isArray(provided.song_queue) ? provided.song_queue : [{ id: clipId }],
                selected_model:
                  typeof provided.selected_model === "string" && provided.selected_model.trim()
                    ? provided.selected_model.trim()
                    : modelName,
                lyrics_id_map:
                  provided.lyrics_id_map && typeof provided.lyrics_id_map === "object"
                    ? provided.lyrics_id_map
                    : {},
                ghostwriter_version:
                  normalizeText(provided.ghostwriter_version) ?? "standard",
                ...provided,
              };
            }
            return {
              current_song_id: clipId,
              song_queue: [{ id: clipId }],
              selected_model: modelName,
              lyrics_id_map: {},
              ghostwriter_version: "standard",
            };
          };

          const buildCreativePrompt = (value) => {
            const lines = [];
            const prompt =
              readStringFields(value, ["user_message", "userMessage", "prompt", "input"]) ??
              defaultVideoPrompt;
            lines.push(prompt);

            const styleImageUrl = readStringFields(value, [
              "style_image_url",
              "styleImageUrl",
              "style_reference_image_url",
              "styleReferenceImageUrl",
            ]);
            if (styleImageUrl) {
              lines.push(`Use this style reference image: ${styleImageUrl}`);
            }

            const likenessImageUrl = readStringFields(value, [
              "likeness_image_url",
              "likenessImageUrl",
              "subject_image_url",
              "subjectImageUrl",
            ]);
            if (likenessImageUrl) {
              lines.push(`Preserve the subject likeness from this image: ${likenessImageUrl}`);
            }

            const aspectRatio = readStringFields(value, ["aspect_ratio", "aspectRatio", "size"]);
            if (aspectRatio) {
              lines.push(`Target aspect ratio: ${aspectRatio}.`);
            }

            const resolution = readStringFields(value, ["resolution"]);
            if (resolution) {
              lines.push(`Target resolution: ${resolution}.`);
            }

            const renderLyrics = value?.render_lyrics ?? value?.renderLyrics ?? value?.display_lyrics;
            if (typeof renderLyrics === "boolean") {
              lines.push(renderLyrics ? "Render lyrics on screen." : "Do not render lyrics on screen.");
            }

            const durationSeconds = readNumberFields(value, [
              "duration_s",
              "durationSeconds",
              "duration",
            ]);
            if (durationSeconds) {
              lines.push(`Keep the cut close to ${durationSeconds} seconds.`);
            }

            return lines.join("\n");
          };

          const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));

          const parseMaybeJson = (value) => {
            if (typeof value !== "string") {
              return value ?? null;
            }
            const trimmed = value.trim();
            if (!trimmed) {
              return null;
            }
            try {
              return JSON.parse(trimmed);
            } catch {
              return trimmed;
            }
          };

          const parseSseFrames = (rawText) => {
            const frames = [];
            let eventName = "";
            let dataLines = [];
            const commit = () => {
              if (!eventName && dataLines.length === 0) {
                return;
              }
              frames.push({
                event: eventName || null,
                dataText: dataLines.join("\n"),
              });
              eventName = "";
              dataLines = [];
            };

            for (const rawLine of String(rawText ?? "").split(/\r?\n/)) {
              const line = rawLine.trimEnd();
              if (!line) {
                commit();
                continue;
              }
              if (line.startsWith("event:")) {
                eventName = line.slice(6).trim();
                continue;
              }
              if (line.startsWith("data:")) {
                dataLines.push(line.slice(5).trimStart());
              }
            }
            commit();
            return frames;
          };

          const extractToolReturnsFromMessage = (message) => {
            if (!message || typeof message !== "object" || !Array.isArray(message.parts)) {
              return [];
            }
            return message.parts
              .filter((part) => part?.part_kind === "tool-return" && part?.content)
              .map((part) => ({
                toolName:
                  typeof part.tool_name === "string" && part.tool_name.trim()
                    ? part.tool_name.trim()
                    : null,
                content: part.content,
              }));
          };

          const extractSuggestionsFromMessage = (message) => {
            if (!message || typeof message !== "object" || !Array.isArray(message.parts)) {
              return [];
            }
            const suggestions = [];
            for (const part of message.parts) {
              if (part?.part_kind !== "suggestion") {
                continue;
              }
              if (typeof part.content === "string" && part.content.trim()) {
                suggestions.push(part.content.trim());
                continue;
              }
              if (Array.isArray(part.content)) {
                for (const entry of part.content) {
                  if (typeof entry === "string" && entry.trim()) {
                    suggestions.push(entry.trim());
                  }
                }
              }
            }
            return suggestions;
          };

          const extractMessageTextsFromMessage = (message) => {
            if (!message || typeof message !== "object" || !Array.isArray(message.parts)) {
              return [];
            }
            const texts = [];
            for (const part of message.parts) {
              if (part?.part_kind === "tool-return" || part?.part_kind === "suggestion") {
                continue;
              }
              if (typeof part?.content === "string" && part.content.trim()) {
                texts.push(part.content.trim());
              }
            }
            return texts;
          };

          const readConversationStream = async ({ jobId, headers, timeoutMs }) => {
            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
            try {
              const response = await fetch(`${baseUrl}/__api/messages/${jobId}/stream?last_id=0`, {
                method: "GET",
                credentials: "include",
                headers,
                signal: controller.signal,
              });
              const rawText = await response.text().catch(() => "");
              if (!response.ok) {
                return {
                  ok: false,
                  error: {
                    status: response.status,
                    code: "producer_browser_stream_failed",
                    message: "Producer message stream request failed.",
                    body: rawText,
                  },
                };
              }

              const frames = parseSseFrames(rawText);
              const toolReturns = [];
              const suggestions = [];
              const messageTexts = [];
              let conversationId = null;
              let finalSeen = false;

              for (const frame of frames) {
                const data = parseMaybeJson(frame.dataText);
                if (frame.event === "conversation_id") {
                  const id =
                    data && typeof data === "object" && typeof data.id === "string"
                      ? data.id.trim()
                      : null;
                  if (id) {
                    conversationId = id;
                  }
                  continue;
                }
                if (frame.event === "message") {
                  const message = data && typeof data === "object" ? data : null;
                  if (message) {
                    toolReturns.push(...extractToolReturnsFromMessage(message));
                    suggestions.push(...extractSuggestionsFromMessage(message));
                    messageTexts.push(...extractMessageTextsFromMessage(message));
                  }
                  continue;
                }
                if (frame.event === "final") {
                  finalSeen = true;
                  continue;
                }
                if (frame.event === "error") {
                  return {
                    ok: false,
                    error: {
                      status: 502,
                      code: "producer_browser_stream_error_event",
                      message:
                        data && typeof data === "object" && typeof data.message === "string"
                          ? data.message
                          : "Producer returned an error event while streaming the conversation.",
                      body: typeof data === "string" ? data : JSON.stringify(data),
                    },
                  };
                }
              }

              return {
                ok: true,
                result: {
                  rawText,
                  conversationId,
                  finalSeen,
                  suggestions,
                  messageTexts,
                  toolReturns,
                },
              };
            } finally {
              clearTimeout(timeoutId);
            }
          };

          const sendConversationMessage = async ({
            baseUrl,
            headers,
            prompt,
            conversationId,
            clientContext,
            modelName,
            timeoutMs,
          }) => {
            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
            try {
              const body = {
                parts: [
                  {
                    content: prompt,
                    part_kind: "user-prompt",
                  },
                ],
                client_context: clientContext,
                model_name: modelName,
                mode: "standard",
              };
              if (conversationId) {
                body.conversation_id = conversationId;
              }

              const response = await fetch(`${baseUrl}/__api/conversation`, {
                method: "POST",
                credentials: "include",
                headers,
                body: JSON.stringify(body),
                signal: controller.signal,
              });
              const bodyText = await response.text().catch(() => "");
              const parsedBody = parseMaybeJson(bodyText);
              if (!response.ok) {
                return {
                  ok: false,
                  error: {
                    status: response.status,
                    code: "producer_browser_conversation_failed",
                    message: "Producer conversation request failed.",
                    body:
                      typeof parsedBody === "string" ? parsedBody : JSON.stringify(parsedBody),
                  },
                };
              }

              const jobId =
                parsedBody && typeof parsedBody === "object"
                  ? normalizeText(parsedBody.job_id ?? parsedBody.jobId)
                  : null;
              if (!jobId) {
                return {
                  ok: false,
                  error: {
                    status: 500,
                    code: "producer_browser_missing_job_id",
                    message: "Producer conversation response did not include a job_id.",
                    body: bodyText,
                  },
                };
              }

              const streamResult = await readConversationStream({
                jobId,
                headers: {
                  accept: "text/event-stream",
                  authorization: headers.authorization,
                  origin: headers.origin,
                  referer: headers.referer,
                },
                timeoutMs,
              });
              if (!streamResult.ok) {
                return streamResult;
              }

              return {
                ok: true,
                jobId,
                stream: streamResult.result,
              };
            } finally {
              clearTimeout(timeoutId);
            }
          };

          const findToolReturn = (toolReturns, toolName) =>
            Array.isArray(toolReturns)
              ? toolReturns.find((entry) => entry?.toolName === toolName) ?? null
              : null;

          const readToolJobId = (toolReturn) => {
            if (!toolReturn || typeof toolReturn !== "object" || !toolReturn.content) {
              return null;
            }
            return normalizeText(toolReturn.content.job_id ?? toolReturn.content.jobId);
          };

          const collectUrls = (value, urls = []) => {
            if (typeof value === "string") {
              if (/^https?:\/\//i.test(value) && /\.(mp4|mov)(\?|$)/i.test(value)) {
                urls.push(value);
              }
              return urls;
            }
            if (!value || typeof value !== "object") {
              return urls;
            }
            if (Array.isArray(value)) {
              for (const entry of value) {
                collectUrls(entry, urls);
              }
              return urls;
            }
            for (const entry of Object.values(value)) {
              collectUrls(entry, urls);
            }
            return urls;
          };

          const chooseVideoUrl = (urls, jobId) => {
            if (!Array.isArray(urls) || urls.length === 0) {
              return null;
            }
            return (
              urls.find((entry) => entry.includes(`/music-video/${jobId}/`)) ??
              urls.find((entry) => entry.includes("/music-video/")) ??
              urls[0]
            );
          };

          const pollVideoStatus = async ({ baseUrl, headers, jobId, timeoutMs }) => {
            const deadline = Date.now() + timeoutMs;
            let latestPayload = null;

            while (Date.now() < deadline) {
              const response = await fetch(`${baseUrl}/__api/music-video/${jobId}/status`, {
                method: "GET",
                credentials: "include",
                headers: {
                  accept: "application/json, text/plain, */*",
                  authorization: headers.authorization,
                  origin: headers.origin,
                  referer: headers.referer,
                },
              });
              const bodyText = await response.text().catch(() => "");
              const parsedBody = parseMaybeJson(bodyText);
              if (!response.ok) {
                return {
                  ok: false,
                  error: {
                    status: response.status,
                    code: "producer_browser_status_failed",
                    message: "Producer music-video status request failed.",
                    body:
                      typeof parsedBody === "string" ? parsedBody : JSON.stringify(parsedBody),
                  },
                };
              }

              latestPayload = parsedBody;
              const status =
                parsedBody && typeof parsedBody === "object" && typeof parsedBody.status === "string"
                  ? parsedBody.status.trim().toLowerCase()
                  : null;
              const videoUrls = collectUrls(parsedBody, []);
              const videoUrl = chooseVideoUrl(videoUrls, jobId);
              const previewUrl =
                videoUrls.find((entry) => entry !== videoUrl) ??
                videoUrls.find((entry) => entry.includes("sample_")) ??
                null;

              if (status === "completed") {
                return {
                  ok: true,
                  result: {
                    statusPayload: parsedBody,
                    videoUrl,
                    previewUrl,
                  },
                };
              }

              if (["failed", "error", "cancelled", "canceled"].includes(status)) {
                return {
                  ok: false,
                  error: {
                    status: 502,
                    code: "producer_browser_video_failed",
                    message: `Producer video job entered terminal status '${status}'.`,
                    body: JSON.stringify(parsedBody),
                  },
                };
              }

              await delay(STATUS_POLL_INTERVAL_MS);
            }

            return {
              ok: false,
              error: {
                status: 504,
                code: "producer_browser_video_timeout",
                message: "Timed out while waiting for Producer video generation to complete.",
                body: latestPayload ? JSON.stringify(latestPayload) : undefined,
              },
            };
          };

          const totalDeadline = Date.now() + timeoutMs;
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

          const bootstrapResult = await sendConversationMessage({
            baseUrl,
            headers: commonHeaders,
            prompt: `Let's make a music video with the song ${baseUrl.replace(/\/+$/, "")}/song/${clipId}`,
            clientContext,
            modelName: "producer:standard",
            timeoutMs: Math.max(5_000, totalDeadline - Date.now()),
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

          const creativeResult = await sendConversationMessage({
            baseUrl,
            headers: commonHeaders,
            prompt: creativePrompt,
            conversationId,
            clientContext,
            modelName: "producer:standard",
            timeoutMs: Math.max(5_000, totalDeadline - Date.now()),
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
            confirmationResult = await sendConversationMessage({
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
              timeoutMs: Math.max(5_000, totalDeadline - Date.now()),
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

          const statusResult = await pollVideoStatus({
            baseUrl,
            headers: commonHeaders,
            jobId: videoJobId,
            timeoutMs: Math.max(5_000, totalDeadline - Date.now()),
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
        },
        {
          authToken,
          baseUrl,
          origin,
          referer,
          requestBody: input.requestBody ?? {},
          model: input.model,
          timeoutMs,
        },
      );

      if (workerResult?.ok) {
        await printJsonAndExit(workerResult);
      }

      await printJsonAndExit(
        workerResult ?? {
          ok: false,
          error: {
            status: 500,
            code: "producer_browser_worker_empty_result",
            message: "Producer browser worker returned an empty result object.",
          },
        },
      );
    } finally {
      await context?.close().catch(() => undefined);
      await browser?.close().catch(() => undefined);
      if (clonedProfileRoot) {
        await rm(clonedProfileRoot, { recursive: true, force: true }).catch(() => undefined);
      }
    }
  } catch (error) {
    await appendTrace("error", {
      stage: currentStage,
      message:
        typeof error?.message === "string" && error.message.trim()
          ? error.message.trim()
          : String(error),
      code:
        typeof error?.code === "string" && error.code.trim() ? error.code.trim() : null,
    });
    await printJsonAndExit({
      ok: false,
      error: normalizeError(error, currentStage),
    });
  }
}

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeBaseUrl(value) {
  const normalized = normalizeString(value);
  if (!normalized) {
    throw Object.assign(new Error("baseUrl is required."), {
      status: 400,
      code: "producer_browser_missing_base_url",
    });
  }
  return normalized.replace(/\/+$/, "");
}

function normalizeTimeoutMs(value) {
  if (typeof value === "number" && Number.isFinite(value) && value > 0) {
    return Math.max(30_000, Math.floor(value));
  }
  return DEFAULT_TIMEOUT_MS;
}

function normalizeLocale(value) {
  return normalizeString(value)?.split(",")[0]?.trim() ?? null;
}

function buildProducerNavigationTargets(baseUrl, referer) {
  const normalizedBaseUrl = normalizeBaseUrl(baseUrl);
  const targets = [];
  const push = (value) => {
    const normalized = normalizeString(value);
    if (!normalized || targets.includes(normalized)) {
      return;
    }
    targets.push(normalized);
  };
  const isSessionReferer = typeof referer === "string" && /\/session\/[^/?#]+/i.test(referer);
  if (!isSessionReferer) {
    push(referer);
  }
  push(`${normalizedBaseUrl}/library/my-songs`);
  push(`${normalizedBaseUrl}/library/videos`);
  push(normalizedBaseUrl);
  if (isSessionReferer) {
    push(referer);
  }
  return targets;
}

async function openProducerLandingPage(page, navigationTargets, timeoutMs) {
  const errors = [];
  const navigationTimeout = Math.min(timeoutMs, 25_000);
  for (const target of navigationTargets) {
    try {
      await appendTrace("navigation-attempt", {
        target,
        timeoutMs: navigationTimeout,
      });
      await page.goto(target, {
        waitUntil: "commit",
        timeout: navigationTimeout,
      });
      await appendTrace("navigation-success", {
        target,
      });
      return target;
    } catch (error) {
      const message =
        typeof error?.message === "string" && error.message.trim()
          ? error.message.trim()
          : String(error);
      errors.push({ target, message });
      await appendTrace("navigation-failed", {
        target,
        message,
      });
    }
  }

  throw Object.assign(
    new Error(
      `Producer browser worker could not open any landing page: ${JSON.stringify(errors)}`,
    ),
    {
      status: 500,
      code: "producer_browser_navigation_failed",
    },
  );
}

function parseBoolean(value, fallback) {
  const normalized = normalizeString(value)?.toLowerCase();
  if (!normalized) {
    return fallback;
  }
  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }
  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }
  return fallback;
}

function resolveExecutablePath(overridePath) {
  const candidate = normalizeString(overridePath);
  if (candidate && existsSync(candidate)) {
    return candidate;
  }
  const platformPaths =
    process.platform === "win32"
      ? WINDOWS_BROWSER_PATHS
      : process.platform === "darwin"
        ? MACOS_BROWSER_PATHS
        : LINUX_BROWSER_PATHS;
  return platformPaths.find((entry) => existsSync(entry)) ?? null;
}

function resolveBrowserProfileSource(input) {
  const userDataDir =
    normalizeString(input.browserUserDataDir) ??
    normalizeString(process.env.PRODUCER_BROWSER_USER_DATA_DIR) ??
    (existsSync(WINDOWS_CHROME_USER_DATA_DIR)
      ? WINDOWS_CHROME_USER_DATA_DIR
      : existsSync(WINDOWS_EDGE_USER_DATA_DIR)
        ? WINDOWS_EDGE_USER_DATA_DIR
        : null);
  if (!userDataDir || !existsSync(userDataDir)) {
    return null;
  }

  const profileDirectory =
    normalizeString(input.browserProfileDirectory) ??
    normalizeString(process.env.PRODUCER_BROWSER_PROFILE_DIRECTORY) ??
    "Default";
  const profilePath = path.join(userDataDir, profileDirectory);
  if (!existsSync(profilePath)) {
    return null;
  }

  return {
    userDataDir,
    profileDirectory,
  };
}

async function cloneBrowserProfile(userDataDir, profileDirectory) {
  const cloneRoot = await mkdtemp(path.join(os.tmpdir(), "producer-browser-profile-"));
  const profileSource = path.join(userDataDir, profileDirectory);
  const profileTarget = path.join(cloneRoot, profileDirectory);
  await mkdir(profileTarget, { recursive: true });

  const localStateSource = path.join(userDataDir, "Local State");
  if (existsSync(localStateSource)) {
    await copyFile(localStateSource, path.join(cloneRoot, "Local State"));
  }

  for (const fileName of PROFILE_FILES_TO_COPY) {
    const source = path.join(profileSource, fileName);
    if (existsSync(source)) {
      await copyFile(source, path.join(profileTarget, fileName));
    }
  }

  for (const dirName of PROFILE_DIRS_TO_COPY) {
    const source = path.join(profileSource, dirName);
    if (existsSync(source)) {
      await copyPathLoose(source, path.join(profileTarget, dirName));
    }
  }

  return cloneRoot;
}

async function copyPathLoose(source, target) {
  const info = await stat(source).catch(() => null);
  if (!info) {
    return;
  }

  if (info.isDirectory()) {
    await mkdir(target, { recursive: true }).catch(() => undefined);
    const entries = await readdir(source).catch(() => []);
    for (const entry of entries) {
      await copyPathLoose(path.join(source, entry), path.join(target, entry));
    }
    return;
  }

  await mkdir(path.dirname(target), { recursive: true }).catch(() => undefined);
  await copyFile(source, target).catch(() => undefined);
}

function validateInput(input) {
  if (!input || typeof input !== "object") {
    throw Object.assign(new Error("Expected a JSON object on stdin."), {
      status: 400,
      code: "producer_browser_invalid_input",
    });
  }
  if (!normalizeString(input.baseUrl)) {
    throw Object.assign(new Error("baseUrl is required."), {
      status: 400,
      code: "producer_browser_missing_base_url",
    });
  }
  if (!input.requestBody || typeof input.requestBody !== "object") {
    throw Object.assign(new Error("requestBody must be a JSON object."), {
      status: 400,
      code: "producer_browser_missing_request_body",
    });
  }
}

function parseCookieHeader(rawHeader, baseUrl) {
  if (!rawHeader) {
    return [];
  }
  const cookies = [];
  for (const segment of rawHeader.split(";")) {
    const raw = segment.trim();
    if (!raw) {
      continue;
    }
    const separator = raw.indexOf("=");
    if (separator <= 0) {
      continue;
    }
    const name = raw.slice(0, separator).trim();
    const value = raw.slice(separator + 1).trim();
    if (!name || !value) {
      continue;
    }
    cookies.push({
      name,
      value,
      url: `${baseUrl.replace(/\/+$/, "")}/`,
      secure: true,
      sameSite: "Lax",
    });
  }
  return cookies;
}

function extractSupabaseSession(cookieHeader) {
  if (!cookieHeader) {
    return null;
  }
  const cookieMap = new Map();
  for (const segment of cookieHeader.split(";")) {
    const raw = segment.trim();
    if (!raw) {
      continue;
    }
    const separator = raw.indexOf("=");
    if (separator <= 0) {
      continue;
    }
    const name = raw.slice(0, separator).trim();
    const value = raw.slice(separator + 1).trim();
    if (name) {
      cookieMap.set(name, value);
    }
  }

  const chunks = [...cookieMap.entries()]
    .filter(([name]) => /^sb-sb-auth-token\.\d+$/.test(name))
    .sort((left, right) => left[0].localeCompare(right[0], undefined, { numeric: true }))
    .map(([, value]) => safeDecodeURIComponent(value))
    .join("");
  if (!chunks) {
    return null;
  }

  const candidates = [chunks, stripOuterQuotes(chunks), stripOuterQuotes(chunks).replace(/\\"/g, '"')];
  for (const candidate of candidates) {
    const parsed = tryParseJsonDeep(decodeSupabaseCookiePayload(candidate));
    const accessToken =
      parsed?.currentSession?.access_token ??
      parsed?.access_token ??
      parsed?.session?.access_token ??
      null;
    const refreshToken =
      parsed?.currentSession?.refresh_token ??
      parsed?.refresh_token ??
      parsed?.session?.refresh_token ??
      null;
    if (typeof accessToken === "string" && accessToken.trim()) {
      return {
        accessToken: accessToken.trim(),
        refreshToken:
          typeof refreshToken === "string" && refreshToken.trim()
            ? refreshToken.trim()
            : null,
      };
    }
  }

  return null;
}

async function refreshSupabaseAccessToken(refreshToken) {
  const anonKey =
    normalizeString(process.env.PRODUCER_SUPABASE_ANON_KEY) ??
    DEFAULT_PRODUCER_SUPABASE_ANON_KEY;
  const response = await fetch(
    `${DEFAULT_PRODUCER_SUPABASE_URL}/auth/v1/token?grant_type=refresh_token`,
    {
    method: "POST",
    headers: {
      apikey: anonKey,
      authorization: `Bearer ${anonKey}`,
      "content-type": "application/json;charset=UTF-8",
      "x-client-info": "producer-browser-worker/1.0",
    },
    body: JSON.stringify({
      refresh_token: refreshToken,
    }),
    },
  ).catch(() => null);
  if (!response || !response.ok) {
    return null;
  }
  const payload = await response.json().catch(() => null);
  const accessToken = normalizeString(payload?.access_token);
  if (!accessToken) {
    return null;
  }
  return {
    accessToken,
    refreshToken: normalizeString(payload?.refresh_token),
  };
}

function safeDecodeURIComponent(value) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function stripOuterQuotes(value) {
  if (typeof value !== "string") {
    return value;
  }
  return value.replace(/^"+|"+$/g, "");
}

function decodeSupabaseCookiePayload(value) {
  const normalized = stripOuterQuotes(value);
  if (!normalized.startsWith("base64-")) {
    return normalized;
  }
  try {
    return Buffer.from(normalized.slice("base64-".length), "base64").toString("utf8");
  } catch {
    return normalized;
  }
}

function tryParseJsonDeep(value) {
  let current = value;
  for (let index = 0; index < 3; index += 1) {
    if (typeof current !== "string") {
      return current;
    }
    try {
      current = JSON.parse(current);
    } catch {
      return null;
    }
  }
  return current;
}

async function browserContextFetch(page, url, options, timeoutMs = 60_000) {
  return await page.evaluate(
    async ({ url, options, timeoutMs }) => {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
      try {
        const response = await fetch(url, {
          ...options,
          signal: controller.signal,
        });
        const text = await response.text().catch(() => "");
        return {
          status: response.status,
          ok: response.ok,
          text,
          pageUrl: location.href,
          title: document.title,
        };
      } catch (error) {
        const message =
          error && typeof error === "object" && typeof error.message === "string"
            ? error.message
            : String(error);
        const isTimeout =
          error && typeof error === "object" && error.name === "AbortError";
        return {
          status: isTimeout ? 504 : 502,
          ok: false,
          text: message,
          pageUrl: location.href,
          title: document.title,
          fetchError: isTimeout ? "producer_browser_fetch_timeout" : "producer_browser_fetch_failed",
        };
      } finally {
        clearTimeout(timeoutId);
      }
    },
    { url, options, timeoutMs },
  );
}

async function browserContextReadSse(page, url, options, timeoutMs = 60_000) {
  return await page.evaluate(
    async ({ url, options, timeoutMs }) => {
      const controller = new AbortController();
      const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
      try {
        const response = await fetch(url, {
          ...options,
          signal: controller.signal,
        });
        if (!response.ok) {
          const text = await response.text().catch(() => "");
          return {
            status: response.status,
            ok: false,
            text,
            pageUrl: location.href,
            title: document.title,
          };
        }

        if (!response.body || typeof response.body.getReader !== "function") {
          const text = await response.text().catch(() => "");
          return {
            status: response.status,
            ok: response.ok,
            text,
            pageUrl: location.href,
            title: document.title,
          };
        }

        const reader = response.body.getReader();
        const decoder = new TextDecoder();
        let text = "";
        while (true) {
          const { value, done } = await reader.read();
          if (done) {
            break;
          }
          text += decoder.decode(value, { stream: true });
          if (/(^|\n)event:\s*final\b/m.test(text) || /(^|\n)event:\s*error\b/m.test(text)) {
            await reader.cancel().catch(() => undefined);
            break;
          }
        }
        text += decoder.decode();
        return {
          status: response.status,
          ok: response.ok,
          text,
          pageUrl: location.href,
          title: document.title,
        };
      } catch (error) {
        const message =
          error && typeof error === "object" && typeof error.message === "string"
            ? error.message
            : String(error);
        const isTimeout =
          error && typeof error === "object" && error.name === "AbortError";
        return {
          status: isTimeout ? 504 : 502,
          ok: false,
          text: message,
          pageUrl: location.href,
          title: document.title,
          fetchError: isTimeout ? "producer_browser_stream_timeout" : "producer_browser_stream_failed",
        };
      } finally {
        clearTimeout(timeoutId);
      }
    },
    { url, options, timeoutMs },
  );
}

async function nodeSideFetch(url, options = {}, timeoutMs = 60_000) {
  const controller = new AbortController();
  const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
  try {
    const response = await fetch(url, {
      ...options,
      signal: controller.signal,
    });
    const text = await response.text().catch(() => "");
    return {
      status: response.status,
      ok: response.ok,
      text,
    };
  } catch (error) {
    const message =
      error && typeof error === "object" && typeof error.message === "string"
        ? error.message
        : String(error);
    const isTimeout = error && typeof error === "object" && error.name === "AbortError";
    return {
      status: isTimeout ? 504 : 502,
      ok: false,
      text: message,
      fetchError: isTimeout ? "producer_node_fetch_timeout" : "producer_node_fetch_failed",
    };
  } finally {
    clearTimeout(timeoutId);
  }
}

async function delay(ms) {
  await new Promise((resolve) => setTimeout(resolve, ms));
}

function extractConversationIdFromStream(rawText) {
  for (const frame of parseSseFrames(rawText)) {
    if (frame.event !== "conversation_id") {
      continue;
    }
    const data = parseMaybeJson(frame.dataText);
    if (data && typeof data === "object" && typeof data.id === "string" && data.id.trim()) {
      return data.id.trim();
    }
  }
  return null;
}

function parseSseFrames(rawText) {
  const frames = [];
  let eventName = "";
  let dataLines = [];
  const commit = () => {
    if (!eventName && dataLines.length === 0) {
      return;
    }
    frames.push({
      event: eventName || null,
      dataText: dataLines.join("\n"),
    });
    eventName = "";
    dataLines = [];
  };

  for (const rawLine of String(rawText ?? "").split(/\r?\n/)) {
    const line = rawLine.trimEnd();
    if (!line) {
      commit();
      continue;
    }
    if (line.startsWith("event:")) {
      eventName = line.slice(6).trim();
      continue;
    }
    if (line.startsWith("data:")) {
      dataLines.push(line.slice(5).trimStart());
    }
  }

  commit();
  return frames;
}

function parseMaybeJson(value) {
  if (typeof value !== "string") {
    return value ?? null;
  }
  const trimmed = value.trim();
  if (!trimmed) {
    return null;
  }
  try {
    return JSON.parse(trimmed);
  } catch {
    return trimmed;
  }
}

function summarizeConversationStream(rawText) {
  const summary = {
    events: [],
    toolCalls: [],
    toolReturns: [],
    retryPrompts: [],
    suggestions: [],
    messageTexts: [],
  };

  for (const frame of parseSseFrames(rawText)) {
    summary.events.push(frame.event);
    const data = parseMaybeJson(frame.dataText);

    if (frame.event === "part" && data && typeof data === "object") {
      const part = data.part;
      if (part?.part_kind === "tool-call") {
        summary.toolCalls.push({
          toolName: part.tool_name ?? null,
          args: part.args ?? null,
        });
      } else if (part?.part_kind === "tool-return") {
        summary.toolReturns.push({
          toolName: part.tool_name ?? null,
          content: compactProducerToolContent(part.content),
        });
      } else if (
        part?.part_kind === "retry-prompt" &&
        typeof part.content === "string" &&
        part.content.trim()
      ) {
        summary.retryPrompts.push(part.content.trim());
      } else if (part?.part_kind === "text" && typeof part.content === "string" && part.content.trim()) {
        summary.messageTexts.push(part.content.trim());
      }
      continue;
    }

    if (frame.event === "suggestion" && data && typeof data === "object" && Array.isArray(data.parts)) {
      for (const part of data.parts) {
        if (part?.part_kind === "text" && typeof part.content === "string" && part.content.trim()) {
          summary.messageTexts.push(part.content.trim());
        }
        if (part?.part_kind === "tool-call" && part?.tool_name === "synthetic__suggest_actions" && part.args) {
          for (const entry of Object.values(part.args)) {
            if (typeof entry === "string" && entry.trim()) {
              summary.suggestions.push(entry.trim());
            }
          }
        }
      }
    }
  }

  return summary;
}

function collectMediaUrls(value, urls = []) {
  if (typeof value === "string") {
    if (/^https?:\/\//i.test(value) && /\.(mp4|mov)(\?|$)/i.test(value)) {
      urls.push(value);
    }
    return urls;
  }
  if (!value || typeof value !== "object") {
    return urls;
  }
  if (Array.isArray(value)) {
    for (const entry of value) {
      collectMediaUrls(entry, urls);
    }
    return urls;
  }
  for (const entry of Object.values(value)) {
    collectMediaUrls(entry, urls);
  }
  return urls;
}

function readStringFields(value, fields) {
  if (!value || typeof value !== "object") {
    return null;
  }
  for (const field of fields) {
    const normalized = normalizeString(value[field]);
    if (normalized) {
      return normalized;
    }
  }
  return null;
}

function readNumberFields(value, fields) {
  if (!value || typeof value !== "object") {
    return null;
  }
  for (const field of fields) {
    const candidate = value[field];
    if (typeof candidate === "number" && Number.isFinite(candidate)) {
      return candidate;
    }
    if (typeof candidate === "string" && candidate.trim()) {
      const parsed = Number(candidate);
      if (Number.isFinite(parsed)) {
        return parsed;
      }
    }
  }
  return null;
}

function compactProducerToolContent(value, depth = 0) {
  if (value == null || depth >= 4) {
    return value ?? null;
  }

  if (typeof value === "string") {
    return value.length > 240 ? `${value.slice(0, 240)}...<truncated>` : value;
  }

  if (typeof value === "number" || typeof value === "boolean") {
    return value;
  }

  if (Array.isArray(value)) {
    const compacted = value
      .slice(0, 5)
      .map((entry) => compactProducerToolContent(entry, depth + 1));
    if (value.length > 5) {
      compacted.push({ truncatedCount: value.length - 5 });
    }
    return compacted;
  }

  if (typeof value !== "object") {
    return String(value);
  }

  const result = {};
  for (const [key, entry] of Object.entries(value)) {
    if (key === "lyrics_text" && typeof entry === "string") {
      result[key] = {
        preview: entry.slice(0, 120),
        charLength: entry.length,
        truncated: entry.length > 120,
      };
      continue;
    }
    if (key === "char_timestamps" && Array.isArray(entry)) {
      result[key] = {
        count: entry.length,
        sample: entry.slice(0, 3).map((item) => compactProducerToolContent(item, depth + 1)),
      };
      continue;
    }
    result[key] = compactProducerToolContent(entry, depth + 1);
  }
  return result;
}

function buildProducerProposalPrompt(requestBody, creativePrompt) {
  const aspectRatio =
    readStringFields(requestBody, ["aspect_ratio", "aspectRatio", "size"]) ?? "16:9";
  const styleImageUrl = readStringFields(requestBody, [
    "style_image_url",
    "styleImageUrl",
    "style_reference_image_url",
    "styleReferenceImageUrl",
  ]);
  const subjectImageUrl = readStringFields(requestBody, [
    "likeness_image_url",
    "likenessImageUrl",
    "subject_image_url",
    "subjectImageUrl",
  ]);
  const durationSeconds =
    readNumberFields(requestBody, ["duration_s", "durationSeconds", "duration"]) ?? null;
  const renderLyrics =
    typeof requestBody?.render_lyrics === "boolean"
      ? requestBody.render_lyrics
      : typeof requestBody?.renderLyrics === "boolean"
        ? requestBody.renderLyrics
        : typeof requestBody?.display_lyrics === "boolean"
          ? requestBody.display_lyrics
          : typeof requestBody?.displayLyrics === "boolean"
            ? requestBody.displayLyrics
            : false;

  return [
    "Please propose the music video.",
    `Vision: ${creativePrompt}`,
    `Use ${aspectRatio}.`,
    styleImageUrl
      ? `Style reference image: ${styleImageUrl}.`
      : "Style reference image: none.",
    subjectImageUrl
      ? `Subject image: ${subjectImageUrl}.`
      : "Subject image: none, generate one.",
    `Lyrics on screen: ${renderLyrics ? "yes" : "no"}.`,
    durationSeconds
      ? `Duration: use about ${durationSeconds} seconds.`
      : "Duration: choose the strongest section under 60 seconds.",
  ].join(" ");
}

function chooseProducerConfirmPrompt(requestBody, suggestions, messageTexts, proposedInputs = null) {
  const explicit =
    normalizeString(requestBody?.confirm_prompt) ?? normalizeString(requestBody?.confirmPrompt);
  if (explicit) {
    return explicit;
  }

  const proposedStartSeconds =
    readNumberFields(proposedInputs, ["start_s", "startSeconds", "start"]) ??
    readNumberFields(requestBody, ["start_s", "startSeconds", "start"]);
  const proposedDurationSeconds =
    readNumberFields(proposedInputs, ["duration_s", "durationSeconds", "duration"]) ??
    readNumberFields(requestBody, ["duration_s", "durationSeconds", "duration"]);
  const proposedAspectRatio =
    readStringFields(proposedInputs, ["aspect_ratio", "aspectRatio", "size"]) ??
    readStringFields(requestBody, ["aspect_ratio", "aspectRatio", "size"]);
  const proposedResolution =
    readStringFields(proposedInputs, ["resolution"]) ??
    readStringFields(requestBody, ["resolution"]);

  if (proposedStartSeconds != null || proposedDurationSeconds != null || proposedAspectRatio) {
    return [
      "Create this exact proposed music video now.",
      proposedStartSeconds != null ? `Keep the current start time at ${proposedStartSeconds}s.` : null,
      proposedDurationSeconds != null ? `Keep the duration at ${proposedDurationSeconds}s.` : null,
      proposedAspectRatio ? `Keep the aspect ratio at ${proposedAspectRatio}.` : null,
      proposedResolution ? `Keep the resolution at ${proposedResolution}.` : null,
      "Do not ask follow-up questions or change the selected song section.",
    ]
      .filter(Boolean)
      .join(" ");
  }

  const candidateTexts = [
    ...suggestions,
    ...(Array.isArray(messageTexts) ? messageTexts : []),
  ];
  const preferred =
    candidateTexts.find((entry) => /create( the)? (music )?video/i.test(entry)) ??
    candidateTexts.find((entry) => /render/i.test(entry)) ??
    candidateTexts.find((entry) => /create/i.test(entry)) ??
    candidateTexts.find((entry) => /start/i.test(entry));
  return preferred ?? DEFAULT_CONFIRM_PROMPT;
}

async function executeProducerConversationVideoFlow(args) {
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

async function readStdin() {
  return await new Promise((resolve, reject) => {
    const chunks = [];
    process.stdin.setEncoding("utf8");
    process.stdin.on("data", (chunk) => chunks.push(chunk));
    process.stdin.on("end", () => resolve(chunks.join("")));
    process.stdin.on("error", reject);
  });
}

async function printJsonAndExit(value) {
  const payload = JSON.stringify(value);
  await new Promise((resolve, reject) => {
    process.stdout.write(payload, (error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });
  process.exit(0);
}

function normalizeError(error, stage = null) {
  const status =
    typeof error?.status === "number" && Number.isFinite(error.status) ? error.status : 500;
  const normalized = {
    status,
    code:
      typeof error?.code === "string" && error.code.trim()
        ? error.code.trim()
        : "producer_browser_worker_failed",
    message:
      typeof error?.message === "string" && error.message.trim()
        ? error.message.trim()
        : String(error),
    body: typeof error?.body === "string" && error.body.trim() ? error.body : undefined,
  };
  if (stage) {
    normalized.stage = stage;
  }
  return normalized;
}

main();
