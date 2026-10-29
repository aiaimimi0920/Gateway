import { normalizeString } from "./configuration.mjs";
import { runChatGptBrowserUiRelay } from "./ui-relay.mjs";
import { mergeChatGptPowBootstrap, extractChatGptPowBootstrapFromHtml, buildLegacyRequirementsToken } from "./sentinel-pow.mjs";
import { browserRequirements } from "./http-requirements.mjs";

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
  };
  if (authHeaderValue) {
    requirementsHeaders.Authorization = `Bearer ${authHeaderValue}`;
  }
  const requirements = await browserRequirements(page, input, bootstrap, legacyToken, requirementsHeaders);
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
    "OpenAI-Sentinel-Chat-Requirements-Token": requirements.token,
    "X-OpenAI-Target-Path": CHATGPT_WEB_DEFAULT_CONVERSATION_PATH,
    "X-OpenAI-Target-Route": CHATGPT_WEB_DEFAULT_CONVERSATION_PATH,
  };
  if (authHeaderValue) {
    conversationHeaders.Authorization = `Bearer ${authHeaderValue}`;
  }
  if (requirements.proofToken) {
    conversationHeaders["OpenAI-Sentinel-Proof-Token"] = requirements.proofToken;
  }
  if (requirements.turnstileToken) {
    conversationHeaders["OpenAI-Sentinel-Turnstile-Token"] = requirements.turnstileToken;
  }
  if (requirements.soToken) {
    conversationHeaders["OpenAI-Sentinel-SO-Token"] = requirements.soToken;
  }
  const relayResponse = await page.evaluate(
    async ({ url, headers, body }) => {
      const response = await fetch(url, {
        method: "POST",
        credentials: "same-origin",
        redirect: "error",
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
    requirementsStatus: requirements.status,
    requirementsContentType: requirements.contentType,
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
  // An auth/verification rejection is not permission to submit the same turn through the UI.
  if ([401, 403, 429].includes(requirementsStatus) || [401, 403, 429].includes(relayStatus)) return false;
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
