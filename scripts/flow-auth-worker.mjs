/** Google Flow normal login only. Cookie result stays in the private Rust subprocess pipe. */
import fs from "node:fs";
import path from "node:path";
import { pathToFileURL } from "node:url";
import { openManualLoginBrowser } from "./aistudio-manual-login-browser.mjs";

export function bundleFromCookies(cookies) {
  if (!Array.isArray(cookies) || cookies.length > 256) throw new Error("flow_cookie_limit");
  const selected = cookies.filter(cookie => [".google.com", "flow.google.com", ".flow.google.com"].includes(cookie.domain)
    && cookie.partitionKey == null && (!/^(?:__Secure-|__Host-)/.test(cookie.name) || cookie.secure)
    && (cookie.expires === -1 || cookie.expires > Date.now() / 1000)).map(cookie => ({
      name: cookie.name, value: cookie.value, domain: cookie.domain, path: cookie.path, expires: cookie.expires,
      secure: cookie.secure, httpOnly: cookie.httpOnly, sameSite: cookie.sameSite,
    }));
  if (!selected.some(cookie => cookie.name === "__Secure-1PSID" && cookie.value && cookie.path === "/")) {
    throw new Error("flow_login_session_missing");
  }
  const bundle = { schemaVersion: 1, provider: "flow-web", authUser: "0", cookies: selected };
  if (selected.length > 64 || Buffer.byteLength(JSON.stringify(bundle)) > 32768) throw new Error("flow_cookie_limit");
  return bundle;
}

export async function captureSession({ tempRoot }, dependencies = {}) {
  if (typeof tempRoot !== "string" || !path.isAbsolute(tempRoot)) throw new Error("flow_temp_root_required");
  const { chromium } = dependencies.chromium ? dependencies : await import("playwright-core");
  const executablePath = dependencies.executablePath ?? process.env.FLOW_CAPTURE_BROWSER_EXECUTABLE_PATH ?? [
    "C:/Program Files/Google/Chrome/Application/chrome.exe", "C:/Program Files (x86)/Microsoft/Edge/Application/msedge.exe",
    "C:/Program Files/Microsoft/Edge/Application/msedge.exe",
  ].find(file => fs.existsSync(file));
  if (!executablePath) throw new Error("flow_browser_not_found");
  const owner = await (dependencies.open ?? openManualLoginBrowser)({ chromium, executablePath,
    targetUrl: "https://flow.google.com/projects", tempRoot, disableBackgroundMode: true, progress: () => {} });
  try { return bundleFromCookies(await owner.context.cookies(["https://flow.google.com/"])); }
  finally { await owner.close(); }
}

async function main() {
  let input = "";
  for await (const chunk of process.stdin) {
    input += chunk;
    if (Buffer.byteLength(input) > 4096) throw new Error("flow_import_input_limit");
  }
  process.stdout.write(JSON.stringify(await captureSession(JSON.parse(input))));
}
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  main().catch(() => { process.stdout.write(JSON.stringify({ ok: false, code: "flow_import_failed" })); process.exitCode = 1; });
}
