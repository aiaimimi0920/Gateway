/** Fixed destinations, decoded-byte admission, one request per stage, no redirect or retry. */
export function createTransport(timeoutMs = 120000) {
  const deadline = Date.now() + Math.max(1000, Math.min(timeoutMs, 180000));
  const hosts = new Set(["aistudio.google.com", "alkalimakersuite-pa.clients6.google.com", "waa-pa.clients6.google.com", "www.google.com"]);
  return async function send(stage, address, options, cap = 2 * 1024 * 1024) {
    const url = new URL(address), remaining = deadline - Date.now();
    if (url.protocol !== "https:" || !hosts.has(url.hostname) || url.username || url.password || url.port) throw new Error("aistudio_http_destination_rejected");
    if (remaining <= 0) throw new Error("aistudio_http_deadline_exceeded");
    const response = await fetch(address, { ...options, redirect: "manual", signal: AbortSignal.timeout(remaining) });
    if (stage === "bootstrap" && response.status >= 300 && response.status < 400) {
      let target;
      try { target = new URL(response.headers.get("location"), address); } catch {}
      if (target?.origin === "https://accounts.google.com") {
        await response.body?.cancel();
        const error = new Error("aistudio_http_login_required");
        error.status = 401;
        throw error; // Normal manual reauthentication only; never follow a sign-in/challenge redirect.
      }
    }
    const reader = response.body?.getReader(), chunks = []; let size = 0;
    if (!reader) throw new Error("aistudio_http_body_missing");
    try {
      while (true) {
        const part = await reader.read();
        if (part.done) break;
        size += part.value.byteLength;
        if (size > cap) { await reader.cancel(); throw new Error("aistudio_http_body_limit"); }
        chunks.push(part.value);
      }
    } finally { reader.releaseLock(); }
    if (response.status !== 200) {
      let driveConsent = false;
      if (stage === "attachment_token" && response.status === 401) {
        try { const rpc = JSON.parse(Buffer.concat(chunks, size).toString("utf8"));
          driveConsent = rpc?.[0] === 16 && typeof rpc[1] === "string" && /\bunauthorized_client\b/i.test(rpc[1]);
        } catch { /* Unknown errors remain authentication failures; never expose their body. */ }
      }
      const error = new Error(driveConsent ? "aistudio_http_drive_authorization_required" : response.status === 403 ? "aistudio_http_permission_denied"
        : response.status === 401 ? "aistudio_http_login_required" : "aistudio_http_upstream_failed");
      error.status = response.status;
      throw error; // Never place authentication headers or arbitrary upstream errors in logs/output.
    }
    return { text: Buffer.concat(chunks, size).toString("utf8"), headers: response.headers };
  };
}
