import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { collectGeminiPageAuthSignal } from "./export-gemini-canvas-auth-signal.mjs";

const GOOGLE_API_KEY_REGEX = /AIza[0-9A-Za-z_-]{20,}/g;

function pushUniqueString(values, candidate) {
  const normalized = normalizeString(candidate);
  if (!normalized || values.includes(normalized)) {
    return;
  }
  values.push(normalized);
}

function extractGoogleApiKeysFromBlob(blob) {
  const text = String(blob ?? "");
  const matches = text.match(GOOGLE_API_KEY_REGEX) ?? [];
  const keys = [];
  for (const candidate of matches) {
    pushUniqueString(keys, candidate);
  }
  return keys;
}

async function collectGeminiPageRuntimeArtifacts(page) {
  const blobs = [];
  try {
    blobs.push(await page.content());
  } catch (error) {
    if (!isTransientNavigationError(error)) {
      throw error;
    }
  }
  try {
    const serializedRuntime = await page.evaluate(() =>
      JSON.stringify({
        firebaseConfig: globalThis.firebaseConfig ?? null,
        WIZ_global_data: globalThis.WIZ_global_data ?? null,
        __NEXT_DATA__: globalThis.__NEXT_DATA__ ?? null,
        APP_INITIALIZATION_STATE: globalThis.APP_INITIALIZATION_STATE ?? null,
      }),
    );
    blobs.push(serializedRuntime);
  } catch (error) {
    if (!isTransientNavigationError(error)) {
      throw error;
    }
  }

  const apiKeys = [];
  for (const blob of blobs) {
    for (const candidate of extractGoogleApiKeysFromBlob(blob)) {
      pushUniqueString(apiKeys, candidate);
    }
  }
  return {
    apiKeys,
  };
}

async function collectGeminiRuntimeMaterial(context, fallbackPage, preferredUrl) {
  const pageEntries = await inspectGeminiPages(context, fallbackPage);
  const preferred = normalizeString(preferredUrl);
  const preferredIndex = pageEntries.findIndex(
    (entry) => normalizeString(entry.url) === preferred,
  );
  const orderedEntries =
    preferredIndex >= 0
      ? [
          pageEntries[preferredIndex],
          ...pageEntries.filter((_, index) => index !== preferredIndex),
        ]
      : pageEntries;
  const apiKeys = [];

  for (const entry of orderedEntries) {
    const artifacts = await collectGeminiPageRuntimeArtifacts(entry.page);
    for (const candidate of artifacts.apiKeys) {
      pushUniqueString(apiKeys, candidate);
    }
  }

  return {
    apiKeys,
  };
}

async function inspectGeminiPages(context, fallbackPage) {
  const pages = context.pages();
  const seen = new Set();
  const entries = [];
  for (const page of [...pages, fallbackPage].filter(Boolean)) {
    if (seen.has(page)) {
      continue;
    }
    seen.add(page);
    try {
      const url = page.url();
      const pageSignal = await collectGeminiPageAuthSignal(page);
      entries.push({
        page,
        url,
        pageSignal,
      });
    } catch (error) {
      if (!isTransientNavigationError(error)) {
        throw error;
      }
    }
  }
  return entries;
}

function isTransientNavigationError(error) {
  const message = error instanceof Error ? error.message : String(error);
  return (
    message.includes("Execution context was destroyed") ||
    message.includes("Target page, context or browser has been closed") ||
    message.includes("Cannot find context with specified id")
  );
}

export {
  extractGoogleApiKeysFromBlob,
  collectGeminiRuntimeMaterial,
  inspectGeminiPages,
  isTransientNavigationError,
};
