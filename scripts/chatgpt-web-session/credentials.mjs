import { mkdir, open, rename, unlink } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import os from "node:os";
import path from "node:path";
import { DEFAULT_BASE_URL, normalizeString, parseBoolean } from "./configuration.mjs";

const DEFAULT_CREDENTIAL_FAMILY_DIR = path.join(
  "chatgpt-platform",
  "chatgpt-web-reverse",
  "session-auth",
);

export async function maybeWriteCredentialFile(input, result) {
  const shouldWrite =
    input?.writeCredentialFile === true ||
    typeof input?.credentialFilePath === "string" ||
    typeof input?.credentialRootDir === "string" ||
    parseBoolean(process.env.CHATGPT_WEB_WRITE_CREDENTIAL_FILE, false);
  if (!shouldWrite || !result?.ok) {
    return null;
  }

  const targetPath = resolveCredentialFilePath(input, result);
  await mkdir(path.dirname(targetPath), { recursive: true, mode: 0o700 });
  const payload = buildCredentialPayload(result);
  const content = `${JSON.stringify(payload, null, 2)}\n`;
  // Same-directory replacement preserves the previous credential on write failure.
  const stagingPath = path.join(path.dirname(targetPath), `.chatgpt-credential-${randomUUID()}.tmp`);
  const file = await open(stagingPath, "wx", 0o600);
  let publicationError;
  try {
    await file.writeFile(content, "utf8");
    await file.sync();
    await file.close();
    for (let attempt = 0; ; attempt += 1) {
      try {
        await rename(stagingPath, targetPath);
        break;
      } catch (error) {
        // Windows replacement can briefly contend with another writer's handle.
        if (process.platform !== "win32" || attempt >= 7 ||
            !["EPERM", "EACCES", "EBUSY"].includes(error.code)) throw error;
        await delay(Math.min(10 * 2 ** attempt, 200));
      }
    }
  } catch (error) {
    publicationError = error;
    throw error;
  } finally {
    await file.close().catch(() => {});
    await unlink(stagingPath).catch((error) => {
      if (error.code === "ENOENT") return;
      if (publicationError) {
        throw new AggregateError([publicationError, error], "Credential publication and staging cleanup failed");
      }
      throw error;
    });
  }
  return targetPath;
}

export function buildCredentialPayload(result) {
  const uiConversationMaterial = extractChatGptUiConversationMaterial(result);
  const payload = {
    adapter: "chatgpt_web_reverse_compatible",
    baseUrl: DEFAULT_BASE_URL,
    sessionAuth: {
      transport: "bearer",
      headerName: "authorization",
    },
    headers: {},
    extraBody: {},
  };
  if (normalizeString(result.authToken)) {
    payload.apiKey = result.authToken;
  }
  if (normalizeString(result.expiresAt)) {
    payload.expiresAt = result.expiresAt;
  }
  if (normalizeString(result.cookieHeader)) {
    payload.headers.Cookie = result.cookieHeader;
  }
  if (normalizeString(result.userAgent)) {
    payload.headers["User-Agent"] = result.userAgent;
    payload.extraBody.userAgent = result.userAgent;
  }
  if (normalizeString(result.language)) {
    payload.extraBody.language = result.language;
  }
  if (normalizeString(result.languageCode)) {
    payload.extraBody.languageCode = result.languageCode;
  }
  if (normalizeString(result.timezone)) {
    payload.extraBody.timezone = result.timezone;
  }
  if (normalizeString(result.clientVersion)) {
    payload.extraBody.clientVersion = result.clientVersion;
  }
  if (normalizeString(result.clientBuildNumber)) {
    payload.extraBody.clientBuildNumber = result.clientBuildNumber;
  }
  if (normalizeString(result.deviceId)) {
    payload.extraBody.deviceId = result.deviceId;
  }
  if (normalizeString(result.sessionId)) {
    payload.extraBody.sessionId = result.sessionId;
  }
  if (normalizeString(uiConversationMaterial.clientVersion)) {
    payload.extraBody.clientVersion = uiConversationMaterial.clientVersion;
  }
  if (normalizeString(uiConversationMaterial.clientBuildNumber)) {
    payload.extraBody.clientBuildNumber = uiConversationMaterial.clientBuildNumber;
  }
  if (normalizeString(uiConversationMaterial.deviceId)) {
    payload.extraBody.deviceId = uiConversationMaterial.deviceId;
  }
  if (normalizeString(uiConversationMaterial.sessionId)) {
    payload.extraBody.sessionId = uiConversationMaterial.sessionId;
  }
  if (normalizeString(uiConversationMaterial.xOaiIs)) {
    payload.headers["X-OAI-IS"] = uiConversationMaterial.xOaiIs;
  }
  if (normalizeString(uiConversationMaterial.xConduitToken)) {
    payload.headers["X-Conduit-Token"] = uiConversationMaterial.xConduitToken;
  }
  if (normalizeString(uiConversationMaterial.oaiTelemetry)) {
    payload.headers["OAI-Telemetry"] = uiConversationMaterial.oaiTelemetry;
  }
  if (normalizeString(uiConversationMaterial.oaiEchoLogs)) {
    payload.headers["OAI-Echo-Logs"] = uiConversationMaterial.oaiEchoLogs;
  }
  if (normalizeString(uiConversationMaterial.turnTraceId)) {
    payload.extraBody.chatgptWebTurnTraceId = uiConversationMaterial.turnTraceId;
  }
  if (normalizeString(uiConversationMaterial.sentinelChatRequirementsToken)) {
    payload.extraBody.chatgptWebSentinelChatRequirementsToken =
      uiConversationMaterial.sentinelChatRequirementsToken;
  }
  if (normalizeString(uiConversationMaterial.sentinelProofToken)) {
    payload.extraBody.chatgptWebSentinelProofToken = uiConversationMaterial.sentinelProofToken;
  }
  if (normalizeString(uiConversationMaterial.sentinelTurnstileToken)) {
    payload.extraBody.chatgptWebSentinelTurnstileToken =
      uiConversationMaterial.sentinelTurnstileToken;
  }
  if (normalizeString(uiConversationMaterial.sentinelSoToken)) {
    payload.extraBody.chatgptWebSentinelSoToken = uiConversationMaterial.sentinelSoToken;
  }
  if (Array.isArray(result.chatgptPowSources) && result.chatgptPowSources.length > 0) {
    payload.extraBody.chatgptPowSources = result.chatgptPowSources;
  }
  if (normalizeString(result.chatgptPowDataBuild)) {
    payload.extraBody.chatgptPowDataBuild = result.chatgptPowDataBuild;
  }
  if (normalizeString(result.accountName)) {
    payload.accountName = result.accountName;
  }
  if (normalizeString(result.credentialMaterialKey)) {
    payload.credentialMaterialKey = result.credentialMaterialKey;
  }
  payload.rawSource = {
    authProbe: result.authProbe ?? null,
    profileSource: result.profileSource ?? null,
    currentUrl: result.currentUrl ?? null,
    challengePresent: result.challengePresent === true,
    localStorageKeys: Array.isArray(result.localStorageKeys) ? result.localStorageKeys : [],
    bodyPreview: result.bodyPreview ?? null,
    uiConversationMaterial: {
      captured: uiConversationMaterial.captured === true,
      rawHeaders: uiConversationMaterial.rawHeaders === true,
      hasXOaiIs: Boolean(uiConversationMaterial.xOaiIs),
      hasXConduitToken: Boolean(uiConversationMaterial.xConduitToken),
      hasOaiTelemetry: Boolean(uiConversationMaterial.oaiTelemetry),
      hasOaiEchoLogs: Boolean(uiConversationMaterial.oaiEchoLogs),
      hasTurnTraceId: Boolean(uiConversationMaterial.turnTraceId),
      hasSentinelChatRequirementsToken: Boolean(
        uiConversationMaterial.sentinelChatRequirementsToken,
      ),
      hasSentinelProofToken: Boolean(uiConversationMaterial.sentinelProofToken),
      hasSentinelTurnstileToken: Boolean(uiConversationMaterial.sentinelTurnstileToken),
      hasSentinelSoToken: Boolean(uiConversationMaterial.sentinelSoToken),
    },
  };
  return payload;
}

