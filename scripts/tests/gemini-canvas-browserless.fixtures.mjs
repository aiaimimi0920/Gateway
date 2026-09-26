import fs from "node:fs/promises";
import path from "node:path";
import { pathToFileURL } from "node:url";
import vm from "node:vm";

export async function importTestableProbe() {
  const root = path.resolve(import.meta.dirname, "..");
  const source = await fs.readFile(path.join(root, "probe-gemini-canvas-program-browserless-invoke.mjs"), "utf8");
  const marker = "\nmain().catch(async (error) => {";
  if (source.split(marker).length !== 2 || !source.trimEnd().endsWith("process.exitCode = 1;\n});")) {
    throw new Error("Unexpected browserless probe entry guard");
  }
  const exports = [
    "resolveInvocationOptions",
    "main", "readJson", "buildSummaryBase", "deriveProgramHandle",
    "fileExists", "pathStatMtime", "findLatestBrowserState", "uniqueStrings", "walkFiles",
    "resolveProgramHandlePath", "resolveProgramHandle", "resolveProfileDir", "resolveStorageStatePath",
    "normalizeOperation", "defaultPromptForOperation", "defaultModelForOperation", "inferBaseUrl", "inferApiBaseUrl",
    "DEFAULT_BASE_URL", "DEFAULT_API_BASE_URL", "DEFAULT_AUTH_USER", "DEFAULT_LOCALE", "DEFAULT_TIMEOUT_MS", "DEFAULT_VIDEO_POLL_TIMEOUT_MS",
    "probeMusic", "normalizeRemoteMusicWsUrl", "DEFAULT_MUSIC_WS_URL",
    "probeText", "probeTts", "probeImage", "probeVideoCreate",
    "buildVideoInvokeUrl", "looksLikeQuotaOrPlanGate", "DEFAULT_VIDEO_POLL_INTERVAL_MS",
    "mimeToExt", "buildTextRequestBody", "buildTtsRequestBody", "buildImageRequestBody",
    "buildVideoCreateRequestBody", "extractTextFromGenerateContentResponse",
    "extractAudioFromGenerateContentResponse", "extractInlineImageFromGenerateContentResponse",
    "extractImagenImages", "parsePcmSampleRate", "parsePcmChannels", "pcmAudioToWavBytes",
    "extractVideoUriFromOperation", "summarizeJsonBody",
    "cookieMatchesUrl", "buildPureHttpSession", "mergeSetCookie", "buildSapisidAuthorization",
    "buildBrowserClientHints", "buildJsonHeaders", "buildJsonHeadersWithOptions", "buildFormHeaders",
    "appendApiKeyQueryIfMissing", "buildAuthAttempts", "responseHeadersToObject", "getSetCookieValues",
    "collectTextResponse", "collectBytesResponse", "tryParseJson", "textPreview", "redactHeaders",
    "sendExactMinimalApiKeyOnlyJson", "sendJsonWithAuthAttempts", "sendJsonWithCustomAttempts",
    "sendGetBytesWithAuthAttempts", "sendFormRequest", "sendGetJsonWithAuthAttempts",
    "normalizeString", "originFromUrl", "outDirFromContext", "extractXsrfToken", "DEFAULT_BROWSERLESS_USER_AGENT",
  ];
  const temporary = path.join(root, `.gemini-canvas-browserless-testable-${process.pid}-${Date.now()}.mjs`);
  await fs.writeFile(temporary, source.slice(0, source.indexOf(marker)) + `\nexport { ${exports.join(", ")} };\n`, "utf8");
  try {
    return await import(pathToFileURL(temporary).href);
  } finally {
    await fs.rm(temporary, { force: true });
  }
}

export function fixtureResponse(options = {}) {
  const status = options.status ?? 200, headers = new Headers(options.headers ?? {});
  if (options.setCookie) headers.getSetCookie = () => options.setCookie;
  if (options.legacyCookies) headers.getSetCookie = undefined;
  return {
    status, ok: status >= 200 && status < 300,
    url: options.url ?? "https://fixture.test/final", headers,
    async text() { if (options.bodyError) throw options.bodyError; return options.text ?? '{"marker":"fixture"}'; },
    async arrayBuffer() {
      if (options.bodyError) throw options.bodyError;
      return Uint8Array.from(options.bytes ?? [0, 128, 255]).buffer;
    },
  };
}

export const fixtureSession = () => ({ cookieHeader: "SAPISID=fixture-session", sapisid: "fixture-session", authUser: "2" });
export const fixtureRequestContext = () => ({ session: fixtureSession(), apiKey: "fixture-api-key", pageOrigin: "https://gemini.google.com", pageReferer: "https://gemini.google.com/app/fixture", locale: "en-US", outDir: "fixture-output" });

export function requestHarness(app, options = {}) {
  const calls = [], archives = [], requests = [], signals = [];
  const context = { ...fixtureRequestContext(), ...options.context };
  const initialSession = context.session, failure = new Error("fixture request boundary failed");
  const responses = options.responses ?? [fixtureResponse()];
  const dependencies = {
    ...app, Buffer, URL, URLSearchParams,
    AbortSignal: { timeout(ms) { const signal = { ms }; signals.push(signal); calls.push("timeout"); return signal; } },
    async fetch(url, init) {
      calls.push("fetch");
      requests.push({ url, ...init });
      if (options.failureAt === "fetch") throw failure;
      const response = responses[requests.length - 1];
      if (!response) throw new Error("Unexpected extra fixture request");
      return response;
    },
    async archiveAttempt(...args) {
      calls.push("archive"); archives.push(args);
      if (options.failureAt === "archive") throw failure;
    },
  };
  const names = ["sendExactMinimalApiKeyOnlyJson", "sendJsonWithAuthAttempts", "sendJsonWithCustomAttempts", "sendGetBytesWithAuthAttempts", "sendFormRequest", "sendGetJsonWithAuthAttempts"];
  const send = Object.fromEntries(names.map((name) => [name, vm.runInNewContext(`(${app[name].toString()})`, dependencies, { timeout: 1000 })]));
  return { context, initialSession, failure, requests, archives, signals, calls, send };
}
