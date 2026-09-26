import { normalizeString, dedupeStrings } from "./configuration.mjs";

export async function runChatGptBrowserProbe(
  page,
  {
    baseUrl,
    modelsPath,
    accessToken,
    languageCode,
    clientVersion,
    clientBuildNumber,
    deviceId,
    sessionId,
    timeoutMs,
  },
) {
  await page.waitForTimeout(1_000);
  return page.evaluate(
    async ({
      baseUrl,
      modelsPath,
      accessToken,
      languageCode,
      clientVersion,
      clientBuildNumber,
      deviceId,
      sessionId,
      timeoutMs,
    }) => {
      const trimString = (value) =>
        typeof value === "string" && value.trim() ? value.trim() : null;
      const decodeJwtExpIso = (token) => {
        try {
          const [, payload] = String(token ?? "").split(".");
          if (!payload) {
            return null;
          }
          const normalized = payload.replace(/-/g, "+").replace(/_/g, "/");
          const padded = normalized.padEnd(Math.ceil(normalized.length / 4) * 4, "=");
          const jsonText = atob(padded);
          const decoded = JSON.parse(jsonText);
          return typeof decoded.exp === "number"
            ? new Date(decoded.exp * 1000).toISOString()
            : null;
        } catch {
          return null;
        }
      };
      const detectChallenge = (text) => {
        const lower = String(text ?? "").toLowerCase();
        return (
          lower.includes("cloudflare") ||
          lower.includes("captcha") ||
          lower.includes("challenge") ||
          lower.includes("verify you are human") ||
          lower.includes("just a moment") ||
          lower.includes("attention required")
        );
      };
      const request = async (path, init = {}) => {
        const url = path.startsWith("http://") || path.startsWith("https://")
          ? path
          : `${baseUrl}${path.startsWith("/") ? path : `/${path}`}`;
        const response = await fetch(url, init);
        const text = await response.text();
        let json = null;
        try {
          json = text ? JSON.parse(text) : null;
        } catch {
          json = null;
        }
        return {
          status: response.status,
          ok: response.ok,
          contentType: response.headers.get("content-type"),
          text,
          json,
          challengePresent: detectChallenge(text),
        };
      };

      let sessionProbe = null;
      try {
        sessionProbe = await request("/api/auth/session", {
          method: "GET",
          headers: {
            Accept: "application/json",
          },
          signal: AbortSignal.timeout(Math.min(timeoutMs, 30_000)),
        });
      } catch {
        sessionProbe = null;
      }

      const sessionAccessToken =
        trimString(sessionProbe?.json?.accessToken) ??
        trimString(sessionProbe?.json?.access_token) ??
        null;

      const authHeaders = {};
      if (sessionAccessToken || trimString(accessToken)) {
        authHeaders.Authorization = `Bearer ${sessionAccessToken ?? trimString(accessToken)}`;
      }
      if (trimString(languageCode)) {
        authHeaders["OAI-Language"] = trimString(languageCode);
      }
      if (trimString(clientVersion)) {
        authHeaders["OAI-Client-Version"] = trimString(clientVersion);
      }
      if (trimString(clientBuildNumber)) {
        authHeaders["OAI-Client-Build-Number"] = trimString(clientBuildNumber);
      }
      if (trimString(deviceId)) {
        authHeaders["OAI-Device-Id"] = trimString(deviceId);
      }
      if (trimString(sessionId)) {
        authHeaders["OAI-Session-Id"] = trimString(sessionId);
      }

      const modelsProbe = await request(modelsPath, {
        method: "GET",
        headers: authHeaders,
        signal: AbortSignal.timeout(Math.min(timeoutMs, 30_000)),
      });
      let initProbe = null;
      try {
        initProbe = await request("/backend-api/conversation/init", {
          method: "POST",
          headers: {
            ...authHeaders,
            "Content-Type": "application/json",
          },
          body: JSON.stringify({
            gizmo_id: null,
            requested_default_model: null,
            conversation_id: null,
            timezone_offset_min: -480,
          }),
          signal: AbortSignal.timeout(Math.min(timeoutMs, 30_000)),
        });
      } catch {
        initProbe = null;
      }

      const pageHtml = document.documentElement?.outerHTML ?? "";
      const scriptSources = Array.from(document.querySelectorAll("script[src]"))
        .map((node) => node.getAttribute("src"))
        .filter((value) => typeof value === "string" && value.includes("/backend-api/sentinel/"));
      const buildCandidates = [];
      const htmlBuild = document.documentElement?.getAttribute("data-build");
      if (trimString(htmlBuild)) {
        buildCandidates.push(trimString(htmlBuild));
      }
      const nextData = document.querySelector("#__NEXT_DATA__");
      if (nextData?.textContent) {
        try {
          const parsed = JSON.parse(nextData.textContent);
          const buildId = trimString(parsed?.buildId);
          if (buildId) {
            buildCandidates.push(buildId);
          }
        } catch {}
      }

      return {
        ok: modelsProbe.ok,
        status: modelsProbe.status,
        bodyPreview: String(modelsProbe.text ?? "").slice(0, 1500),
        contentType: modelsProbe.contentType,
        challengePresent:
          modelsProbe.challengePresent || detectChallenge(pageHtml) || detectChallenge(location.href),
        modelCount: Array.isArray(modelsProbe.json?.categories)
          ? modelsProbe.json.categories.length
          : Array.isArray(modelsProbe.json?.models)
            ? modelsProbe.json.models.length
            : Array.isArray(modelsProbe.json?.items)
              ? modelsProbe.json.items.length
              : Array.isArray(modelsProbe.json)
                ? modelsProbe.json.length
                : 0,
        modelSlugs: Array.isArray(modelsProbe.json?.models)
          ? modelsProbe.json.models
              .map((item) => trimString(item?.slug))
              .filter(Boolean)
          : [],
        conversationInitStatus: initProbe?.status ?? null,
        defaultModelSlug: trimString(initProbe?.json?.default_model_slug),
        conversationInitPreview: String(initProbe?.text ?? "").slice(0, 800),
        sessionStatus: sessionProbe?.status ?? null,
        sessionPreview: String(sessionProbe?.text ?? "").slice(0, 800),
        sessionAccessToken,
        sessionExpiresAt:
          trimString(sessionProbe?.json?.expires) ??
          trimString(sessionProbe?.json?.expires_at) ??
          null,
        email:
          trimString(sessionProbe?.json?.user?.email) ??
          trimString(sessionProbe?.json?.email) ??
          null,
        deviceId:
          trimString(deviceId) ??
          trimString(localStorage.getItem("oai/device_id")) ??
          trimString(localStorage.getItem("oaiDeviceId")),
        sessionId:
          trimString(sessionId) ??
          trimString(localStorage.getItem("oai/session_id")) ??
          trimString(localStorage.getItem("oaiSessionId")),
        clientVersion,
        clientBuildNumber,
        expiresAt: decodeJwtExpIso(accessToken),
        powSources: scriptSources,
        powDataBuild: buildCandidates.find(Boolean) ?? null,
        localStorageKeys: Object.keys(localStorage),
      };
    },
    {
      baseUrl,
      modelsPath,
      accessToken,
      languageCode,
      clientVersion,
      clientBuildNumber,
      deviceId,
      sessionId,
      timeoutMs,
    },
  );
}

