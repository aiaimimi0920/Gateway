import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { submitPrompt } from "./gemini-canvas-browser-pool-composer.mjs";

export function createTextOperationOwner({
  DEFAULT_TIMEOUT_MS, operationConfig, resolveProgramPageUrl, startNetworkCapture,
  resetConversation, buildProgramHandleState, log,
}) {
  function isTransientAssistantStatusLine(line) {
    const normalized = String(line || "").trim();
    if (
      /^(显示思路|Show thinking|Gemini 说|Gemini (?:says?|said)|复制提示|Copy prompt|修改|Modify|重做|Redo|听回答|Listen|答得好|答得不好|立即回答)$/i.test(
        normalized,
      )
    ) {
      return true;
    }
    return /^(Analyzing|Assessing|Checking|Clarifying|Considering|Crafting|Determining|Evaluating|Examining|Exploring|Formulating|Gathering|Identifying|Interpreting|Pinpointing|Planning|Preparing|Reasoning|Refining|Resolving|Reviewing|Searching|Synthesizing|Thinking|Understanding|Verifying)(?:\s+[A-Za-z][A-Za-z'-]*){0,6}$/i.test(
      normalized,
    );
  }

  function normalizeAssistantText(text) {
    const lines = String(text || "")
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter(Boolean)
      .filter((line) => !isTransientAssistantStatusLine(line));
    return lines.join("\n").trim();
  }

  function promptLooksLikeGreeting(prompt) {
    const normalized = String(prompt || "").trim().toLowerCase();
    return /^(?:hi|hello|hey|你好|您好|嗨)[!！。.\s]*$/i.test(normalized);
  }

  function textLooksLikeGenericWelcome(prompt, text) {
    if (promptLooksLikeGreeting(prompt)) {
      return false;
    }
    const normalized = String(text || "").replace(/\s+/g, " ").trim().toLowerCase();
    return (
      /hello[!,]?\s*(?:it looks like [^.!?]+[.!?]\s*)?how can i help you today/.test(normalized)
      || normalized.startsWith("how can i help you today")
      || /feel free to (?:ask a question|share a piece of writing)/.test(normalized)
      || /what project (?:you would|you'd|you are|you're) like to work on/.test(normalized)
      || /meet gemini, your personal ai assistant/.test(normalized)
      || /认识 gemini[：:]?你的私人 ai 助理/.test(normalized)
    );
  }

  function bodyContainsSubmittedPrompt(bodyText, prompt) {
    const compact = (value) => String(value || "").replace(/\s+/g, " ").trim();
    const normalizedPrompt = compact(prompt);
    if (!normalizedPrompt) {
      return false;
    }
    const anchor = normalizedPrompt.slice(0, 120);
    return compact(bodyText).includes(anchor);
  }

  function bodyIndicatesGenerationInProgress(bodyText) {
    const text = String(bodyText || "");
    if (!text) {
      return false;
    }
    return /(Gemini 正在输入|Gemini is typing|正在输入|is typing|正在思考|Thinking|正在生成|Generating)/i.test(
      text,
    );
  }

  function extractExactAnswerDirective(prompt) {
    const text = String(prompt || "").trim();
    if (!text) {
      return null;
    }
    const lowered = text.toLowerCase();
    const markers = [
      "reply with exactly:",
      "return exactly:",
      "output exactly:",
      "respond with exactly:",
    ];
    for (const marker of markers) {
      const index = lowered.indexOf(marker);
      if (index === -1) {
        continue;
      }
      const remainder = text.slice(index + marker.length).trim();
      if (!remainder) {
        continue;
      }
      const firstLine = remainder
        .split(/\r?\n/)
        .map((line) => line.trim().replace(/^["`]+|["`]+$/g, "").trim())
        .find(Boolean);
      if (firstLine) {
        return firstLine;
      }
    }
    return null;
  }

  function augmentToolHistoryPrompt(prompt) {
    const text = String(prompt || "");
    if (!text) {
      return text;
    }
    const hasToolHistory =
      /Caller-provided result for\s+`[^`]+`:/i.test(text) ||
      /The caller has already executed every required external tool\./i.test(text);
    if (!hasToolHistory || /Required exact final answer:/i.test(text)) {
      return text;
    }
    const exactAnswer = extractExactAnswerDirective(text);
    if (!exactAnswer) {
      return text;
    }
    return `${text}\n\nRequired exact final answer:\n${exactAnswer}\n\nReturn exactly that text and nothing else.`;
  }

  async function collectTextSnapshot(page) {
    return page.evaluate(() => {
      const collectTexts = (selector) =>
        Array.from(document.querySelectorAll(selector))
          .map((node) => (node.innerText || "").trim())
          .filter(Boolean);

      const primaryTexts = collectTexts("message-content");
      const fallbackTexts = collectTexts(".markdown.markdown-main-panel, model-response");
      const sendButton = Array.from(document.querySelectorAll("button")).find((node) =>
        /(发送|Send)/i.test((node.innerText || node.getAttribute("aria-label") || "").trim()),
      );

      return {
        url: location.href,
        bodyText: (document.body?.innerText ?? "").slice(0, 12000),
        primaryTexts,
        fallbackTexts,
        sendDisabled: Boolean(sendButton?.disabled),
      };
    });
  }

  async function runTextOperation(entry, args) {
    const prompt = normalizeString(args.prompt);
    if (!prompt) {
      throw Object.assign(
        new Error("Gemini Canvas text invocation requires a non-empty prompt."),
        {
          status: 400,
          code: "gemini_canvas_invalid_text_prompt",
        },
      );
    }

    const timeoutMs = Math.max(
      Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
      operationConfig("text").resultTimeoutMs,
    );
    const effectivePrompt = augmentToolHistoryPrompt(prompt);
    const page = entry.page;
    const preferredProgramPageUrl = resolveProgramPageUrl(
      normalizeString(args.baseUrl) ?? "https://gemini.google.com",
      args,
    );
    const capture = startNetworkCapture(page, "text");
    log("text operation starting", effectivePrompt.slice(0, 160));
    try {
      await capture.ready;
      await resetConversation(
        page,
        normalizeString(args.baseUrl) ?? "https://gemini.google.com",
        timeoutMs,
        preferredProgramPageUrl,
      );
      log("text conversation reset complete", page.url());
      const baseline = await collectTextSnapshot(page);
      const baselineTexts =
        baseline.primaryTexts.length > 0 ? baseline.primaryTexts : baseline.fallbackTexts;
      const baselineLastText = normalizeAssistantText(baselineTexts.at(-1) || "");
      const selectLatestCandidate = (snapshot) => {
        const candidateTexts =
          snapshot.primaryTexts.length > 0 ? snapshot.primaryTexts : snapshot.fallbackTexts;
        const rawCandidate = candidateTexts.at(-1) || "";
        const candidate = normalizeAssistantText(rawCandidate);
        const submittedPromptObserved = bodyContainsSubmittedPrompt(
          snapshot.bodyText,
          effectivePrompt,
        );
        const isNewResponse =
          Boolean(candidate) &&
          submittedPromptObserved &&
          !textLooksLikeGenericWelcome(effectivePrompt, candidate) &&
          (candidateTexts.length > baselineTexts.length || candidate !== baselineLastText);
        return { candidate, candidateTexts, isNewResponse, submittedPromptObserved };
      };

      log("text submitting prompt");
      await submitPrompt(page, effectivePrompt, timeoutMs);
      log("text prompt submitted");

      const deadline = Date.now() + timeoutMs;
      let lastStableText = null;
      let stableHits = 0;
      let lastSnapshot = baseline;
      let pollCount = 0;
      while (Date.now() < deadline) {
        pollCount += 1;
        lastSnapshot = await collectTextSnapshot(page);
        const { candidate, isNewResponse, submittedPromptObserved } =
          selectLatestCandidate(lastSnapshot);
        const responseInProgress = bodyIndicatesGenerationInProgress(lastSnapshot.bodyText);

        if (pollCount <= 3 || pollCount % 10 === 0) {
          log(
            "text poll snapshot",
            JSON.stringify({
              pollCount,
              primaryCount: lastSnapshot.primaryTexts.length,
              fallbackCount: lastSnapshot.fallbackTexts.length,
              sendDisabled: lastSnapshot.sendDisabled,
              responseInProgress,
              submittedPromptObserved,
              bodyPreview: String(lastSnapshot.bodyText || "").slice(0, 240),
            }),
          );
        }

        if (isNewResponse) {
          if (candidate === lastStableText) {
            stableHits += 1;
          } else {
            lastStableText = candidate;
            stableHits = 1;
          }
          const looksLikeToolPayload = /<tool_calls>|<function_calls>|<invoke\b/i.test(candidate);
          const looksSubstantive = candidate.length >= 24 || looksLikeToolPayload;
          if (pollCount <= 3 || pollCount % 10 === 0) {
            log(
              "text candidate observed",
              JSON.stringify({
                pollCount,
                stableHits,
                sendDisabled: lastSnapshot.sendDisabled,
                responseInProgress,
                looksSubstantive,
                candidatePreview: candidate.slice(0, 120),
              }),
            );
          }
          if (
            stableHits >= 2 &&
            (lastSnapshot.sendDisabled || looksSubstantive || !responseInProgress)
          ) {
            return {
              operation: "text",
              pageUrl: lastSnapshot.url,
              bodyText: lastSnapshot.bodyText,
              ...buildProgramHandleState(
                normalizeString(args.baseUrl) ?? "https://gemini.google.com",
                args,
                lastSnapshot.url,
                capture.state,
              ),
              text: candidate,
              media: [],
              networkEvents: capture.state.events,
              rpcCaptures: capture.state.rpcCaptures,
            };
          }
        }

        await page.waitForTimeout(1500);
      }

      const fallback = selectLatestCandidate(lastSnapshot);
      if (fallback.isNewResponse && fallback.candidate) {
        log("text operation returning fallback candidate", fallback.candidate.slice(0, 160));
        return {
          operation: "text",
          pageUrl: lastSnapshot.url,
          bodyText: lastSnapshot.bodyText,
          ...buildProgramHandleState(
            normalizeString(args.baseUrl) ?? "https://gemini.google.com",
            args,
            lastSnapshot.url,
            capture.state,
          ),
          text: fallback.candidate,
          media: [],
          networkEvents: capture.state.events,
          rpcCaptures: capture.state.rpcCaptures,
        };
      }

      log(
        "text operation timed out",
        JSON.stringify({
          pageUrl: lastSnapshot?.url ?? null,
          bodyPreview: String(lastSnapshot?.bodyText ?? "").slice(0, 240),
        }),
      );
      throw Object.assign(new Error("Timed out waiting for Gemini Canvas text response."), {
        status: 504,
        code: "gemini_canvas_text_timeout",
        bodyText: lastSnapshot?.bodyText ?? null,
        captureState: capture.state,
      });
    } finally {
      await capture.stop();
    }
  }

  return { isTransientAssistantStatusLine, normalizeAssistantText, promptLooksLikeGreeting, textLooksLikeGenericWelcome, bodyContainsSubmittedPrompt, bodyIndicatesGenerationInProgress, extractExactAnswerDirective, augmentToolHistoryPrompt, collectTextSnapshot, runTextOperation };
}
