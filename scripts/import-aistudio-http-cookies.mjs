/** Manual login only. Imports scoped cookies to the credential owner, never storageState or prompts. */
import { existsSync } from "node:fs";
import { mkdir, writeFile } from "node:fs/promises";
import { randomUUID } from "node:crypto";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { chromium } from "playwright-core";
import { openManualLoginBrowser } from "./aistudio-manual-login-browser.mjs";

export const TARGET_URL = "https://aistudio.google.com/prompts/new_chat";
const RPC_ORIGIN = "https://alkalimakersuite-pa.clients6.google.com";
const AUTH_NAMES = ["SAPISID", "__Secure-1PAPISID", "__Secure-3PAPISID"];
const DOMAINS = new Set(["google.com", "aistudio.google.com", "alkalimakersuite-pa.clients6.google.com"]);

export function accountIndex(value) {
  const url = new URL(value);
  if (url.origin !== "https://aistudio.google.com" || url.username || url.password) {
    throw new Error("AI Studio login is not on the expected origin.");
  }
  const result = /^\/u\/(\d+)\//.exec(url.pathname)?.[1] ?? url.searchParams.get("authuser") ?? "0";
  if (!/^\d{1,3}$/.test(result)) throw new Error("Invalid AI Studio account scope.");
  return result;
}

export function cookieBundle(input, authUser) {
  if (!/^\d{1,3}$/.test(authUser)) throw new Error("Invalid AI Studio account scope.");
  const cookies = input.filter(cookie => DOMAINS.has(String(cookie.domain).replace(/^\./, "")) && !cookie.partitionKey);
  if (cookies.length > 64) throw new Error("AI Studio cookie count exceeded the import limit.");
  for (const cookie of cookies) {
    if (!/^[!#$%&'*+.^_`|~0-9A-Za-z-]+$/.test(cookie.name) || /[;\r\n\0]/.test(cookie.value)
      || typeof cookie.value !== "string" || cookie.value.length > 8192
      || !cookie.path?.startsWith("/") || !Number.isFinite(cookie.expires)) {
      throw new Error("AI Studio cookie material is invalid.");
    }
  }
  if (!AUTH_NAMES.every(name => cookies.some(cookie => cookie.name === name && cookie.value
    && (cookie.expires < 0 || cookie.expires > Date.now() / 1000)))) {
    throw new Error("AI Studio signing cookies are not ready; complete normal Google login.");
  }
  // Copy only the browser's scoped Cookie records; never origins, localStorage, IndexedDB or profiles.
  const result = { schemaVersion: 1, provider: "aistudio-web", authUser, cookies };
  if (Buffer.byteLength(JSON.stringify(result)) > 32768) throw new Error("AI Studio cookie bytes exceeded the import limit.");
  return result;
}

export async function captureCookieBundle({ tempRoot, progress } = {}) {
  const executablePath = process.env.AISTUDIO_CAPTURE_BROWSER_EXECUTABLE_PATH ?? [
    "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
    "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
    "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
  ].find(existsSync);
  if (!executablePath || !path.isAbsolute(executablePath) || !existsSync(executablePath)) {
    throw new Error("Set AISTUDIO_CAPTURE_BROWSER_EXECUTABLE_PATH to an installed browser.");
  }
  const session = await openManualLoginBrowser({ chromium, executablePath, targetUrl: TARGET_URL,
    tempRoot: tempRoot ?? process.env.AISTUDIO_CAPTURE_TEMP_ROOT, progress });
  const { browser, context } = session;
  try {
    const page = context.pages().find(item => item.url().startsWith("https://aistudio.google.com/"))
      ?? await context.newPage();
    if (page.url() === "about:blank") await page.goto(TARGET_URL, { waitUntil: "domcontentloaded", timeout: 60000 });
    const deadline = Date.now() + 60_000;
    while (Date.now() < deadline) {
      if (!browser.isConnected()) throw new Error("AI Studio login window was closed.");
      let bundle;
      try {
        const authUser = accountIndex(page.url());
        const surface = await page.locator("ms-prompt-box, textarea").count();
        if (surface) bundle = cookieBundle(await context.cookies([TARGET_URL, RPC_ORIGIN]), authUser);
      } catch { /* Navigation and incomplete login are expected; no generation or challenge automation. */ }
      if (bundle) {
        return bundle;
      }
      await new Promise(resolve => setTimeout(resolve, 1000));
    }
    throw new Error("AI Studio manual login timed out.");
  } finally {
    await session.close();
  }
}

async function main() {
  const root = process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR;
  if (process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER !== "local" || !root || !path.isAbsolute(root)) {
    throw new Error("AI Studio cookie import requires an explicit absolute local credential object root.");
  }
  const bundle = await captureCookieBundle();
  const key = `credential-runtime/aistudio/${randomUUID()}/http-cookies.json`;
  const target = path.join(root, ...key.split("/"));
  await mkdir(path.dirname(target), { recursive: true });
  await writeFile(target, JSON.stringify(bundle), { encoding: "utf8", flag: "wx", mode: 0o600 });
  process.stdout.write(`${JSON.stringify({ ok: true, targetFamily: "aistudio-web", runtimeStateObjectKey: key,
    authUser: bundle.authUser, cookieCount: bundle.cookies.length, storageStateExported: false })}\n`);
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  main().catch(() => {
    process.stdout.write(`${JSON.stringify({ ok: false, error: "AI Studio cookie import failed; private details omitted." })}\n`);
    process.exitCode = 1;
  });
}
