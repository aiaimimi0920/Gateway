// Playwright serializes this callback; browser helpers must stay inside it.
export const sendProducerPageConversation = async ({
  baseUrl,
  headers,
  prompt,
  conversationId,
  clientContext,
  modelName,
  timeoutMs,
}) => {
  const controller = new AbortController();
  const readResponseText = async (response, signal) => {
    if (signal.aborted) throw signal.reason;
    if (!response.body) return "";
    const reader = response.body.getReader();
    const limit = 16 * 1024 * 1024;
    let buffer = null;
    let bytes = 0;
    // This callback is serialized independently; its browser reader stays local.
    const cancel = () => { void reader.cancel(signal.reason).catch(() => undefined); };
    signal.addEventListener("abort", cancel, { once: true });
    try {
      while (true) {
        if (signal.aborted) throw signal.reason;
        const { value, done } = await reader.read();
        if (signal.aborted) throw signal.reason;
        if (done) break;
        if (value.byteLength > limit - bytes) {
          throw Object.assign(new Error("Producer conversation response exceeds 16 MiB."), {
            code: "producer_browser_conversation_response_too_large",
          });
        }
        if (!value.byteLength) continue;
        const required = bytes + value.byteLength;
        if (!buffer || required > buffer.byteLength) {
          const capacity = Math.min(
            limit,
            Math.max(64 * 1024, required, (buffer?.byteLength ?? 0) * 2),
          );
          const grown = new Uint8Array(capacity);
          if (buffer) grown.set(buffer.subarray(0, bytes));
          buffer = grown;
        }
        buffer.set(value, bytes);
        bytes += value.byteLength;
      }
      return buffer ? new TextDecoder().decode(buffer.subarray(0, bytes)) : "";
    } finally {
      signal.removeEventListener("abort", cancel);
      await reader.cancel().catch(() => undefined);
      reader.releaseLock();
    }
  };
  const normalizeText = (value) =>
    typeof value === "string" && value.trim() ? value.trim() : null;

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
    let lineCount = 0;
    const rejectComplexStream = () => {
      throw Object.assign(new Error("Producer stream exceeds its frame or line budget."), {
        code: "producer_browser_stream_too_complex",
      });
    };
    const commit = () => {
      if (!eventName && dataLines.length === 0) {
        return;
      }
      if (frames.length >= 4096) rejectComplexStream();
      frames.push({
        event: eventName || null,
        dataText: dataLines.join("\n"),
      });
      eventName = "";
      dataLines = [];
    };

    // Keep allocation bounded before rejecting excessive frame or line counts.
    for (const match of String(rawText ?? "").matchAll(/[^\r\n]*(?:\r\n|\r|\n|$)/g)) {
      if (!match[0]) continue;
      if (++lineCount > 65536) rejectComplexStream();
      const line = match[0].trimEnd();
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

  const readConversationStream = async ({ jobId, headers }) => {
    const response = await fetch(`${baseUrl}/__api/messages/${jobId}/stream?last_id=0`, {
      method: "GET",
      credentials: "include",
      redirect: "error",
      headers,
      signal: controller.signal,
    });
    const rawText = await readResponseText(response, controller.signal);
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
      if (frame.event === "message" || frame.event === "part") {
        const message = data && typeof data === "object"
          ? frame.event === "part" ? { parts: [data.part] } : data
          : null;
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
  };

  const sendConversationMessage = async ({
    baseUrl,
    headers,
    prompt,
    conversationId,
    clientContext,
    modelName,
  }) => {
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
      redirect: "error",
      headers,
      body: JSON.stringify(body),
      signal: controller.signal,
    });
    const bodyText = await readResponseText(response, controller.signal);
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
    });
    if (!streamResult.ok) {
      return streamResult;
    }

    return {
      ok: true,
      jobId,
      stream: streamResult.result,
    };
  };

  // One elapsed budget covers both the submission and its subsequent stream.
  const timeoutId = setTimeout(() => controller.abort(), timeoutMs);
  try {
    return await sendConversationMessage({ baseUrl, headers, prompt, conversationId, clientContext, modelName });
  } catch (error) {
    const timedOut = controller.signal.aborted;
    const tooLarge = error?.code === "producer_browser_conversation_response_too_large";
    const tooComplex = error?.code === "producer_browser_stream_too_complex";
    return {
      ok: false,
      error: {
        status: timedOut ? 504 : 502,
        code: timedOut ? "producer_browser_conversation_timeout"
          : tooLarge ? "producer_browser_conversation_response_too_large"
          : tooComplex ? "producer_browser_stream_too_complex"
          : "producer_browser_conversation_fetch_failed",
        message: timedOut ? "Producer conversation request timed out."
          : tooLarge ? "Producer conversation response exceeds 16 MiB."
          : tooComplex ? "Producer stream exceeds its frame or line budget."
          : "Producer conversation transport failed.",
      },
    };
  } finally {
    clearTimeout(timeoutId);
  }
};