export function extractBootstrapArtifacts(html) {
  const text = String(html ?? "");
  const accessToken =
    firstRegexCapture(text, /"accessToken"\s*:\s*"([^"]+)"/i) ??
    firstRegexCapture(text, /\\"accessToken\\"\s*:\s*\\"([^\\]+)\\"/i);
  const sessionId =
    firstRegexCapture(text, /"session_id"\s*:\s*"([^"]+)"/i) ??
    firstRegexCapture(text, /\\"session_id\\"\s*:\s*\\"([^\\]+)\\"/i);
  const deviceId =
    firstRegexCapture(text, /"deviceId"\s*:\s*"([0-9a-f-]{20,})"/i) ??
    firstRegexCapture(text, /"oai-did"\s*[:=]\s*"([0-9a-f-]{20,})"/i);
  const clientBuildNumber =
    firstRegexCapture(text, /"clientBuildNumber"\s*:\s*"([^"]+)"/i) ??
    firstRegexCapture(text, /\\"clientBuildNumber\\"\s*:\s*\\"([^\\]+)\\"/i);
  const powDataBuild =
    firstRegexCapture(text, /data-build="([^"]+)"/i) ??
    firstRegexCapture(text, /"buildId"\s*:\s*"([^"]+)"/i);
  const powSources = Array.from(
    text.matchAll(/https:\/\/chatgpt\.com\/backend-api\/sentinel\/[^"'\\\s<]+/gi),
    (match) => normalizeString(match[0]),
  ).filter(Boolean);
  return {
    accessToken: normalizeString(accessToken),
    sessionId: normalizeString(sessionId),
    deviceId: normalizeString(deviceId),
    clientBuildNumber: normalizeString(clientBuildNumber),
    powDataBuild: normalizeString(powDataBuild),
    powSources: dedupeStrings(powSources),
  };
}

export function firstRegexCapture(text, pattern) {
  const match = pattern.exec(String(text ?? ""));
  return match?.[1] ?? null;
}
