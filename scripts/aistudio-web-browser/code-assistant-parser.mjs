import { tryParseJson, collectGeminiTextStrings } from "./payload.mjs";
import { normalizeString } from "./settings.mjs";

export function extractCodeAssistantGenerationId(bodyText) {
  const parsed = tryParseJson(bodyText);
  return Array.isArray(parsed) && typeof parsed[0] === "string" ? parsed[0] : null;
}

export function extractModelMessageBlocks(node, blocks = []) {
  if (Array.isArray(node)) {
    if (node.length >= 2 && node[1] === "model") {
      const texts = collectGeminiTextStrings(node[0], [])
        .map((entry) => normalizeString(entry))
        .filter(Boolean);
      if (texts.length) {
        blocks.push(texts);
      }
      return blocks;
    }
    for (const entry of node) {
      extractModelMessageBlocks(entry, blocks);
    }
    return blocks;
  }
  if (node && typeof node === "object") {
    for (const value of Object.values(node)) {
      extractModelMessageBlocks(value, blocks);
    }
  }
  return blocks;
}

export function extractFinalTextFromCodeAssistantStream(bodyText) {
  const parsed = tryParseJson(bodyText);
  if (!parsed) {
    return null;
  }
  const blocks = extractModelMessageBlocks(parsed, []).filter((entry) => Array.isArray(entry));
  if (!blocks.length) {
    return null;
  }
  const lastBlock = blocks[blocks.length - 1]
    .map((entry) => normalizeString(entry))
    .filter(Boolean);
  return lastBlock.length ? lastBlock[lastBlock.length - 1] : null;
}

export function parseToolCallsFromAssistantText(text) {
  const normalized = normalizeString(text);
  if (!normalized) {
    return { cleanText: "", toolCalls: [] };
  }

  const outerMatch = normalized.match(/<tool_calls>\s*([\s\S]*?)\s*<\/tool_calls>/i);
  if (!outerMatch) {
    return { cleanText: normalized, toolCalls: [] };
  }

  const toolCalls = [];
  const inner = outerMatch[1];
  const entryRegex = /<tool_call>\s*([\s\S]*?)\s*<\/tool_call>/gi;
  let entryMatch;
  while ((entryMatch = entryRegex.exec(inner)) !== null) {
    const block = entryMatch[1];
    const nameMatch = block.match(/<tool_name>\s*([\s\S]*?)\s*<\/tool_name>/i);
    const parametersMatch = block.match(/<parameters>\s*([\s\S]*?)\s*<\/parameters>/i);
    const name = normalizeString(nameMatch?.[1]);
    const argumentsText = normalizeString(parametersMatch?.[1]) ?? "{}";
    if (!name) {
      continue;
    }
    let parsedArguments = {};
    try {
      parsedArguments = JSON.parse(argumentsText);
    } catch (_) {
      parsedArguments = {};
    }
    toolCalls.push({
      id: `call_${name}`,
      name,
      args: parsedArguments,
    });
  }

  const cleanText = normalized.replace(outerMatch[0], "").trim();
  return { cleanText, toolCalls };
}

export function buildSyntheticGenerateContentResponse(text, model, toolCalls = []) {
  const parts = [];
  const normalizedText = normalizeString(text);
  if (normalizedText || !toolCalls.length) {
    parts.push({ text: normalizedText ?? "" });
  }
  for (const toolCall of toolCalls) {
    parts.push({
      functionCall: {
        id: toolCall.id,
        name: toolCall.name,
        args: toolCall.args ?? {},
      },
    });
  }
  return JSON.stringify({
    candidates: [
      {
        index: 0,
        content: {
          role: "model",
          parts,
        },
        finishReason: "STOP",
      },
    ],
    modelVersion: model,
  });
}
