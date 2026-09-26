import { normalizeString } from "./configuration.mjs";
import { safeParseJson } from "./json-parsing.mjs";
import { runChatGptBrowserUiRelay } from "./ui-relay.mjs";
import { mergeChatGptPowBootstrap, extractChatGptPowBootstrapFromHtml, buildLegacyRequirementsToken, buildProofToken } from "./sentinel-pow.mjs";

const DEFAULT_REQUIREMENTS_PATH = "/backend-api/sentinel/chat-requirements";

const CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH = DEFAULT_REQUIREMENTS_PATH;

const CHATGPT_WEB_DEFAULT_CONVERSATION_PATH = "/backend-api/conversation";

export async function runChatGptBrowserRelay(page, input) {
  const html = await page.content().catch(() => "");
  const bootstrap = mergeChatGptPowBootstrap(
    extractChatGptPowBootstrapFromHtml(html),
    input,
  );
  const legacyToken = buildLegacyRequirementsToken(bootstrap, input.userAgent);
  const authHeaderValue = normalizeString(input.accessToken);
  const requirementsHeaders = {
    Accept: "application/json",
    "Accept-Language": input.acceptLanguage,
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    Pragma: "no-cache",
    Priority: "u=1, i",
    "OAI-Client-Build-Number": input.clientBuildNumber,
    "OAI-Client-Version": input.clientVersion,
    "OAI-Device-Id": input.deviceId,
    "OAI-Language": input.languageCode,
    "OAI-Session-Id": input.sessionId,
    "X-OpenAI-Target-Path": CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH,
    "X-OpenAI-Target-Route": CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH,
  };
  if (authHeaderValue) {
    requirementsHeaders.Authorization = `Bearer ${authHeaderValue}`;
  }
  const requirementsResponse = await page.evaluate(
    async ({ url, headers, body }) => {
      const response = await fetch(url, {
        method: "POST",
        credentials: "include",
        headers,
        body: JSON.stringify(body),
      });
      const text = await response.text();
      return {
        status: response.status,
        contentType: response.headers.get("content-type"),
        bodyText: text,
      };
    },
    {
      url: `${input.baseUrl}${CHATGPT_WEB_DEFAULT_REQUIREMENTS_PATH}`,
      headers: requirementsHeaders,
      body: { p: legacyToken },
    },
  );
  const requirementsValue = safeParseJson(requirementsResponse.bodyText);
  let proofToken = null;
  if (
    requirementsValue?.proofofwork?.required === true &&
    requirementsValue?.proofofwork?.seed &&
    requirementsValue?.proofofwork?.difficulty
  ) {
    proofToken = buildProofToken(
      bootstrap,
      input.userAgent,
      requirementsValue.proofofwork.seed,
      requirementsValue.proofofwork.difficulty,
    );
  }
  const conversationHeaders = {
    Accept: "text/event-stream",
    "Accept-Language": input.acceptLanguage,
    "Cache-Control": "no-cache",
    "Content-Type": "application/json",
    Pragma: "no-cache",
    Priority: "u=1, i",
    "OAI-Client-Build-Number": input.clientBuildNumber,
    "OAI-Client-Version": input.clientVersion,
    "OAI-Device-Id": input.deviceId,
    "OAI-Language": input.languageCode,
    "OAI-Session-Id": input.sessionId,
    "OpenAI-Sentinel-Chat-Requirements-Token": normalizeString(requirementsValue?.token) ?? "",
    "X-OpenAI-Target-Path": CHATGPT_WEB_DEFAULT_CONVERSATION_PATH,
    "X-OpenAI-Target-Route": CHATGPT_WEB_DEFAULT_CONVERSATION_PATH,
  };
  if (authHeaderValue) {
    conversationHeaders.Authorization = `Bearer ${authHeaderValue}`;
  }
  if (proofToken) {
    conversationHeaders["OpenAI-Sentinel-Proof-Token"] = proofToken;
  }
  if (normalizeString(requirementsValue?.turnstile?.token)) {
    conversationHeaders["OpenAI-Sentinel-Turnstile-Token"] = normalizeString(
      requirementsValue.turnstile.token,
    );
  }
  if (normalizeString(requirementsValue?.so_token)) {
    conversationHeaders["OpenAI-Sentinel-SO-Token"] = normalizeString(
      requirementsValue.so_token,
    );
  }
  const relayResponse = await page.evaluate(
    async ({ url, headers, body }) => {
      const response = await fetch(url, {
        method: "POST",
        credentials: "include",
        headers,
        body: JSON.stringify(body),
      });
      const text = await response.text();
      return {
        status: response.status,
        contentType: response.headers.get("content-type"),
        bodyText: text,
      };
    },
    {
      url: `${input.baseUrl}${CHATGPT_WEB_DEFAULT_CONVERSATION_PATH}`,
      headers: conversationHeaders,
      body: input.requestBody,
    },
  );
  const result = {
    requirementsStatus: requirementsResponse.status,
    requirementsContentType: requirementsResponse.contentType,
    requirementsPreview: String(requirementsResponse.bodyText ?? "").slice(0, 500),
    status: relayResponse.status,
    contentType: relayResponse.contentType,
    bodyText: relayResponse.bodyText,
    bodyPreview: String(relayResponse.bodyText ?? "").slice(0, 1000),
  };
  if (shouldFallbackToUiRelay(result)) {
    return runChatGptBrowserUiRelay(page, input);
  }
  return result;
}

export function shouldFallbackToUiRelay(relayResponse) {
  const requirementsStatus = Number(relayResponse?.requirementsStatus ?? 0);
  const relayStatus = Number(relayResponse?.status ?? 0);
  const preview = String(
    relayResponse?.bodyText ??
      relayResponse?.bodyPreview ??
      relayResponse?.requirementsPreview ??
      "",
  ).toLowerCase();
  if (!(requirementsStatus >= 200 && requirementsStatus < 300)) {
    return true;
  }
  if (!(relayStatus >= 200 && relayStatus < 300)) {
    return true;
  }
  return (
    preview.includes("unusual activity") ||
    preview.includes("unauthorized") ||
    preview.includes("internal server error") ||
    preview.includes("your session has ended") ||
    preview.includes("登录以继续") ||
    preview.includes("log in to continue") ||
    preview.includes("login_with") ||
    preview.includes("/api/auth/error") ||
    preview.includes("/auth/error") ||
    preview.includes("captcha") ||
    preview.includes("challenge")
  );
}
