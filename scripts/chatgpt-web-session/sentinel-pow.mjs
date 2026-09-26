import { createHash, randomUUID } from "node:crypto";
import { normalizeString, dedupeStrings } from "./configuration.mjs";

export const DEFAULT_POW_SCRIPT = "https://chatgpt.com/backend-api/sentinel/sdk.js";

export function extractChatGptPowBootstrapFromHtml(html) {
  const text = String(html ?? "");
  const scriptMatches = Array.from(
    text.matchAll(/<script[^>]+src=["']([^"']+)["']/gi),
    (match) => normalizeString(match[1]),
  ).filter(Boolean);
  const buildMatch =
    text.match(/c\/[^/]*\/_/i)?.[0] ??
    text.match(/<html[^>]*data-build=["']([^"']*)["']/i)?.[1] ??
    "";
  return {
    powScriptSources: scriptMatches.length > 0 ? scriptMatches : [DEFAULT_POW_SCRIPT],
    powDataBuild: normalizeString(buildMatch) ?? "",
  };
}

export function mergeChatGptPowBootstrap(bootstrap, input) {
  const fallbackSources = Array.isArray(input?.chatgptPowSources)
    ? input.chatgptPowSources
        .map((value) => normalizeString(value))
        .filter(Boolean)
    : [];
  const mergedSources = dedupeStrings([
    ...(Array.isArray(bootstrap?.powScriptSources) ? bootstrap.powScriptSources : []),
    ...fallbackSources,
    DEFAULT_POW_SCRIPT,
  ]);
  return {
    powScriptSources: mergedSources.length > 0 ? mergedSources : [DEFAULT_POW_SCRIPT],
    powDataBuild:
      normalizeString(bootstrap?.powDataBuild) ??
      normalizeString(input?.chatgptPowDataBuild) ??
      "",
  };
}

export function buildLegacyRequirementsToken(bootstrap, userAgent) {
  const seed = Math.random().toFixed(15);
  const config = buildPowConfig(bootstrap, userAgent);
  const [answer] = powGenerate(seed, "0fffff", config, 500000);
  return `gAAAAAC${answer}`;
}

export function buildProofToken(bootstrap, userAgent, seed, difficulty) {
  const config = buildPowConfig(bootstrap, userAgent);
  const [answer, solved] = powGenerate(seed, difficulty, config, 500000);
  if (!solved) {
    return null;
  }
  return `gAAAAAB${answer}`;
}

export function buildPowConfig(bootstrap, userAgent) {
  const scriptSources =
    Array.isArray(bootstrap?.powScriptSources) && bootstrap.powScriptSources.length > 0
      ? bootstrap.powScriptSources
      : [DEFAULT_POW_SCRIPT];
  const navigatorKeys = [
    "registerProtocolHandler−function registerProtocolHandler() { [native code] }",
    "storage−[object StorageManager]",
    "locks−[object LockManager]",
    "appCodeName−Mozilla",
    "permissions−[object Permissions]",
    "share−function share() { [native code] }",
    "webdriver−false",
    "managed−[object NavigatorManagedData]",
    "canShare−function canShare() { [native code] }",
    "vendor−Google Inc.",
  ];
  const windowKeys = [
    "window",
    "self",
    "document",
    "location",
    "navigator",
    "indexedDB",
    "sessionStorage",
    "localStorage",
    "__NEXT_DATA__",
  ];
  const documentKeys = ["_reactListeningo743lnnpvdg", "location"];
  const cores = [8, 16, 24, 32];
  return [
    3000,
    "Mon Jan 02 2006 15:04:05 GMT-0500 (Eastern Standard Time)",
    4294705152,
    0,
    userAgent,
    pickRandom(scriptSources) ?? DEFAULT_POW_SCRIPT,
    normalizeString(bootstrap?.powDataBuild) ?? "",
    "en-US",
    "en-US,es-US,en,es",
    0,
    pickRandom(navigatorKeys) ?? navigatorKeys[0],
    pickRandom(documentKeys) ?? documentKeys[0],
    pickRandom(windowKeys) ?? windowKeys[0],
    Date.now(),
    randomUUID(),
    "",
    pickRandom(cores) ?? 8,
    0,
  ];
}

export function powGenerate(seed, difficulty, config, limit) {
  const target = Buffer.from(String(difficulty || ""), "hex");
  const diffLen = Math.floor(String(difficulty || "").length / 2);
  const part1 = joinJsonSlice(config.slice(0, 3), true, true);
  const part2 = joinJsonSlice(config.slice(4, 9), false, false);
  const part3 = joinJsonSlice(config.slice(10), false, true);
  for (let i = 0; i < limit; i += 1) {
    const finalJson = `${part1}${i},${part2}${i >> 1}${part3}`;
    const encoded = Buffer.from(finalJson, "utf8").toString("base64");
    const hash = createHash("sha3-512")
      .update(String(seed || ""), "utf8")
      .update(encoded, "utf8")
      .digest();
    if (diffLen > 0 && Buffer.compare(hash.subarray(0, diffLen), target.subarray(0, diffLen)) <= 0) {
      return [encoded, true];
    }
  }
  return [
    `wQ8Lk5FbGpA2NcR9dShT6gYjU7VxZ4D${Buffer.from(`\"${seed}\"`, "utf8").toString("base64")}`,
    false,
  ];
}

export function joinJsonSlice(items, dropLastBracket, dropFirstBracket) {
  let text = JSON.stringify(items ?? []);
  if (dropFirstBracket && text.startsWith("[")) {
    text = text.slice(1);
  }
  if (dropLastBracket && text.endsWith("]")) {
    text = text.slice(0, -1);
  }
  return text;
}

export function pickRandom(values) {
  return Array.isArray(values) && values.length > 0
    ? values[Math.floor(Math.random() * values.length)]
    : null;
}
