// Standard PKCE challenge; tokens stay in this process and are never refreshed or saved.
import { createHash, randomBytes, randomUUID } from "node:crypto";
import { setTimeout as delay } from "node:timers/promises";
import { endpoints, parseJson, ProbeError, request } from "./transport.mjs";

export function challenge() {
  const verifier = randomBytes(32).toString("base64url");
  const uuid = randomUUID();
  const publicChallenge = createHash("sha256").update(verifier).digest("base64url");
  const url = new URL("https://www.cursor.com/loginDeepControl");
  url.search = new URLSearchParams({
    challenge: publicChallenge, uuid, mode: "login", redirectTarget: "cli",
  });
  return { verifier, uuid, url: url.href };
}

export function tokenClaims(token) {
  if (typeof token !== "string" || token.length > 32768 ||
      !/^[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+$/.test(token)) {
    throw new ProbeError("invalid_login_token");
  }
  let claims;
  try { claims = JSON.parse(Buffer.from(token.split(".")[1], "base64url").toString("utf8")); }
  catch { throw new ProbeError("invalid_token_claims"); }
  if (!claims || typeof claims !== "object" || Array.isArray(claims) ||
      typeof claims.sub !== "string" || !claims.sub ||
      !/^[A-Za-z0-9_|.-]{1,256}$/.test(claims.sub) ||
      !Number.isFinite(claims.exp) || claims.exp * 1000 < Date.now() + 60000) {
    throw new ProbeError("invalid_or_expired_token");
  }
  // These are unverified claims until GetMe authenticates the same full subject.
  return claims;
}

export async function login(auth, options, call = request, wait = delay) {
  const url = new URL(endpoints.poll);
  url.search = new URLSearchParams({ uuid: auth.uuid, verifier: auth.verifier });
  const deadline = Date.now() + 10 * 60000;
  while (Date.now() < deadline) {
    const response = await call(url.href, {
      ...options, method: "GET", headers: { accept: "application/json" },
    });
    if (response.status === 200) {
      const data = parseJson(response);
      const token = data.accessToken ?? data.apiKey;
      tokenClaims(token);
      return token;
    }
    if (![202, 404].includes(response.status)) {
      throw new ProbeError("login_poll_rejected", response.status);
    }
    await wait(2000, undefined, { signal: options.signal });
  }
  throw new ProbeError("login_expired");
}
