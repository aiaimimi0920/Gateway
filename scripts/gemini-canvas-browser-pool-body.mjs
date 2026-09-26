const TEXT_BODY_LIMIT_BYTES = 4 * 1024 * 1024;
const BINARY_BODY_LIMIT_BYTES = 16 * 1024 * 1024;
// JSON can expand each one-byte control character to six bytes when a body is serialized.
export const BROWSER_POOL_CONNECTED_CLIENT_FRAME_LIMIT_BYTES = 6 * TEXT_BODY_LIMIT_BYTES + 1024 * 1024;

export const BROWSER_POOL_TEXT_BODY_LIMIT_BYTES = TEXT_BODY_LIMIT_BYTES;
export const BROWSER_POOL_BINARY_BODY_LIMIT_BYTES = BINARY_BODY_LIMIT_BYTES;
export const BROWSER_POOL_BODY_LIMIT_CODE = "gemini_canvas_browser_body_too_large";
export const BROWSER_POOL_BODY_SIZE_UNKNOWN_CODE = "gemini_canvas_browser_body_size_unknown";
// Capability assigned only by the native owner after decoded-byte admission.
export const BOUNDED_NATIVE_RESPONSE = Symbol("bounded native response");

function bodyLimitError(kind, limit, actual = null) {
  const suffix = Number.isSafeInteger(actual) ? ` (${actual} bytes)` : "";
  return Object.assign(
    new Error(`Gemini Canvas browser ${kind} body exceeded ${limit} bytes${suffix}.`),
    {
      status: 413,
      code: BROWSER_POOL_BODY_LIMIT_CODE,
      kind,
      limit,
      actual,
    },
  );
}

function bodySizeUnknownError(kind, limit) {
  return Object.assign(
    new Error(`Gemini Canvas browser ${kind} body has no declared size; refusing a whole-body read.`),
    { status: 502, code: BROWSER_POOL_BODY_SIZE_UNKNOWN_CODE, kind, limit },
  );
}

function headerValue(headers, name) {
  if (!headers) return null;
  if (typeof headers.get === "function") {
    return headers.get(name) ?? headers.get(name.toLowerCase()) ?? null;
  }
  const wanted = name.toLowerCase();
  const entry = Object.entries(headers).find(([key]) => key.toLowerCase() === wanted);
  return entry ? entry[1] : null;
}

function declaredLength(headers) {
  const value = String(headerValue(headers, "content-length") ?? "").trim();
  if (!/^\d+$/.test(value)) return null;
  const length = Number(value);
  return Number.isSafeInteger(length) ? length : null;
}

export function assertDeclaredBodyWithinLimit(headers, limit, kind = "response") {
  const length = declaredLength(headers);
  if (length !== null && length > limit) {
    throw bodyLimitError(kind, limit, length);
  }
  return length;
}

export function assertBytesWithinLimit(bytes, limit, kind = "response") {
  const length = bytes?.byteLength ?? bytes?.length;
  if (!Number.isSafeInteger(length) || length < 0) {
    throw new TypeError("Browser pool body bytes must expose a non-negative length.");
  }
  assertByteLengthWithinLimit(length, limit, kind);
  return bytes;
}

export function assertByteLengthWithinLimit(length, limit, kind = "response") {
  if (!Number.isSafeInteger(length) || length < 0) {
    throw new TypeError("Browser pool body byte length must be a non-negative safe integer.");
  }
  if (length > limit) throw bodyLimitError(kind, limit, length);
  return length;
}

export function assertTextWithinLimit(text, limit = TEXT_BODY_LIMIT_BYTES, kind = "response") {
  if (typeof text !== "string") {
    throw new TypeError("Browser pool body text must be a string.");
  }
  const bytes = Buffer.byteLength(text, "utf8");
  if (bytes > limit) throw bodyLimitError(kind, limit, bytes);
  return text;
}

function hasNoPlaywrightBody(response) {
  const status = response?.status?.();
  const method = response?.request?.()?.method?.()?.toUpperCase();
  return method === "HEAD" || [204, 205, 304].includes(status);
}

function admitPlaywrightBodyRead(response, limit, kind) {
  if (hasNoPlaywrightBody(response)) return false;
  if (response?.[BOUNDED_NATIVE_RESPONSE] === true) return true;
  const length = assertDeclaredBodyWithinLimit(response?.headers?.(), limit, kind);
  if (length === null) throw bodySizeUnknownError(kind, limit);
  return true;
}

export async function readPlaywrightResponseText(response, limit = TEXT_BODY_LIMIT_BYTES, kind = "response") {
  if (!admitPlaywrightBodyRead(response, limit, kind)) return "";
  return assertTextWithinLimit(await response.text(), limit, kind);
}

export async function readPlaywrightResponseBody(response, limit = BINARY_BODY_LIMIT_BYTES, kind = "response") {
  if (!admitPlaywrightBodyRead(response, limit, kind)) return new Uint8Array(0);
  return assertBytesWithinLimit(await response.body(), limit, kind);
}

async function readStreamedResponseBody(reader, limit, kind) {
  const chunks = [];
  let total = 0;
  let complete = false;
  let cancelled = false;
  try {
    while (true) {
      const { done, value } = await reader.read();
      if (done) { complete = true; break; }
      if (!(value instanceof Uint8Array)) throw new TypeError("Fetch response stream chunks must be Uint8Array values.");
      if (value.byteLength > limit - total) {
        cancelled = true;
        await reader.cancel().catch(() => undefined);
        throw bodyLimitError(kind, limit, total + value.byteLength);
      }
      total += value.byteLength;
      chunks.push(value);
    }
  } catch (error) {
    if (!complete && !cancelled) {
      cancelled = true;
      await reader.cancel(error).catch(() => undefined);
    }
    throw error;
  } finally {
    try { reader.releaseLock?.(); } catch { /* Keep the read result or error. */ }
  }
  const bytes = new Uint8Array(total);
  let offset = 0;
  for (const chunk of chunks) {
    bytes.set(chunk, offset);
    offset += chunk.byteLength;
  }
  return bytes;
}

export async function readFetchResponseBody(response, limit = BINARY_BODY_LIMIT_BYTES, kind = "response") {
  if ([204, 205, 304].includes(response?.status) || response?.body === null) return new Uint8Array(0);
  const length = assertDeclaredBodyWithinLimit(response?.headers, limit, kind);
  const reader = response?.body?.getReader?.();
  if (reader) return assertBytesWithinLimit(await readStreamedResponseBody(reader, limit, kind), limit, kind);
  if (length === null) throw bodySizeUnknownError(kind, limit);
  return assertBytesWithinLimit(new Uint8Array(await response.arrayBuffer()), limit, kind);
}

export function encodeBodyBase64(bytes) {
  return Buffer.from(assertBytesWithinLimit(bytes, BINARY_BODY_LIMIT_BYTES, "binary")).toString("base64");
}
