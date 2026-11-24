// Private executor metadata only. Never return the browser cookie jar to media callers.
export async function sessionReceipt(context, input, result) {
  if (!input?.sessionBinding) return undefined;
  const binding = input.sessionBinding;
  if (typeof binding !== "string" || !binding.length || binding.length > 128) {
    throw new Error("Invalid Luma session binding.");
  }
  const host = new URL(input.baseUrl).hostname;
  const cookies = context ? (await context.cookies())
    .filter((cookie) => cookie.name === "wos-session") : [];
  if (cookies.length > 1) throw new Error("Ambiguous Luma session Cookie scope.");
  const cookie = cookies.length === 1 ? cookies[0] : null;
  if (cookie && (cookie.domain.replace(/^\./, "") !== host || cookie.path !== "/"
    || !cookie.httpOnly || !cookie.secure || !["Lax", "Strict"].includes(cookie.sameSite)
    || !cookie.value.length || cookie.value.length > 32768 || /[^\x21-\x7e]|[;,"\\]/.test(cookie.value))) {
    throw new Error("Invalid Luma session Cookie scope.");
  }
  return { version: 1, binding, cookie,
    reauthenticationRequired: Boolean(context && !cookie) || result.error?.status === 401 };
}

// A bounded authenticated GET checks the session without dispatching an action or consuming credits.
export async function checkSession(page, realmId, timeoutMs) {
  return page.evaluate(async ({ realmId, timeoutMs }) => {
    const controller = new AbortController();
    const timer = setTimeout(() => controller.abort(), timeoutMs);
    try {
      const response = await fetch(`/api/vespa/realms/${encodeURIComponent(realmId)}/signature`,
        { credentials: "include", redirect: "manual", signal: controller.signal });
      if (response.ok && response.headers.get("content-type")?.includes("application/json")) {
        const reader = response.body?.getReader();
        if (!reader) throw new Error("Missing session check body.");
        let bytes = 0;
        const chunks = [];
        try {
          for (;;) {
            const { done, value } = await reader.read();
            if (done) break;
            bytes += value.length;
            if (bytes > 65536) throw new Error("Session check response exceeded limit.");
            chunks.push(value);
          }
        } finally { await reader.cancel(); }
        const data = new Uint8Array(bytes);
        let offset = 0;
        for (const chunk of chunks) { data.set(chunk, offset); offset += chunk.length; }
        const value = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(data));
        if (typeof value.ws_token === "string" && value.ws_token.length > 0) return { ok: true };
      } else { await response.body?.cancel(); }
      return { ok: false, error: { code: "lumalabs_session_check_failed", stage: "session",
        status: response.type === "opaqueredirect" || response.ok ? 401 : response.status,
        message: "Luma Web session check was rejected." } };
    } finally { clearTimeout(timer); }
  }, { realmId, timeoutMs });
}
