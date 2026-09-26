import { chromium } from "playwright-core";
import { rm } from "node:fs/promises";
import { resolveExecutablePath, resolveBrowserProfileSource, cloneBrowserProfile } from "./producer-browser/profile.mjs";
import { normalizeString } from "./producer-browser/request-fields.mjs";
import { normalizeBaseUrl, normalizeTimeoutMs, parseBoolean, parseInput } from "./producer-browser/input.mjs";
import { appendTrace } from "./producer-browser/trace.mjs";
import { executeProducerConversationVideoFlow } from "./producer-browser/video-flow.mjs";
import { executeProducerPageVideoFlow } from "./producer-browser/page-video-flow.mjs";
import { readStdin } from "./producer-browser/stdin.mjs";

const DEFAULT_LOCALE = "zh-CN";
const DEFAULT_VIDEO_PROMPT = "Create a cinematic music video for this song.";
const DEFAULT_PRODUCER_SUPABASE_URL = "https://sb.flowmusic.app";
const DEFAULT_PRODUCER_SUPABASE_ANON_KEY =
  "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJpc3MiOiJzdXBhYmFzZSIsInJlZiI6ImVkbmpjY3FjbWJ4ZWF4YmlkaW5yIiwicm9sZSI6ImFub24iLCJpYXQiOjE3NzE1NjEwNjQsImV4cCI6MjA4NzEzNzA2NH0.XCXSuL7Th1xHecfRrP0vAOFmKwJxwBqVFLu06SxtVzg";


async function main() {
  let currentStage = "init";
  try {
    await appendTrace("stage", { stage: currentStage });
    currentStage = "read-input";
    await appendTrace("stage", { stage: currentStage });
    const input = parseInput(await readStdin());

    currentStage = "resolve-browser";
    await appendTrace("stage", { stage: currentStage });
    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ?? process.env.PRODUCER_BROWSER_EXECUTABLE_PATH ?? null,
    );
    if (!executablePath) {
      throw Object.assign(
        new Error(
          "Unable to locate a Chromium-compatible browser. Set PRODUCER_BROWSER_EXECUTABLE_PATH.",
        ),
        { status: 500, code: "producer_browser_not_found" },
      );
    }

    currentStage = "resolve-session";
    await appendTrace("stage", { stage: currentStage });
    const timeoutMs = normalizeTimeoutMs(input.timeoutMs);
    const cookieHeader = normalizeString(input.cookieHeader);
    const supabaseSession = extractSupabaseSession(cookieHeader);
    const refreshedSession = supabaseSession?.refreshToken
      ? await refreshSupabaseAccessToken(supabaseSession.refreshToken)
      : null;
    const authToken =
      refreshedSession?.accessToken ??
      supabaseSession?.accessToken ??
      normalizeString(input.authToken);
    if (!authToken) {
      throw Object.assign(
        new Error(
          "Producer browser worker requires a bearer token or sb-sb-auth-token cookies carrying a current access token.",
        ),
        { status: 401, code: "producer_browser_missing_auth_token" },
      );
    }

    currentStage = "prepare-browser";
    await appendTrace("stage", { stage: currentStage });
    const baseUrl = normalizeBaseUrl(input.baseUrl);
      const referer =
        normalizeString(input.referer) ?? `${baseUrl.replace(/\/+$/, "")}/library/videos`;
    const origin = normalizeString(input.origin) ?? baseUrl.replace(/\/+$/, "");
    const sourceProfile = resolveBrowserProfileSource(input);
    const clonedProfileRoot = sourceProfile
      ? await cloneBrowserProfile(sourceProfile.userDataDir, sourceProfile.profileDirectory)
      : null;
    let browser = null;
    let context = null;

    try {
      currentStage = "launch-browser";
      await appendTrace("stage", { stage: currentStage });
      const locale = normalizeLocale(input.acceptLanguage) ?? DEFAULT_LOCALE;
      if (clonedProfileRoot) {
        context = await chromium.launchPersistentContext(clonedProfileRoot, {
          executablePath,
          headless: parseBoolean(process.env.PRODUCER_BROWSER_HEADLESS, true),
          locale,
          userAgent: normalizeString(input.userAgent) ?? undefined,
          args: [
            "--disable-blink-features=AutomationControlled",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--no-default-browser-check",
            `--profile-directory=${sourceProfile.profileDirectory}`,
          ],
        });
      } else {
        browser = await chromium.launch({
          executablePath,
          headless: parseBoolean(process.env.PRODUCER_BROWSER_HEADLESS, true),
          args: [
            "--disable-blink-features=AutomationControlled",
            "--disable-dev-shm-usage",
            "--no-first-run",
            "--no-default-browser-check",
          ],
        });
        context = await browser.newContext({
          locale,
          userAgent: normalizeString(input.userAgent) ?? undefined,
        });
      }
      if (cookieHeader) {
        await context.addCookies(parseCookieHeader(cookieHeader, baseUrl));
      }

      currentStage = "open-page";
      const navigationTargets = buildProducerNavigationTargets(baseUrl, referer);
      await appendTrace("stage", {
        stage: currentStage,
        referer,
        navigationTargets,
      });
      const page = context.pages()[0] ?? (await context.newPage());
      page.setDefaultNavigationTimeout(Math.min(timeoutMs, 90_000));
      await openProducerLandingPage(page, navigationTargets, timeoutMs);
      await page.waitForTimeout(800);

      const clipId =
        normalizeString(input.requestBody?.clip_id) ??
        normalizeString(input.requestBody?.clipId) ??
        normalizeString(input.requestBody?.song_id) ??
        normalizeString(input.requestBody?.songId);
      if (clipId) {
        currentStage = "video-flow";
        await appendTrace("stage", { stage: currentStage, clipId });
        const creativePrompt =
          normalizeString(input.requestBody?.user_message) ??
          normalizeString(input.requestBody?.userMessage) ??
          normalizeString(input.requestBody?.prompt) ??
          normalizeString(input.requestBody?.input) ??
          DEFAULT_VIDEO_PROMPT;
        const nodeWorkerResult = await executeProducerConversationVideoFlow({
          page,
          baseUrl,
          authToken,
          cookieHeader,
          clipId,
          creativePrompt,
          requestBody: input.requestBody ?? {},
          acceptAsyncJob: input.acceptAsyncJob !== false,
          timeoutMs,
          initialReferer: referer,
        });
        return nodeWorkerResult;
      }

      currentStage = "page-evaluate";
      await appendTrace("stage", { stage: currentStage });
      const workerResult = await executeProducerPageVideoFlow(
        page,
        {
          authToken,
          baseUrl,
          origin,
          referer,
          requestBody: input.requestBody ?? {},
          model: input.model,
          timeoutMs,
        },
      );

      return workerResult ?? {
        ok: false,
        error: {
          status: 500,
          code: "producer_browser_worker_empty_result",
          message: "Producer browser worker returned an empty result object.",
        },
      };
    } finally {
      await context?.close().catch(() => undefined);
      await browser?.close().catch(() => undefined);
      if (clonedProfileRoot) {
        await rm(clonedProfileRoot, { recursive: true, force: true }).catch(() => undefined);
      }
    }
  } catch (error) {
    await appendTrace("error", {
      stage: currentStage,
      message:
        typeof error?.message === "string" && error.message.trim()
          ? error.message.trim()
          : String(error),
      code:
        typeof error?.code === "string" && error.code.trim() ? error.code.trim() : null,
    });
    return {
      ok: false,
      error: normalizeError(error, currentStage),
    };
  }
}

