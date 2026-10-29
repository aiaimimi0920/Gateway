// Attach to one existing page only. Closing this socket never closes the user's browser/tab.
export async function connectPage(url, { commandTimeoutMs = 20000 } = {}) {
  const socket = new WebSocket(url);
  const pending = new Map();
  let nextId = 0;
  const fail = () => {
    for (const item of pending.values()) { clearTimeout(item.timer); item.reject(new Error("browser_unavailable")); }
    pending.clear();
  };
  socket.addEventListener("close", fail);
  socket.addEventListener("error", fail);
  socket.addEventListener("message", (event) => {
    if (typeof event.data !== "string" || Buffer.byteLength(event.data) > 512 * 1024) { fail(); socket.close(); return; }
    let message;
    try { message = JSON.parse(event.data); } catch { fail(); socket.close(); return; }
    const item = pending.get(message.id);
    if (!item) return;
    pending.delete(message.id);
    clearTimeout(item.timer);
    if (message.error) item.reject(new Error("browser_command_failed"));
    else item.resolve(message.result);
  });
  try {
    await new Promise((resolve, reject) => {
      const timer = setTimeout(() => { socket.close(); reject(new Error("browser_unavailable")); }, 5000);
      socket.addEventListener("open", () => { clearTimeout(timer); resolve(); }, { once: true });
      socket.addEventListener("error", () => { clearTimeout(timer); reject(new Error("browser_unavailable")); }, { once: true });
    });
  } catch { socket.close(); throw new Error("browser_unavailable"); }
  return {
    command(method, params = {}) {
      return new Promise((resolve, reject) => {
        const id = ++nextId;
        const timer = setTimeout(() => { pending.delete(id); reject(new Error("session_read_timeout")); }, commandTimeoutMs);
        pending.set(id, { resolve, reject, timer });
        try { socket.send(JSON.stringify({ id, method, params })); }
        catch { fail(); }
      });
    },
    close() { fail(); socket.close(); },
  };
}

// Executed in the selected ChatGPT page; no model calls, navigation, or OAuth exchange.
export async function pageSession() {
  // A ready debugger is not a ready document. Never mistake navigation/fetch failure for logout.
  const failure = (code) => ({ failure: code });
  if (location.origin !== "https://chatgpt.com" || document.readyState === "loading") return failure("session_page_unavailable");
  let response;
  try {
    response = await fetch("https://chatgpt.com/api/auth/session", {
      credentials: "same-origin", redirect: "error", headers: { Accept: "application/json" }, signal: AbortSignal.timeout(15000),
    });
  } catch { return failure("session_request_failed"); }
  let rejected;
  if (response.headers.get("cf-mitigated") === "challenge") rejected = "session_verification_required";
  else if (!response.ok) {
    rejected = response.status === 401 ? "login_required" : response.status === 403 ? "session_access_forbidden"
      : response.status === 429 ? "session_rate_limited" : response.status >= 500 ? "session_service_unavailable" : "session_response_invalid";
  } else if (!response.headers.get("content-type")?.includes("application/json")) rejected = "session_response_not_json";
  if (rejected || !response.body) {
    await response.body?.cancel().catch(() => {});
    return failure(rejected || "session_response_invalid");
  }
  const reader = response.body.getReader();
  const chunks = [];
  let length = 0;
  try {
    for (;;) {
      const { done, value } = await reader.read();
      if (done) break;
      length += value.byteLength;
      if (length > 128 * 1024) return failure("session_response_invalid");
      chunks.push(value);
    }
  } catch { return failure("session_request_failed"); }
  finally { await reader.cancel().catch(() => {}); }
  const bytes = new Uint8Array(length);
  let offset = 0;
  for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
  let session;
  try { session = JSON.parse(new TextDecoder().decode(bytes)); }
  catch { return failure("session_response_invalid"); }
  if (!session || typeof session !== "object" || Array.isArray(session)) return failure("session_response_invalid");
  if (location.origin !== "https://chatgpt.com") return failure("session_page_unavailable");
  return {
    accessToken: session.accessToken ?? session.access_token,
    accountName: typeof session.user?.email === "string" ? session.user.email.slice(0, 254) : "",
    userAgent: navigator.userAgent, language: navigator.language,
    timezone: Intl.DateTimeFormat().resolvedOptions().timeZone,
    chatgptPowSources: Array.from(document.querySelectorAll("script[src]"))
      .map((node) => node.getAttribute("src")).filter((src) => src?.includes("/backend-api/sentinel/")).slice(0, 32),
    chatgptPowDataBuild: document.documentElement.getAttribute("data-build") || undefined,
  };
}