export function extractChatGptUiConversationMaterial(result) {
  const capture = result?.relayResponse?.uiRequestCapture;
  const requests = Array.isArray(capture?.requests) ? capture.requests : [];
  const request = [...requests]
    .reverse()
    .find((item) => {
      try {
        return new URL(item?.url).pathname === "/backend-api/f/conversation";
      } catch {
        return false;
      }
    });
  const headers = request?.headers && typeof request.headers === "object" ? request.headers : {};
  const readHeader = (name) => {
    const normalized = name.toLowerCase();
    const value = headers[normalized] ?? headers[name];
    return typeof value === "string" ? normalizeString(value) : null;
  };
  return {
    captured: Boolean(request),
    rawHeaders: capture?.rawHeaders === true,
    clientVersion: readHeader("oai-client-version"),
    clientBuildNumber: readHeader("oai-client-build-number"),
    deviceId: readHeader("oai-device-id"),
    sessionId: readHeader("oai-session-id"),
    xOaiIs: readHeader("x-oai-is"),
    xConduitToken: readHeader("x-conduit-token"),
    turnTraceId: readHeader("x-oai-turn-trace-id"),
    oaiTelemetry: readHeader("oai-telemetry"),
    oaiEchoLogs: readHeader("oai-echo-logs"),
    sentinelChatRequirementsToken: readHeader("openai-sentinel-chat-requirements-token"),
    sentinelProofToken: readHeader("openai-sentinel-proof-token"),
    sentinelTurnstileToken: readHeader("openai-sentinel-turnstile-token"),
    sentinelSoToken: readHeader("openai-sentinel-so-token"),
  };
}

export function resolveCredentialFilePath(input, result) {
  const explicitPath = normalizeString(input?.credentialFilePath);
  if (explicitPath) {
    return ensureJsonExtension(explicitPath);
  }

  const rootDir =
    normalizeString(input?.credentialRootDir) ??
    normalizeString(process.env.NEURO_PROVIDER_CREDENTIAL_ROOT_DIR) ??
    path.join(os.homedir(), ".neuro");
  const familyDir =
    normalizeString(input?.credentialFamilyDir) ?? DEFAULT_CREDENTIAL_FAMILY_DIR;
  const explicitFileName = normalizeString(input?.credentialFileName);
  const accountHint =
    normalizeString(result?.accountName) ??
    normalizeString(result?.credentialMaterialKey) ??
    `chatgpt-web-${Date.now()}`;
  const inferredFileName =
    explicitFileName ??
    sanitizeFileNameComponent(accountHint) ??
    `chatgpt-web-${Date.now()}.json`;
  return path.join(rootDir, familyDir, ensureJsonExtension(inferredFileName));
}

export function ensureJsonExtension(filePath) {
  return String(filePath).toLowerCase().endsWith(".json") ? filePath : `${filePath}.json`;
}

export function sanitizeFileNameComponent(value) {
  const normalized = String(value ?? "")
    .trim()
    .toLowerCase()
    .replace(/[^a-z0-9._-]+/g, "-")
    .replace(/-+/g, "-")
    .replace(/^-|-$/g, "");
  return normalized || null;
}