function normalizeLocale(value) {
  return normalizeString(value)?.split(",")[0]?.trim() ?? null;
}

function buildProducerNavigationTargets(baseUrl, referer) {
  const normalizedBaseUrl = normalizeBaseUrl(baseUrl);
  const targets = [];
  const push = (value) => {
    const normalized = normalizeString(value);
    if (!normalized || targets.includes(normalized)) {
      return;
    }
    targets.push(normalized);
  };
  const isSessionReferer = typeof referer === "string" && /\/session\/[^/?#]+/i.test(referer);
  if (!isSessionReferer) {
    push(referer);
  }
  push(`${normalizedBaseUrl}/library/my-songs`);
  push(`${normalizedBaseUrl}/library/videos`);
  push(normalizedBaseUrl);
  if (isSessionReferer) {
    push(referer);
  }
  return targets;
}

async function openProducerLandingPage(page, navigationTargets, timeoutMs) {
  const errors = [];
  const navigationTimeout = Math.min(timeoutMs, 25_000);
  for (const target of navigationTargets) {
    try {
      await appendTrace("navigation-attempt", {
        target,
        timeoutMs: navigationTimeout,
      });
      await page.goto(target, {
        waitUntil: "commit",
        timeout: navigationTimeout,
      });
      await appendTrace("navigation-success", {
        target,
      });
      return target;
    } catch (error) {
      const message =
        typeof error?.message === "string" && error.message.trim()
          ? error.message.trim()
          : String(error);
      errors.push({ target, message });
      await appendTrace("navigation-failed", {
        target,
        message,
      });
    }
  }

  throw Object.assign(
    new Error(
      `Producer browser worker could not open any landing page: ${JSON.stringify(errors)}`,
    ),
    {
      status: 500,
      code: "producer_browser_navigation_failed",
    },
  );
}

function parseCookieHeader(rawHeader, baseUrl) {
  if (!rawHeader) {
    return [];
  }
  const cookies = [];
  for (const segment of rawHeader.split(";")) {
    const raw = segment.trim();
    if (!raw) {
      continue;
    }
    const separator = raw.indexOf("=");
    if (separator <= 0) {
      continue;
    }
    const name = raw.slice(0, separator).trim();
    const value = raw.slice(separator + 1).trim();
    if (!name || !value) {
      continue;
    }
    cookies.push({
      name,
      value,
      url: `${baseUrl.replace(/\/+$/, "")}/`,
      secure: true,
      sameSite: "Lax",
    });
  }
  return cookies;
}

function extractSupabaseSession(cookieHeader) {
  if (!cookieHeader) {
    return null;
  }
  const cookieMap = new Map();
  for (const segment of cookieHeader.split(";")) {
    const raw = segment.trim();
    if (!raw) {
      continue;
    }
    const separator = raw.indexOf("=");
    if (separator <= 0) {
      continue;
    }
    const name = raw.slice(0, separator).trim();
    const value = raw.slice(separator + 1).trim();
    if (name) {
      cookieMap.set(name, value);
    }
  }

  const chunks = [...cookieMap.entries()]
    .filter(([name]) => /^sb-sb-auth-token\.\d+$/.test(name))
    .sort((left, right) => left[0].localeCompare(right[0], undefined, { numeric: true }))
    .map(([, value]) => safeDecodeURIComponent(value))
    .join("");
  if (!chunks) {
    return null;
  }

  const candidates = [chunks, stripOuterQuotes(chunks), stripOuterQuotes(chunks).replace(/\\"/g, '"')];
  for (const candidate of candidates) {
    const parsed = tryParseJsonDeep(decodeSupabaseCookiePayload(candidate));
    const accessToken =
      parsed?.currentSession?.access_token ??
      parsed?.access_token ??
      parsed?.session?.access_token ??
      null;
    const refreshToken =
      parsed?.currentSession?.refresh_token ??
      parsed?.refresh_token ??
      parsed?.session?.refresh_token ??
      null;
    if (typeof accessToken === "string" && accessToken.trim()) {
      return {
        accessToken: accessToken.trim(),
        refreshToken:
          typeof refreshToken === "string" && refreshToken.trim()
            ? refreshToken.trim()
            : null,
      };
    }
  }

  return null;
}

async function refreshSupabaseAccessToken(refreshToken) {
  const anonKey =
    normalizeString(process.env.PRODUCER_SUPABASE_ANON_KEY) ??
    DEFAULT_PRODUCER_SUPABASE_ANON_KEY;
  const response = await fetch(
    `${DEFAULT_PRODUCER_SUPABASE_URL}/auth/v1/token?grant_type=refresh_token`,
    {
    method: "POST",
    headers: {
      apikey: anonKey,
      authorization: `Bearer ${anonKey}`,
      "content-type": "application/json;charset=UTF-8",
      "x-client-info": "producer-browser-worker/1.0",
    },
    body: JSON.stringify({
      refresh_token: refreshToken,
    }),
    },
  ).catch(() => null);
  if (!response || !response.ok) {
    return null;
  }
  const payload = await response.json().catch(() => null);
  const accessToken = normalizeString(payload?.access_token);
  if (!accessToken) {
    return null;
  }
  return {
    accessToken,
    refreshToken: normalizeString(payload?.refresh_token),
  };
}

function safeDecodeURIComponent(value) {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}

function stripOuterQuotes(value) {
  if (typeof value !== "string") {
    return value;
  }
  return value.replace(/^"+|"+$/g, "");
}

function decodeSupabaseCookiePayload(value) {
  const normalized = stripOuterQuotes(value);
  if (!normalized.startsWith("base64-")) {
    return normalized;
  }
  try {
    return Buffer.from(normalized.slice("base64-".length), "base64").toString("utf8");
  } catch {
    return normalized;
  }
}

function tryParseJsonDeep(value) {
  let current = value;
  for (let index = 0; index < 3; index += 1) {
    if (typeof current !== "string") {
      return current;
    }
    try {
      current = JSON.parse(current);
    } catch {
      return null;
    }
  }
  return current;
}




async function printJsonAndExit(value) {
  const payload = JSON.stringify(value);
  await new Promise((resolve, reject) => {
    process.stdout.write(payload, (error) => {
      if (error) {
        reject(error);
        return;
      }
      resolve();
    });
  });
  process.exit(0);
}

function normalizeError(error, stage = null) {
  const status =
    typeof error?.status === "number" && Number.isFinite(error.status) ? error.status : 500;
  const normalized = {
    status,
    code:
      typeof error?.code === "string" && error.code.trim()
        ? error.code.trim()
        : "producer_browser_worker_failed",
    message:
      typeof error?.message === "string" && error.message.trim()
        ? error.message.trim()
        : String(error),
    body: typeof error?.body === "string" && error.body.trim() ? error.body : undefined,
  };
  if (stage) {
    normalized.stage = stage;
  }
  return normalized;
}

await printJsonAndExit(await main());
