/** Public page context and WAA challenge; source digest is checked before any JS execution. */
import { createHash } from "node:crypto";
import { ORIGIN, RPC, USER_AGENT, matchingCookies, requestHeaders } from "./auth.mjs";
import { parseWire } from "./wire.mjs";

const WAA_URL = "https://waa-pa.clients6.google.com/$rpc/google.internal.waa.v1.Waa/Create";
export function loggingExtension(fields) {
  if (!Array.isArray(fields) || fields.length > 128) throw new Error("aistudio_http_logging_context_invalid");
  const bytes = [];
  const varint = n => {
    if (n < 0n) n = BigInt.asUintN(64, n);
    while (n >= 128n) { bytes.push(Number(n & 127n) | 128); n >>= 7n; }
    bytes.push(Number(n));
  };
  fields.forEach((value, index) => {
    if (value == null) return;
    const field = BigInt(index + 1);
    if (typeof value === "string") {
      const data = Buffer.from(value);
      if (data.length > 8192) throw new Error("aistudio_http_logging_context_invalid");
      varint(field << 3n | 2n); varint(BigInt(data.length)); bytes.push(...data);
    } else if (typeof value === "boolean" || Number.isSafeInteger(value)) {
      varint(field << 3n); varint(BigInt(value));
    } else throw new Error("aistudio_http_logging_context_invalid");
  });
  return Buffer.from(bytes).toString("base64");
}

function challengeFields(outer) {
  if (typeof outer?.[1] === "string" && outer[1]) {
    const bytes = Buffer.from(outer[1], "base64");
    for (let i = 0; i < bytes.length; i++) bytes[i] = bytes[i] + 97 & 255;
    return JSON.parse(bytes.toString("utf8"));
  }
  if (!Array.isArray(outer?.[0])) throw new Error("aistudio_http_challenge_invalid");
  return outer[0];
}

function profile(bundle, page, model) {
  const globals = {}, toggles = /window\["_F_toggles_default_MakerSuite"\] = \[([^\]]*)\]/.exec(page.text)?.[1];
  if (toggles) globals._F_toggles_default_MakerSuite = toggles.split(",").map(value => Number(value.trim())).filter(Number.isSafeInteger);
  for (const match of page.text.matchAll(/var (_F_cssRowKey|_F_combinedSignature) = '([^']*)'/g)) globals[match[1]] = match[2];
  return { PageURL: `${ORIGIN}/prompts/new_chat?model=${encodeURIComponent(model.replace(/^models\//, ""))}`,
    Locale: "zh-CN", TimeZone: "Asia/Shanghai", HasFocus: true, NavigationStart: new Date().toISOString(),
    TrustedTypes: /require-trusted-types-for\s+'script'/.test(page.headers.get("content-security-policy") ?? ""), Globals: globals,
    Window: { outerWidth: 1440, outerHeight: 900, innerWidth: 1440, innerHeight: 843, screenX: 0, screenY: 0,
      screenLeft: 0, screenTop: 0, devicePixelRatio: 1, mozInnerScreenX: 0 },
    Navigator: { userAgent: USER_AGENT, appCodeName: "Mozilla", appName: "Netscape", appVersion: "5.0 (Windows)",
      platform: "Win32", oscpu: "Windows NT 10.0; Win64; x64", language: "zh-CN", languages: ["zh-CN", "zh"],
      hardwareConcurrency: 4, product: "Gecko", maxTouchPoints: 0, doNotTrack: "unspecified", globalPrivacyControl: false },
    Screen: { width: 1920, height: 1080, availWidth: 1920, availHeight: 1040, availLeft: 0, availTop: 0, colorDepth: 24, pixelDepth: 24 },
    Document: { cookie: matchingCookies(bundle, `${ORIGIN}/`).filter(c => !c.httpOnly).map(c => `${c.name}=${c.value}`).join("; "),
      lastModified: new Date().toLocaleString("en-US", { timeZone: "Asia/Shanghai", hour12: false }).replace(",", "") } };
}

export async function prepareProof(send, bundle, page, key, visit, prompt, model) {
  const headers = requestHeaders(bundle, key, visit);
  const logging = await send("logging", `${RPC}GetLoggingContext`, { method: "POST", headers, body: "[]" });
  const extension = loggingExtension(parseWire(logging.text));
  if (extension) headers["X-Goog-Ext-519733851-Bin"] = extension;
  // These are the public webpage WAA project/request keys, not a user's cloud API credentials.
  const challenge = await send("challenge", WAA_URL, { method: "POST",
    headers: requestHeaders(bundle, "AIzaSyBGb5fGAyC-pRcRU6MUHb__b_vKha71HRE", visit, WAA_URL),
    body: JSON.stringify(["lmnUSbltwc5ULv48iKLX"]) });
  const outer = parseWire(challenge.text), fields = challengeFields(outer);
  let interpreter = fields?.[1]?.find(value => typeof value === "string" && value);
  if (!interpreter) {
    const raw = fields?.[2]?.find(value => typeof value === "string" && value);
    const url = new URL(raw?.startsWith("//") ? `https:${raw}` : raw);
    if (url.origin !== "https://www.google.com" || !url.pathname.startsWith("/js/bg/") || url.username || url.password) {
      throw new Error("aistudio_http_interpreter_url_rejected");
    }
    interpreter = (await send("interpreter", url.href, { headers: { "User-Agent": USER_AGENT, Referer: `${ORIGIN}/`, Accept: "*/*" } })).text;
  }
  if (createHash("sha256").update(interpreter).digest("base64url") !== fields[3]) throw new Error("aistudio_http_interpreter_digest_mismatch");
  return { headers, input: { challenge: outer, interpreter, profile: profile(bundle, page, model), prompt } };
}
