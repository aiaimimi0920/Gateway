// Executed only in the authenticated, explicitly selected ChatGPT page. Read-only: no generation or challenge actions.
export async function pageWebCatalog(accessToken, deviceId) {
  const failed = (code, status) => ({ code, ...(status ? { status } : {}) });
  if (location.origin !== "https://chatgpt.com") return failed("catalog_unavailable");
  let response;
  try {
    response = await fetch("https://chatgpt.com/backend-api/models?history_and_training_disabled=false", {
      credentials: "same-origin", redirect: "error", signal: AbortSignal.timeout(10000),
      headers: { Accept: "application/json", Authorization: `Bearer ${accessToken}`, "Oai-Device-Id": deviceId },
    });
    if (!response.ok) {
      await response.body?.cancel().catch(() => {});
      return failed(response.status === 403 ? "catalog_verification_required"
        : response.status === 401 ? "catalog_session_invalid" : "catalog_http", response.status);
    }
    if (!response.body || response.headers.get("content-type")?.split(";")[0].trim().toLowerCase() !== "application/json") {
      await response.body?.cancel().catch(() => {});
      return failed("catalog_not_json", response.status);
    }
    const reader = response.body.getReader();
    const chunks = [];
    let length = 0;
    try {
      for (;;) {
        const { done, value } = await reader.read();
        if (done) break;
        length += value.byteLength;
        if (length > 1024 * 1024) return failed("catalog_invalid", response.status);
        chunks.push(value);
      }
    } finally { await reader.cancel().catch(() => {}); }
    const bytes = new Uint8Array(length);
    let offset = 0;
    for (const chunk of chunks) { bytes.set(chunk, offset); offset += chunk.length; }
    let body;
    try { body = JSON.parse(new TextDecoder().decode(bytes)); }
    catch { return failed("catalog_invalid", response.status); }
    if (location.origin !== "https://chatgpt.com") return failed("catalog_unavailable");
    if (!Array.isArray(body?.models) || body.models.length > 2048) return failed("catalog_invalid", response.status);
    const models = [...new Set(body.models.map((entry) => entry?.slug).filter((slug) => typeof slug === "string"
      && slug.length > 0 && slug.trim() === slug && new TextEncoder().encode(slug).length <= 200 && !/[\p{Cc}*?]/u.test(slug)))];
    return models.length ? { status: response.status, models } : failed("catalog_empty", response.status);
  } catch { return failed("catalog_unavailable", response?.status); }
}
