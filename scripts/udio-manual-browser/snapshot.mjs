import { createHash } from "node:crypto";
import { writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";
import { normalizeString } from "./settings.mjs";
import { ensureParentDir, putObject, writeJsonFile } from "./persistence.mjs";

const UDIO_AUTH_COOKIE_PREFIX = "sb-ssr-production-auth-token";

export function sha1(value) {
  return createHash("sha1").update(value).digest("hex");
}

export function hasUdioAuthCookies(cookies) {
  return cookies.some((cookie) => {
    const name = String(cookie?.name ?? "");
    return name === UDIO_AUTH_COOKIE_PREFIX || name.startsWith(`${UDIO_AUTH_COOKIE_PREFIX}.`);
  });
}

export function buildCookieHeader(cookies) {
  return cookies
    .map((cookie) => {
      const name = normalizeString(cookie?.name);
      const value = normalizeString(cookie?.value);
      return name && value ? `${name}=${value}` : null;
    })
    .filter(Boolean)
    .join("; ");
}

export async function readAccountSnapshot(page) {
  const fallback = {
    title: await page.title().catch(() => null),
    currentUrl: page.url() || null,
    heading: null,
    accountName: null,
    accountEmail: null,
    localStorageKeys: [],
  };
  try {
    return await page.evaluate(() => {
      const trim = (value) => (typeof value === "string" && value.trim() ? value.trim() : null);
      const storageKeys = Object.keys(localStorage);
      let accountName = null;
      let accountEmail = null;
      for (const key of storageKeys) {
        const lowered = key.toLowerCase();
        if (!lowered.includes("auth") && !lowered.includes("user") && !lowered.includes("account")) {
          continue;
        }
        const raw = trim(localStorage.getItem(key));
        if (!raw) {
          continue;
        }
        try {
          const parsed = JSON.parse(raw);
          if (!parsed || typeof parsed !== "object") {
            continue;
          }
          accountName =
            trim(parsed.name) ??
            trim(parsed.full_name) ??
            trim(parsed.username) ??
            trim(parsed.user?.name) ??
            trim(parsed.user?.full_name) ??
            accountName;
          accountEmail =
            trim(parsed.email) ??
            trim(parsed.user?.email) ??
            trim(parsed.currentSession?.user?.email) ??
            trim(parsed.session?.user?.email) ??
            accountEmail;
        } catch {
          // Ignore non-JSON auth cache entries.
        }
      }
      return {
        title: trim(document.title),
        currentUrl: trim(location.href),
        heading:
          trim(document.querySelector("h1")?.textContent ?? null) ??
          trim(document.querySelector("[role='heading']")?.textContent ?? null),
        accountName,
        accountEmail,
        localStorageKeys: storageKeys,
      };
    });
  } catch {
    return fallback;
  }
}

export async function exportAuthSnapshot({
  context,
  page,
  profileSource,
  storageStatePath,
  statusPath,
  objectKey,
  lastSnapshotHash,
  lastRemoteHash,
}) {
  const cookies = await context.cookies(["https://www.udio.com", "https://udio.com"]);
  const authenticated = hasUdioAuthCookies(cookies);
  const cookieHeader = buildCookieHeader(cookies);
  const pageSnapshot = await readAccountSnapshot(page);
  const storageState = await context.storageState();
  const storageText = JSON.stringify(storageState, null, 2);
  const storageHash = sha1(storageText);
  let runtimeStateAbsolutePath = null;

  if (authenticated && storageHash !== lastSnapshotHash.value) {
    await ensureParentDir(storageStatePath);
    await writeFile(path.resolve(process.cwd(), storageStatePath), storageText, "utf8");
    lastSnapshotHash.value = storageHash;
  }

  if (authenticated && objectKey && storageHash !== lastRemoteHash.value) {
    runtimeStateAbsolutePath = await putObject(objectKey, Buffer.from(storageText, "utf8"));
    lastRemoteHash.value = storageHash;
  }

  const status = {
    ok: true,
    pid: process.pid,
    startedAt: globalThis.__UDIO_MANUAL_HELPER_STARTED_AT,
    updatedAt: new Date().toISOString(),
    authenticated,
    targetUrl: globalThis.__UDIO_MANUAL_HELPER_TARGET_URL,
    currentUrl: pageSnapshot.currentUrl,
    title: pageSnapshot.title,
    heading: pageSnapshot.heading,
    accountName: pageSnapshot.accountName,
    accountEmail: pageSnapshot.accountEmail,
    profileSource,
    cookieCount: cookies.length,
    cookieHeader,
    storageStatePath: path.resolve(process.cwd(), storageStatePath),
    runtimeStateObjectKey: objectKey,
    runtimeStateAbsolutePath,
    latestGenerateCapturePath: path.resolve(
      process.cwd(),
      globalThis.__UDIO_MANUAL_HELPER_GENERATE_CAPTURE_PATH,
    ),
  };

  await writeJsonFile(statusPath, status);
  return status;
}
