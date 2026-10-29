// A new WebChannel page is only a trigger: import still requires a real legacy reply.
import { randomUUID } from "node:crypto";

export const LEGACY_STREAM_PATH =
  "/_/BardChatUi/data/assistant.lamda.BardFrontendService/StreamGenerate";
export const VALIDATION_BODY_LIMIT = 1024 * 1024;

export function isProcessSessionValidationRequest(value, postData, prompt) {
  try {
    const url = new URL(value);
    if (url.origin !== "https://geminiweb-pa.clients6.google.com"
      || url.pathname !== "/v1/processSession" || typeof postData !== "string"
      || Buffer.byteLength(postData) > 512 * 1024) return false;
    for (const [key, encoded] of new URLSearchParams(postData)) {
      if (!/^req[0-7]___data__$/.test(key) || encoded.length > 64 * 1024
        || !/^[A-Za-z0-9+/_-]+={0,2}$/.test(encoded)) continue;
      if (Buffer.from(encoded, "base64").includes(Buffer.from(prompt))) return true;
    }
  } catch { /* Unrelated or malformed requests are not authentication evidence. */ }
  return false;
}

export function isLegacyBootstrapSource(value, postData) {
  try {
    const url = new URL(value);
    return url.origin === "https://gemini.google.com"
      && /^\/(?:u\/\d+\/)?_\/BardChatUi\/data\/batchexecute$/.test(url.pathname)
      && typeof postData === "string" && Buffer.byteLength(postData) <= 512 * 1024
      && Boolean(new URLSearchParams(postData).get("at")?.trim());
  } catch { return false; }
}

export function buildLegacyValidationRequest(source, prompt) {
  if (!isLegacyBootstrapSource(source.url, source.postData)) {
    throw new Error("Gemini Web compatibility validation requires an authenticated bootstrap request.");
  }
  const observed = new URL(source.url);
  const target = new URL(LEGACY_STREAM_PATH, "https://gemini.google.com");
  for (const key of ["bl", "f.sid", "hl", "authuser"]) {
    if (observed.searchParams.has(key)) target.searchParams.set(key, observed.searchParams.get(key));
  }
  const account = /^\/u\/(\d+)\//.exec(observed.pathname)?.[1];
  if (account && !target.searchParams.has("authuser")) target.searchParams.set("authuser", account);
  target.searchParams.set("_reqid", String(10000 + Date.now() % 89999));
  target.searchParams.set("rt", "c");
  const input = Array(69).fill(null);
  input[0] = [prompt, 0, null, null, null, null, 0];
  input[1] = [observed.searchParams.get("hl") || "zh-CN"];
  input[2] = ["", "", "", null, null, null, null, null, null, ""];
  for (const [index, value] of [[6, [1]], [7, 1], [10, 1], [11, 0], [17, [[0]]],
    [18, 0], [27, 1], [30, [4]], [41, [1]], [53, 0], [59, randomUUID()], [61, []], [68, 2]]) {
    input[index] = value;
  }
  const headers = { origin: "https://gemini.google.com", accept: "*/*" };
  for (const name of ["referer", "user-agent", "x-goog-authuser"]) {
    if (source.headers?.[name]) headers[name] = source.headers[name];
  }
  if (headers.referer && new URL(headers.referer).origin !== "https://gemini.google.com") {
    throw new Error("Gemini Web compatibility validation rejected a foreign account source.");
  }
  return { url: target.href, headers, form: {
    at: new URLSearchParams(source.postData).get("at"),
    "f.req": JSON.stringify([null, JSON.stringify(input)]),
  } };
}

export function legacyResponseContainsParis(status, body) {
  if (status !== 200 || typeof body !== "string"
    || Buffer.byteLength(body) > VALIDATION_BODY_LIMIT) return false;
  const candidateMatches = (value, depth = 0) => {
    if (!Array.isArray(value) || depth > 12) return false;
    if (value[0] === "wrb.fr" && typeof value[2] === "string") {
      try {
        const payload = JSON.parse(value[2]);
        const text = payload?.[4]?.[0]?.[1]?.[0];
        if (typeof text === "string" && text.includes("巴黎")) return true;
      } catch { /* An error envelope is not an assistant answer. */ }
    }
    return value.some(item => candidateMatches(item, depth + 1));
  };
  for (const line of body.split("\n")) {
    try { if (candidateMatches(JSON.parse(line))) return true; } catch { /* XSSI / lengths. */ }
  }
  return false;
}
