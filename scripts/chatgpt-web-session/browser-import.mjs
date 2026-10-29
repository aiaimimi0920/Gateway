import { connectPage, pageSession } from "./browser-import-cdp.mjs";
import { pageWebCatalog } from "./browser-catalog.mjs";

// Only these fixed codes cross the worker boundary; never forward page exception text/body.
export const SESSION_FAILURE_CODES = ["login_required", "session_page_unavailable", "session_request_failed",
  "session_verification_required", "session_access_forbidden", "session_rate_limited", "session_service_unavailable",
  "session_response_not_json", "session_response_invalid", "session_token_invalid", "session_expired", "session_read_timeout"];

export function validateImportInput(input) {
  if (!input || Object.keys(input).some((key) => !["operation", "port", "targetId"].includes(key)) ||
      !["list", "prepare"].includes(input.operation) || !Number.isInteger(input.port) || input.port < 1024 || input.port > 65535 ||
      [4200, 4300].includes(input.port)) throw new Error("invalid_input");
  if (input.operation === "prepare" && (typeof input.targetId !== "string" || !/^[a-zA-Z0-9_-]{1,128}$/.test(input.targetId))) throw new Error("invalid_input");
  return { operation: input.operation, port: input.port, ...(input.operation === "prepare" ? { targetId: input.targetId } : {}) };
}

const isChatgpt = (value) => {
  try { const url = new URL(value); return url.origin === "https://chatgpt.com" && !url.username && !url.password; }
  catch { return false; }
};
export function selectChatgptTabs(targets) {
  if (!Array.isArray(targets) || targets.length > 512) throw new Error("invalid_targets");
  return targets.filter((target) => target.type === "page" && isChatgpt(target.url) && /^[a-zA-Z0-9_-]{1,128}$/.test(target.id))
    .slice(0, 64).map((target) => ({ id: target.id, title: String(target.title || "ChatGPT").slice(0, 128) }));
}
export function validateDebuggerUrl(value, port) {
  const url = new URL(value);
  if (url.protocol !== "ws:" || url.hostname !== "127.0.0.1" || Number(url.port) !== port || url.username || url.password ||
      !/^\/devtools\/page\/[a-zA-Z0-9_-]+$/.test(url.pathname) || url.search || url.hash) throw new Error("invalid_debugger");
  return url.href;
}
async function readTargets(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`, { redirect: "error", signal: AbortSignal.timeout(5000) });
  if (!response.ok) throw new Error("browser_unavailable");
  const chunks = [];
  let size = 0;
  try {
    for await (const chunk of response.body) {
      size += chunk.byteLength;
      if (size > 256 * 1024) throw new Error("invalid_targets");
      chunks.push(chunk);
    }
  } catch { throw new Error("browser_unavailable"); }
  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}
export function materialFromCapture(capture, cookies) {
  const token = capture?.accessToken;
  if (!token) throw new Error("login_required");
  if (typeof token !== "string" || token.length > 32768 || /[\r\n]/.test(token)) throw new Error("session_token_invalid");
  let expiry;
  try { expiry = JSON.parse(Buffer.from(token.split(".")[1], "base64url")).exp * 1000; } catch { throw new Error("session_token_invalid"); }
  if (!Number.isFinite(expiry) || expiry > 8640000000000000) throw new Error("session_token_invalid");
  if (expiry <= Date.now()) throw new Error("session_expired");
  const scoped = cookies.filter((cookie) => ["chatgpt.com", ".chatgpt.com"].includes(cookie.domain));
  if (scoped.some((cookie) => typeof cookie.name !== "string" || typeof cookie.value !== "string" || /[;\r\n\x00]/.test(cookie.name + cookie.value))) throw new Error("invalid_cookie");
  const cookieHeader = scoped.map((cookie) => `${cookie.name}=${cookie.value}`).join("; ");
  const deviceId = scoped.find((cookie) => cookie.name === "oai-did")?.value;
  if (!cookieHeader || cookieHeader.length > 65536 || !deviceId) throw new Error("incomplete_session");
  return { accountName: capture.accountName || "", material: {
    authToken: token, expiresAt: new Date(expiry).toISOString(), cookieHeader, deviceId,
    userAgent: capture.userAgent, language: capture.language, timezone: capture.timezone,
    chatgptPowSources: capture.chatgptPowSources, chatgptPowDataBuild: capture.chatgptPowDataBuild,
  } };
}
export async function importBrowserSession(raw, { connect = connectPage, catalogDeadline = Infinity } = {}) {
  const input = validateImportInput(raw);
  const targets = await readTargets(input.port);
  const tabs = selectChatgptTabs(targets);
  if (input.operation === "list") return { tabs };
  if (!tabs.some((tab) => tab.id === input.targetId)) throw new Error("tab_unavailable");
  const target = targets.find((entry) => entry.id === input.targetId);
  const cdp = await connect(validateDebuggerUrl(target.webSocketDebuggerUrl, input.port));
  try {
    const result = await cdp.command("Runtime.evaluate", { expression: `(${pageSession.toString()})()`, awaitPromise: true, returnByValue: true });
    if (result.exceptionDetails || !result.result?.value) throw new Error("session_page_unavailable");
    const capture = result.result.value;
    if (capture.failure) throw new Error(SESSION_FAILURE_CODES.includes(capture.failure) ? capture.failure : "session_response_invalid");
    const { cookies } = await cdp.command("Network.getCookies", { urls: ["https://chatgpt.com/"] });
    const imported = materialFromCapture(capture, cookies);
    // A catalog rejection must never discard a successfully read account/session.
    let catalog;
    let timer;
    const catalogBudget = Math.min(10500, catalogDeadline - Date.now() - 500);
    try {
      if (catalogBudget < 1000) throw new Error("catalog_deferred");
      const result = await Promise.race([
        cdp.command("Runtime.evaluate", { expression: `(${pageWebCatalog.toString()})(${JSON.stringify(capture.accessToken)},${JSON.stringify(imported.material.deviceId)})`, awaitPromise: true, returnByValue: true }),
        new Promise((_, reject) => { timer = setTimeout(() => reject(new Error("catalog_deferred")), catalogBudget); }),
      ]);
      catalog = !result.exceptionDetails && result.result?.value;
    } catch { /* Only bounded status metadata crosses the import boundary. */ }
    finally { clearTimeout(timer); }
    if (Array.isArray(catalog?.models) && catalog.models.length) imported.material.modelSlugs = catalog.models;
    return { ...imported, catalog: { code: catalog?.code || (!catalog?.models ? "catalog_unavailable" : undefined), status: catalog?.status } };
  } finally { cdp.close(); }
}
