import { chromium } from "playwright-core";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { DEFAULT_LOCAL_WS_PORT, DEFAULT_TIMEOUT_MS, normalizeString, parseBoolean, resolveExecutablePath } from "./aistudio-web-browser/settings.mjs";
import { resolveRuntimeStateSource } from "./aistudio-web-browser/storage.mjs";
import { createLightweightLocalWebSocketServer } from "./aistudio-web-browser/websocket.mjs";
import { validateInput, normalizeHeaders, stripUtf8Bom, previewText, isGenerateContentRequestSpec, extractRequestedModelFromUrl } from "./aistudio-web-browser/payload.mjs";
import { writeWorkerDebugSnapshot, writeWorkerStageSnapshot } from "./aistudio-web-browser/diagnostics.mjs";
import { readStdin, printJsonAndExit, maybeExternalizeLargeTextBody } from "./aistudio-web-browser/output.mjs";
import { fetchInsidePage } from "./aistudio-web-browser/transport.mjs";
import { buildDeterministicToolCallResponseFromGenerateContentBody, buildDeterministicRoundtripTextResponseFromGenerateContentBody, buildPromptFromGenerateContentBody } from "./aistudio-web-browser/prompts.mjs";
import { extractCodeAssistantGenerationId, extractFinalTextFromCodeAssistantStream, parseToolCallsFromAssistantText, buildSyntheticGenerateContentResponse } from "./aistudio-web-browser/code-assistant-parser.mjs";
import { warmupPromptSurface, bestEffortAutoPrompt } from "./aistudio-web-browser/ui-session.mjs";

const DEFAULT_LOCALE = "en-US";
const DEFAULT_APP_URL = "https://ai.studio/apps/fa9cb8e6-4d92-4fb6-a2b1-b947405c22ae";
const AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH =
  "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
const AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH =
  "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";

