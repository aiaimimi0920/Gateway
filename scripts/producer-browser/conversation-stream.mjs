export function extractConversationIdFromStream(rawText) {
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

export function parseSseFrames(rawText) {
  const frames = [];
  let eventName = "";
  let dataLines = [];
  let lineCount = 0;
  const rejectComplexStream = () => {
    throw Object.assign(new Error("Producer stream exceeds its frame or line budget."), {
      status: 502, code: "producer_browser_stream_too_complex",
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

  // Iterate lazily so rejected inputs never allocate an array of every line.
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
}

export function parseMaybeJson(value) {
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

export function summarizeConversationStream(rawText) {
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

export function collectMediaUrls(value, urls = []) {
  const fail = () => {
    throw Object.assign(new Error("Producer media payload exceeds traversal limits."), {
      status: 502,
      code: "producer_media_payload_too_complex",
    });
  };
  if (urls.length > 1024) fail();
  const stack = [{ value, depth: 0 }];
  const active = new WeakSet();
  let scheduled = 1;
  while (stack.length) {
    const item = stack.pop();
    const current = item.value;
    if (item.exit) {
      active.delete(current);
      continue;
    }
    if (item.depth > 64) fail();
    if (typeof current === "string") {
      if (/^https?:\/\//i.test(current) && /\.(mp4|mov)(\?|$)/i.test(current)) {
        if (urls.length >= 1024) fail();
        urls.push(current);
      }
      continue;
    }
    if (!current || typeof current !== "object") continue;
    if (active.has(current)) fail();
    const children = Array.isArray(current) ? current : [];
    if (!Array.isArray(current)) {
      for (const key in current) {
        if (!Object.prototype.hasOwnProperty.call(current, key)) continue;
        if (children.length >= 100_000 - scheduled) fail();
        children.push(current[key]);
      }
    }
    if (children.length > 100_000 - scheduled) fail();
    scheduled += children.length;
    active.add(current);
    stack.push({ value: current, exit: true });
    // Reverse pushes preserve depth-first order; only ancestor cycles are rejected.
    for (let index = children.length - 1; index >= 0; index--) {
      stack.push({ value: children[index], depth: item.depth + 1 });
    }
  }
  return urls;
}

export function compactProducerToolContent(value, depth = 0) {
  let remaining = 256;
  const visit = (current, level) => {
    if (remaining <= 0) return "<truncated>";
    remaining -= 1;
    if (current == null) return null;
    if (typeof current === "string") {
      return current.length > 240 ? `${current.slice(0, 240)}...<truncated>` : current;
    }
    if (typeof current === "number" || typeof current === "boolean") return current;
    if (typeof current !== "object") return String(current).slice(0, 240);
    // Returning the original object here would bypass every summary bound.
    if (level >= 4) return "<truncated>";
    if (Array.isArray(current)) {
      const compacted = current.slice(0, 5).map((entry) => visit(entry, level + 1));
      if (current.length > 5) compacted.push({ truncatedCount: current.length - 5 });
      return compacted;
    }
    const result = Object.create(null);
    let entries = 0;
    // The video flow consumes these root fields; diagnostic detail must not crowd them out.
    if (level === depth) {
      for (const key of ["job_id", "jobId"]) {
        if (Object.prototype.hasOwnProperty.call(current, key)) {
          result[key] = visit(current[key], level + 1);
          entries += 1;
        }
      }
    }
    for (const key in current) {
      if (!Object.prototype.hasOwnProperty.call(current, key)) continue;
      if (Object.prototype.hasOwnProperty.call(result, key)) continue;
      if (entries++ >= 32 || remaining <= 0) {
        result["<truncated>"] = true;
        break;
      }
      if (key.length > 120) {
        result["<truncated>"] = true;
        continue;
      }
      const entry = current[key];
      if (key === "lyrics_text" && typeof entry === "string") {
        remaining -= 1;
        result[key] = {
          preview: entry.slice(0, 120),
          charLength: entry.length,
          truncated: entry.length > 120,
        };
      } else if (key === "char_timestamps" && Array.isArray(entry)) {
        remaining -= 1;
        result[key] = {
          count: entry.length,
          sample: entry.slice(0, 3).map((item) => visit(item, level + 1)),
        };
      } else {
        result[key] = visit(entry, level + 1);
      }
    }
    return result;
  };
  return visit(value, depth);
}
