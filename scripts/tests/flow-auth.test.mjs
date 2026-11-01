import assert from "node:assert/strict";
import test from "node:test";
import { bundleFromCookies, captureSession } from "../flow-auth-worker.mjs";

const cookie = { name: "__Secure-1PSID", value: "fixture-private", domain: ".google.com", path: "/",
  secure: true, expires: -1, httpOnly: true, sameSite: "Lax" };

test("Flow imports only current product-scoped cookies, not full browser state or other providers", () => {
  const result = bundleFromCookies([cookie, { ...cookie, domain: ".flowmusic.app" },
    { ...cookie, name: "expired", expires: 1 }, { ...cookie, domain: "gemini.google.com" },
    { ...cookie, name: "partitioned", partitionKey: "https://foreign.invalid" }]);
  assert.equal(result.provider, "flow-web");
  assert.equal(result.authUser, "0");
  assert.equal(result.cookies.length, 1);
  assert.deepEqual(Object.keys(result).sort(), ["authUser", "cookies", "provider", "schemaVersion"]);
  assert.throws(() => bundleFromCookies([]), /session_missing/);
  assert.throws(() => bundleFromCookies(Array.from({ length: 257 }, () => cookie)), /limit/);
});

test("normal Flow login closes its owned browser even if cookie import fails", async () => {
  let closes = 0;
  const dependencies = { chromium: {}, executablePath: "fixture", open: async options => {
    assert.equal(options.targetUrl, "https://flow.google.com/projects");
    assert.equal(options.disableBackgroundMode, true);
    return { context: { cookies: async urls => { assert.deepEqual(urls, ["https://flow.google.com/"]); return []; } },
      close: async () => { closes++; } };
  } };
  await assert.rejects(captureSession({ tempRoot: "C:/fixture" }, dependencies), /session_missing/);
  assert.equal(closes, 1);
  await assert.rejects(captureSession({ tempRoot: "relative" }, dependencies), /temp_root/);
});

test("HTTPS Flow imports retain ordinary non-Secure cookies without weakening prefixed-cookie rules", () => {
  const ordinary = { ...cookie, name: "SID", value: "synthetic-ordinary", secure: false };
  const result = bundleFromCookies([cookie, ordinary]);
  assert.equal(result.cookies.length, 2);
  assert.equal(result.cookies.find(item => item.name === "SID").secure, false);
  assert.equal(bundleFromCookies([cookie, { ...cookie, name: "__Secure-invalid", secure: false }]).cookies.length, 1);
});
