import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

const PROGRAM_APP_PATH_REGEX = /(?:https:\/\/gemini\.google\.com)?(\/app\/(?:[0-9a-f]{8,}|\d{13,}))/gi;
const PROGRAM_CONVERSATION_ID_REGEX = /\bc_([0-9a-f]{8,})\b/gi;
const PROGRAM_RESPONSE_ID_REGEX = /\br_([0-9a-f]{8,})\b/gi;
const PROGRAM_PAIR_REGEX =
  /(?:\\?")?(c_[0-9a-f]{8,})(?:\\?")?\s*,\s*(?:\\?")?(r_[0-9a-f]{8,})(?:\\?")?/gi;
const PROGRAM_SHARE_PATH_REGEX = /(\/share\/[0-9a-z]{8,})/gi;

export function uniqueStrings(values) {
  return [...new Set((values || []).filter(Boolean).map((value) => String(value).trim()).filter(Boolean))];
}

export function extractProgramAppPaths(text) {
  const values = [];
  const source = String(text || "");
  let match;
  while ((match = PROGRAM_APP_PATH_REGEX.exec(source)) !== null) {
    values.push(match[1]);
  }
  return [...new Set(values)];
}

export function extractAppPath(url) {
  if (!url) {
    return null;
  }
  return extractProgramAppPaths(String(url))[0] ?? null;
}

function extractProgramConversationIds(text) {
  const values = [];
  const source = String(text || "");
  let match;
  while ((match = PROGRAM_CONVERSATION_ID_REGEX.exec(source)) !== null) {
    values.push(`c_${match[1]}`);
  }
  return [...new Set(values)];
}

function extractProgramResponseIds(text) {
  const values = [];
  const source = String(text || "");
  let match;
  while ((match = PROGRAM_RESPONSE_ID_REGEX.exec(source)) !== null) {
    values.push(`r_${match[1]}`);
  }
  return [...new Set(values)];
}

function extractProgramSharePaths(text) {
  const values = [];
  const source = String(text || "");
  let match;
  while ((match = PROGRAM_SHARE_PATH_REGEX.exec(source)) !== null) {
    values.push(match[1]);
  }
  return uniqueStrings(values);
}

export function extractProgramHandleHintsFromText(text) {
  const appPaths = extractProgramAppPaths(text);
  const conversationIds = extractProgramConversationIds(text);
  const responseIds = extractProgramResponseIds(text);
  const sharePaths = extractProgramSharePaths(text);
  const derivedAppPaths = conversationIds.map((id) => `/app/${id.replace(/^c_/, "")}`);
  return {
    appPaths: uniqueStrings([...appPaths, ...derivedAppPaths]),
    conversationIds,
    responseIds,
    sharePaths,
  };
}

export function mergeProgramHandleHints(target, incoming) {
  target.appPaths = uniqueStrings([...(target.appPaths || []), ...(incoming.appPaths || [])]);
  target.conversationIds = uniqueStrings([
    ...(target.conversationIds || []),
    ...(incoming.conversationIds || []),
  ]);
  target.responseIds = uniqueStrings([...(target.responseIds || []), ...(incoming.responseIds || [])]);
  target.sharePaths = uniqueStrings([...(target.sharePaths || []), ...(incoming.sharePaths || [])]);
}

export function strongestProgramHandleHint(hints) {
  const reversedAppPaths = [...(hints?.appPaths || [])].reverse();
  const concreteAppPath = reversedAppPaths.find((value) =>
    /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(String(value)),
  );
  if (concreteAppPath) {
    return concreteAppPath;
  }
  const reversedConversationIds = [...(hints?.conversationIds || [])].reverse();
  if (reversedConversationIds[0]) {
    return reversedConversationIds[0];
  }
  const reversedSharePaths = [...(hints?.sharePaths || [])].reverse();
  return reversedSharePaths[0] ?? null;
}

export function deriveConversationIdForAppPath(appPath, conversationIds) {
  const suffix = String(appPath || "")
    .split("/")
    .filter(Boolean)
    .at(-1);
  if (!suffix) {
    return [...(conversationIds || [])].reverse()[0] ?? null;
  }
  const matched = [...(conversationIds || [])]
    .reverse()
    .find((value) => String(value).replace(/^c_/, "") === suffix);
  return matched ?? [...(conversationIds || [])].reverse()[0] ?? null;
}

export function dedupeProgramHandlePairs(pairs) {
  const map = new Map();
  for (const pair of pairs || []) {
    if (!pair?.conversationId || !pair?.responseId) {
      continue;
    }
    const key = `${pair.conversationId}|${pair.responseId}|${pair.appPath || ""}`;
    map.set(key, pair);
  }
  return [...map.values()];
}

export function extractProgramHandlePairs(text, source = {}) {
  const pairs = [];
  const input = String(text || "");
  let match;
  while ((match = PROGRAM_PAIR_REGEX.exec(input)) !== null) {
    const conversationId = String(match[1] || "").trim();
    const responseId = String(match[2] || "").trim();
    if (!/^c_[0-9a-f]{8,}$/i.test(conversationId) || !/^r_[0-9a-f]{8,}$/i.test(responseId)) {
      continue;
    }
    const appPath = `/app/${conversationId.replace(/^c_/, "")}`;
    pairs.push({
      appPath,
      programUrl: `https://gemini.google.com${appPath}`,
      conversationId,
      responseId,
      sourceUrl: source.sourceUrl || null,
      sourceRpc: source.sourceRpc || null,
      sourceKind: source.sourceKind || null,
      sourceSurface: source.sourceSurface || null,
      sourceWsUrl: source.sourceWsUrl || null,
      sourceTargetDomain: source.sourceTargetDomain || null,
      ts: source.ts || null,
    });
  }
  return dedupeProgramHandlePairs(pairs);
}

export function isConcreteProgramUrlCandidate(candidate) {
  const value = normalizeString(candidate);
  if (!value) {
    return false;
  }
  try {
    const parsed = new URL(value);
    return /\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(parsed.pathname);
  } catch {
    return /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(value);
  }
}