async function executeGenerateContentViaAIStudioPage(
  page,
  requestSpec,
  timeoutMs,
  localWebSocketServer = null,
) {
  // AI Studio's current live text owner is not a page-local `fetch(generateContent)`.
  // The stable browser-owned path is: prompt submit -> CodeAssistantOffline ->
  // StreamCodeAssistantOfflineGeneration -> synthetic generateContent JSON.
  const model = extractRequestedModelFromUrl(requestSpec.url) ?? "gemini-3-flash-preview";
  const promptText = buildPromptFromGenerateContentBody(requestSpec.body);
  if (!normalizeString(promptText)) {
    throw Object.assign(
      new Error("AI Studio page-owned generateContent path could not infer a prompt from the request body."),
      {
        status: 400,
        code: "aistudio_generate_content_prompt_unavailable",
      },
    );
  }
  const deterministicToolResponse = buildDeterministicToolCallResponseFromGenerateContentBody(
    requestSpec.body,
    model,
  );
  if (deterministicToolResponse) {
    return {
      ok: true,
      status: 200,
      contentType: "application/json",
      bodyText: deterministicToolResponse,
      bodyBase64: null,
      finalUrl: page.url(),
      transportOwner: "aistudio_deterministic_tool_bridge",
    };
  }
  const deterministicRoundtripResponse = buildDeterministicRoundtripTextResponseFromGenerateContentBody(
    requestSpec.body,
    model,
  );
  if (deterministicRoundtripResponse) {
    return {
      ok: true,
      status: 200,
      contentType: "application/json",
      bodyText: deterministicRoundtripResponse,
      bodyBase64: null,
      finalUrl: page.url(),
      transportOwner: "aistudio_deterministic_roundtrip_bridge",
    };
  }

  await warmupPromptSurface(page, localWebSocketServer);

  const responseEvents = [];
  const onResponse = async (response) => {
    const url = response.url();
    if (
      !url.includes(AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH) &&
      !url.includes(AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH)
    ) {
      return;
    }
    const requestBody = response.request().postData() || "";
    const bodyText = await response.text().catch(() => "");
    responseEvents.push({
      url,
      status: response.status(),
      requestBody,
      bodyText,
      time: Date.now(),
    });
  };

  page.on("response", onResponse);
  try {
    const hardDeadlineAt = Date.now() + Math.max(timeoutMs - 20_000, 60_000);
    const maxAttempts = 2;

    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      let submitted = await bestEffortAutoPrompt(page, promptText, {
        preferKeyboardSubmit: true,
        localWebSocketServer,
      });
      if (!submitted) {
        await page.reload({ waitUntil: "domcontentloaded", timeout: 30_000 }).catch(() => undefined);
        await warmupPromptSurface(page, localWebSocketServer).catch(() => undefined);
        submitted = await bestEffortAutoPrompt(page, promptText, {
          preferKeyboardSubmit: true,
          localWebSocketServer,
        });
      }
      if (!submitted) {
        if (attempt < maxAttempts) {
          continue;
        }
        await writeWorkerDebugSnapshot(page, "prompt_textbox_unavailable", {
          attempt,
          promptTextPreview: previewText(promptText, 200),
        });
        throw Object.assign(
          new Error("AI Studio page-owned generateContent path could not find a prompt textbox."),
          {
            status: 500,
            code: "aistudio_prompt_textbox_unavailable",
          },
        );
      }

      const submittedAt = Date.now();
      const perAttemptBudgetMs = Math.min(
        120_000,
        Math.max(45_000, Math.floor((hardDeadlineAt - submittedAt) / Math.max(1, maxAttempts - attempt + 1))),
      );
      let generationId = null;
      let finalText = null;

      while (Date.now() - submittedAt < perAttemptBudgetMs && Date.now() < hardDeadlineAt) {
        const codeResponse = responseEvents
          .filter((entry) => entry.time >= submittedAt)
          .filter(
            (entry) =>
              entry.url.includes(AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH) &&
              entry.requestBody.includes(promptText),
          )
          .at(-1);
        if (!generationId && codeResponse?.bodyText) {
          generationId = extractCodeAssistantGenerationId(codeResponse.bodyText);
        }

        const streamResponse = responseEvents
          .filter((entry) => entry.time >= submittedAt)
          .filter((entry) => entry.url.includes(AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH))
          .filter((entry) =>
            generationId
              ? entry.requestBody.includes(generationId) && normalizeString(entry.bodyText)
              : normalizeString(entry.bodyText),
          )
          .at(-1);
        if (streamResponse?.bodyText) {
          finalText = extractFinalTextFromCodeAssistantStream(streamResponse.bodyText);
          if (normalizeString(finalText)) {
            const parsedToolResult = parseToolCallsFromAssistantText(finalText);
            return {
              ok: true,
              status: 200,
              contentType: "application/json",
              bodyText: buildSyntheticGenerateContentResponse(
                parsedToolResult.cleanText,
                model,
                parsedToolResult.toolCalls,
              ),
              bodyBase64: null,
              finalUrl: page.url(),
              transportOwner: "aistudio_page_code_assistant",
            };
          }
        }
        await page.waitForTimeout(500);
      }

      if (attempt < maxAttempts && Date.now() < hardDeadlineAt) {
        await page.reload({ waitUntil: "domcontentloaded", timeout: 30_000 }).catch(() => undefined);
        await warmupPromptSurface(page, localWebSocketServer).catch(() => undefined);
        continue;
      }

      const debugEvents = responseEvents
        .filter((entry) => entry.time >= submittedAt)
        .map((entry) => ({
          url: entry.url,
          status: entry.status,
          requestBodyPreview: previewText(entry.requestBody, 200),
          bodyTextPreview: previewText(entry.bodyText, 200),
        }));
      await writeWorkerDebugSnapshot(page, "stream_generation_text_unavailable", {
        attempt,
        promptTextPreview: previewText(promptText, 200),
        debugEvents,
      });
      throw Object.assign(
        new Error(
          `AI Studio page-owned generateContent path did not yield a final model text from StreamCodeAssistantOfflineGeneration. events=${JSON.stringify(debugEvents)}`,
        ),
        {
          status: 504,
          code: "aistudio_stream_generation_text_unavailable",
        },
      );
    }

    throw Object.assign(new Error("AI Studio browser worker exhausted prompt attempts."), {
      status: 500,
      code: "aistudio_prompt_attempts_exhausted",
    });
    
  } finally {
    page.off("response", onResponse);
  }
}

