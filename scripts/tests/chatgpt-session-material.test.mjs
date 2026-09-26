import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import os from "node:os";
import path from "node:path";
import test from "node:test";
import {
  buildCredentialPayload,
  extractChatGptUiConversationMaterial,
  maybeWriteCredentialFile,
  resolveCredentialFilePath,
} from "../chatgpt-web-session/credentials.mjs";
import {
  decodeJwtExpIso,
  dedupeCookies,
  normalizeCookie,
  parseCookieHeader,
  parseCookieNames,
} from "../chatgpt-web-session/cookies.mjs";

test("cookie header parsing retains equals signs and drops nameless entries", () => {
  const cookies = parseCookieHeader("session=part=two; ; =invalid; other= value ", "chatgpt.com");
  assert.deepEqual(cookies.map(({ name, value }) => [name, value]), [["session", "part=two"], ["other", "value"]]);
  assert.equal(cookies[0].domain, "chatgpt.com");
  assert.equal(cookies[0].secure, true);
  assert.deepEqual(parseCookieHeader(null, "chatgpt.com"), []);
  assert.deepEqual(parseCookieNames("a=1; b=2;"), ["a", "b"]);
});

test("cookie normalization and duplicate replacement preserve browser shape", () => {
  assert.equal(normalizeCookie(null, "chatgpt.com"), null);
  const first = normalizeCookie({ name: " session ", value: 1, domain: ".chatgpt.com", expires: Infinity }, null);
  assert.deepEqual(first, {
    name: "session", value: "1", domain: "chatgpt.com", path: "/",
    secure: true, httpOnly: false, expires: undefined, sameSite: "Lax",
  });
  const replacement = { ...first, value: "2" };
  assert.deepEqual(dedupeCookies([first, replacement]), [replacement]);
});

test("JWT expiry extraction is tolerant parsing, not token verification", () => {
  const payload = Buffer.from(JSON.stringify({ exp: 1000 })).toString("base64url");
  assert.equal(decodeJwtExpIso(`header.${payload}.signature`), "1970-01-01T00:16:40.000Z");
  assert.equal(decodeJwtExpIso("invalid"), null);
  assert.equal(decodeJwtExpIso("header.e30.signature"), null);
});

test("latest conversation material overrides fallback client metadata without mutation", () => {
  const requests = [
    { url: "https://chatgpt.com/backend-api/f/conversation", headers: { "oai-client-version": "old" } },
    { url: "invalid", headers: {} },
    { url: "https://chatgpt.com/backend-api/f/conversation", headers: { "oai-client-version": "new", "x-conduit-token": "synthetic" } },
  ];
  const result = { clientVersion: "fallback", relayResponse: { uiRequestCapture: { rawHeaders: true, requests } } };
  const before = JSON.stringify(result);
  const material = extractChatGptUiConversationMaterial(result);
  assert.equal(material.clientVersion, "new");
  const payload = buildCredentialPayload(result);
  assert.equal(payload.adapter, "chatgpt_web_reverse_compatible");
  assert.equal(payload.baseUrl, "https://chatgpt.com");
  assert.equal(payload.extraBody.clientVersion, "new");
  assert.equal(payload.headers["X-Conduit-Token"], "synthetic");
  assert.equal(payload.rawSource.uiConversationMaterial.hasXConduitToken, true);
  assert.equal(JSON.stringify(result), before);
});

test("credential target preserves explicit extension and sanitized account fallback", () => {
  assert.equal(resolveCredentialFilePath({ credentialFilePath: "explicit.JSON" }, {}), "explicit.JSON");
  assert.equal(resolveCredentialFilePath({ credentialRootDir: "root", credentialFamilyDir: "family" }, { accountName: " A Person@example.test " }), path.join("root", "family", "a-person-example.test.json"));
});

test("credential file materialization skips failed results and writes the existing payload contract", async (t) => {
  const root = await mkdtemp(path.join(os.tmpdir(), "gateway-chatgpt-credential-test-"));
  t.after(() => rm(root, { recursive: true, force: true }));
  const input = { credentialFilePath: path.join(root, "nested", "fixture") };
  assert.equal(await maybeWriteCredentialFile(input, { ok: false }), null);
  const result = { ok: true, authToken: "synthetic-token", cookieHeader: "session=synthetic" };
  const target = await maybeWriteCredentialFile(input, result);
  assert.equal(target, `${input.credentialFilePath}.json`);
  assert.deepEqual(JSON.parse(await readFile(target, "utf8")), buildCredentialPayload(result));
});
