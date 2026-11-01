/** Normal manual login, then import only the FlowMusic Supabase session through a private pipe. */
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { openManualLoginBrowser } from "./aistudio-manual-login-browser.mjs";

export function validateMaterial(value) {
  if (!value || Object.keys(value).some(key => !["schemaVersion", "provider", "accessToken", "refreshToken"].includes(key))
    || value.schemaVersion !== 1 || value.provider !== "flowmusic-web"
    || typeof value.accessToken !== "string" || !/^[A-Za-z0-9_.-]{3,16384}$/.test(value.accessToken)
    || (value.refreshToken !== undefined && (typeof value.refreshToken !== "string" || !/^[A-Za-z0-9_.-]{1,4096}$/.test(value.refreshToken)))) {
    throw new Error("flowmusic_session_invalid");
  }
  return value;
}

export function sessionFromCookies(cookies) {
  if (!Array.isArray(cookies) || cookies.length > 256) throw new Error("flowmusic_cookie_limit");
  const groups = new Map();
  for (const cookie of cookies) {
    if (!["flowmusic.app", "www.flowmusic.app"].includes(cookie.domain?.replace(/^\./, ""))) continue;
    const match = /^(sb-[\w-]+-auth-token)(?:\.(\d+))?$/.exec(cookie.name);
    if (!match || typeof cookie.value !== "string" || cookie.value.length > 32768) continue;
    const group = groups.get(match[1]) ?? new Map();
    const index = Number(match[2] ?? 0);
    if (index > 16 || group.has(index)) throw new Error("flowmusic_cookie_chunks_invalid");
    group.set(index, decodeURIComponent(cookie.value)); groups.set(match[1], group);
  }
  for (const group of groups.values()) {
    const entries = [...group].sort((a, b) => a[0] - b[0]);
    if (entries.some(([index], position) => index !== position)) continue;
    let value = entries.map(([, text]) => text).join("");
    if (value.length > 32768) throw new Error("flowmusic_cookie_limit");
    try {
      if (value.startsWith("base64-")) value = Buffer.from(value.slice(7), "base64url").toString("utf8");
      let session = JSON.parse(value);
      if (typeof session === "string") session = JSON.parse(session);
      session = session.currentSession ?? session.session ?? session;
      return validateMaterial({ schemaVersion: 1, provider: "flowmusic-web", accessToken: session.access_token,
        ...(session.refresh_token ? { refreshToken: session.refresh_token } : {}) });
    } catch { /* A malformed unrelated cookie must never be accepted as a session. */ }
  }
  throw new Error("flowmusic_login_session_missing");
}

export async function captureSession({ tempRoot }, dependencies = {}) {
  const { chromium } = dependencies.chromium ? dependencies : await import("playwright-core");
  const executablePath = dependencies.executablePath ?? process.env.FLOWMUSIC_CAPTURE_BROWSER_EXECUTABLE_PATH ?? [
    "C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
  ].find(file => fs.existsSync(file));
  if (!executablePath) throw new Error("flowmusic_browser_not_found");
  const owner = await (dependencies.open ?? openManualLoginBrowser)({ chromium, executablePath,
    targetUrl: "https://www.flowmusic.app/", tempRoot, disableBackgroundMode: true, progress: () => {} });
  try {
    return sessionFromCookies(await owner.context.cookies(["https://www.flowmusic.app/", "https://flowmusic.app/"]));
  } finally { await owner.close(); }
}

async function main() {
  let input = "";
  for await (const chunk of process.stdin) {
    input += chunk;
    if (Buffer.byteLength(input) > 4096) throw new Error("flowmusic_import_input_limit");
  }
  const material = await captureSession(JSON.parse(input));
  process.stdout.write(JSON.stringify(material));
}
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  main().catch(() => { process.stdout.write(JSON.stringify({ ok: false, code: "flowmusic_import_failed" })); process.exitCode = 1; });
}