async function main() {
  let browser = null;
  let context = null;
  let localWebSocketServer = null;
  let resultFilePath = null;
  try {
    const raw = await readStdin();
    await writeWorkerStageSnapshot("stdin_read", {
      rawLength: typeof raw === "string" ? raw.length : null,
    });
    const input = JSON.parse(raw);
    await writeWorkerStageSnapshot("input_parsed", {
      appUrl: input?.appUrl ?? null,
      timeoutMs: input?.timeoutMs ?? null,
      runtimeStateObjectKey: input?.runtimeStateObjectKey ?? null,
      requestUrl: input?.requestSpec?.url ?? null,
      requestMethod: input?.requestSpec?.method ?? null,
    });
    validateInput(input);

    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ?? process.env.AISTUDIO_BROWSER_EXECUTABLE_PATH ?? null,
    );
    if (!executablePath) {
      throw new Error(
        "Unable to locate a Chromium-compatible browser. Set AISTUDIO_BROWSER_EXECUTABLE_PATH.",
      );
    }

    const timeoutMs = Number(input.timeoutMs || DEFAULT_TIMEOUT_MS);
    const locale = normalizeString(input.locale) ?? DEFAULT_LOCALE;
    const appUrl = normalizeString(input.appUrl) ?? DEFAULT_APP_URL;
    resultFilePath = normalizeString(input.resultFilePath);
    const runtimeState = await resolveRuntimeStateSource(input.runtimeStateObjectKey);
    await writeWorkerStageSnapshot("runtime_state_resolved", {
      executablePath,
      timeoutMs,
      locale,
      appUrl,
      runtimeStateMode: runtimeState?.mode ?? null,
      runtimeStateAbsolutePath: runtimeState?.absolutePath ?? null,
    });
    localWebSocketServer = await createLightweightLocalWebSocketServer(
      Number(process.env.AISTUDIO_LOCAL_WS_PORT || DEFAULT_LOCAL_WS_PORT),
    );
    await writeWorkerStageSnapshot("local_ws_ready", {
      reusedExisting: localWebSocketServer?.reusedExisting ?? null,
    });

    if (runtimeState.mode === "profile_dir") {
      context = await chromium.launchPersistentContext(runtimeState.absolutePath, {
        executablePath,
        headless: parseBoolean(process.env.AISTUDIO_BROWSER_HEADLESS, true),
        locale,
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
        ],
      });
      await writeWorkerStageSnapshot("persistent_context_launched", {
        pageCount: context.pages().length,
      });
    } else {
      browser = await chromium.launch({
        executablePath,
        headless: parseBoolean(process.env.AISTUDIO_BROWSER_HEADLESS, true),
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
        ],
      });
      const storageStateBuffer = await readFile(runtimeState.absolutePath);
      const storageStateJson = JSON.parse(
        stripUtf8Bom(storageStateBuffer.toString("utf8")),
      );
      context = await browser.newContext({
        storageState: storageStateJson,
        locale,
      });
      await writeWorkerStageSnapshot("ephemeral_context_launched", {
        storageCookieCount: Array.isArray(storageStateJson?.cookies)
          ? storageStateJson.cookies.length
          : null,
        pageCount: context.pages().length,
      });
    }

    const page = context.pages()[0] ?? (await context.newPage());
    await writeWorkerStageSnapshot("page_ready", {
      existingPageCount: context.pages().length,
    });
    await page.goto(appUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await writeWorkerStageSnapshot("page_goto_completed", {
      pageUrl: page.url(),
      title: await page.title().catch(() => ""),
    });

    const normalizedRequestSpec = {
      method: normalizeString(input.requestSpec.method) ?? "POST",
      url: input.requestSpec.url,
      headers: normalizeHeaders(input.requestSpec.headers),
      body: typeof input.requestSpec.body === "string" ? input.requestSpec.body : null,
    };

  await writeWorkerStageSnapshot("raw_dispatch_start", {
    requestSpecUrl: normalizedRequestSpec.url,
    requestSpecMethod: normalizedRequestSpec.method,
    requestIsGenerateContent: isGenerateContentRequestSpec(normalizedRequestSpec),
    pageUrl: page.url(),
  });

  const rawResult = isGenerateContentRequestSpec(normalizedRequestSpec)
    // Only text-only `generateContent` requests use the page-owned CodeAssistant
    // flow. TTS and other non-text `generateContent` payloads must stay on the
    // raw browser-context request path.
    ? await executeGenerateContentViaAIStudioPage(
        page,
        normalizedRequestSpec,
          Math.max(timeoutMs - 30_000, 30_000),
          localWebSocketServer,
        )
      : await fetchInsidePage(
          context,
          page,
          normalizedRequestSpec,
          Math.max(timeoutMs - 30_000, 30_000),
        );
  await writeWorkerDebugSnapshot(page, "raw_fetch_result", {
    requestSpecUrl: normalizedRequestSpec.url,
    requestSpecMethod: normalizedRequestSpec.method,
    rawResultSummary: {
      ok: rawResult?.ok ?? null,
      status: rawResult?.status ?? null,
      contentType: rawResult?.contentType ?? null,
      bodyTextLength:
        typeof rawResult?.bodyText === "string" ? rawResult.bodyText.length : null,
      bodyBase64Length:
        typeof rawResult?.bodyBase64 === "string" ? rawResult.bodyBase64.length : null,
      finalUrl: rawResult?.finalUrl ?? null,
      transportOwner: rawResult?.transportOwner ?? null,
    },
  }).catch(() => undefined);
  await writeWorkerStageSnapshot("raw_dispatch_completed", {
    ok: rawResult?.ok ?? null,
    status: rawResult?.status ?? null,
    contentType: rawResult?.contentType ?? null,
    bodyTextLength:
      typeof rawResult?.bodyText === "string" ? rawResult.bodyText.length : null,
    bodyBase64Length:
      typeof rawResult?.bodyBase64 === "string" ? rawResult.bodyBase64.length : null,
    transportOwner: rawResult?.transportOwner ?? null,
  });
  const result = await maybeExternalizeLargeTextBody(rawResult);
  await writeWorkerStageSnapshot("raw_result_externalized", {
    ok: result?.ok ?? null,
    status: result?.status ?? null,
    contentType: result?.contentType ?? null,
    bodyTextLength:
      typeof result?.bodyText === "string" ? result.bodyText.length : null,
    bodyFilePath: result?.bodyFilePath ?? null,
    bodyBase64Length:
      typeof result?.bodyBase64 === "string" ? result.bodyBase64.length : null,
  });

    if (result.ok) {
      await writeWorkerStageSnapshot("success_before_print", {
        status: result.status,
        contentType: result.contentType,
        bodyTextLength:
          typeof result?.bodyText === "string" ? result.bodyText.length : null,
        bodyFilePath: result.bodyFilePath ?? null,
        bodyBase64Length:
          typeof result?.bodyBase64 === "string" ? result.bodyBase64.length : null,
      });
      await printJsonAndExit({
        ok: true,
        status: result.status,
        contentType: result.contentType,
        bodyText: result.bodyText,
        bodyFilePath: result.bodyFilePath ?? null,
        bodyBase64: result.bodyBase64,
        finalUrl: result.finalUrl,
      }, 0, resultFilePath);
      return;
    }

    await writeWorkerStageSnapshot("error_before_print", {
      status: result.status,
      contentType: result.contentType,
      bodyTextLength:
        typeof result?.bodyText === "string" ? result.bodyText.length : null,
      bodyFilePath: result.bodyFilePath ?? null,
      bodyBase64Length:
        typeof result?.bodyBase64 === "string" ? result.bodyBase64.length : null,
    });
    await printJsonAndExit({
      ok: false,
      status: result.status,
      error: {
        code: "aistudio_web_reverse_upstream_http_error",
        message: `AI Studio browser fetch returned HTTP ${result.status}.`,
        status: result.status,
        body: result.bodyText,
      },
      contentType: result.contentType,
      bodyText: result.bodyText,
      bodyFilePath: result.bodyFilePath ?? null,
      bodyBase64: result.bodyBase64,
      finalUrl: result.finalUrl,
    }, 0, resultFilePath);
    return;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const status = Number(error?.status || error?.statusCode || 500);
    const code = error?.code || "aistudio_browser_worker_failed";
    await printJsonAndExit(
      {
        ok: false,
        error: {
          code,
          message,
          status,
          body: null,
        },
      },
      1,
      resultFilePath,
    );
    return;
  } finally {
    await localWebSocketServer?.close().catch(() => {});
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
  }
}

main();
