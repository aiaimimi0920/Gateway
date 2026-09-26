import assert from "node:assert/strict";
import test from "node:test";
import { importTestableScript } from "./gemini-canvas-browser-pool.fixtures.mjs";

const app = await importTestableScript();
const origin = "https://gemini.google.com";
const target = "https://generativelanguage.googleapis.com/v1beta/probe";
const cookie = (name, value) => Object.freeze({ name, value });
const first = cookie("__Secure-1PAPISID", "synthetic-first");
const third = cookie("__Secure-3PAPISID", "synthetic-third");
const basic = cookie("SAPISID", "synthetic-basic");
const entry = (cookies, page = { url: () => `${origin}/u/3/app?authuser=5` }) => ({ context: { cookies: async () => cookies }, page });

test("Google auth account query takes precedence and malformed inputs retain the primary fallback", () => {
  for (const [input, expected] of [
    [`${origin}/u/3/app?authuser=05`, "05"], [`${origin}/u/3/app?authuser=bad`, "3"],
    [`${origin}/u/3/app?authuser=`, "3"], [`${origin}/u/3x/app`, "0"],
    [`${origin}/u/7/app`, "7"], ["broken URL", "0"], [null, "0"], [" ", "0"],
  ]) assert.equal(app.inferGoogleAuthUser(input), expected);
});

test("Google fetch authentication skips unrelated targets before accessing cookies or page state", async () => {
  const inaccessible = { get context() { assert.fail("unrelated target must not read cookies"); } };
  assert.deepEqual(await app.buildGoogleFetchAuthHeaders(inaccessible, "invalid base", "https://fixture.invalid/probe"), {});
});

test("Google fetch authentication retains cookie priority and fixed SAPISID wire vectors", async (t) => {
  t.mock.method(Date, "now", () => 1700000000123);
  const cases = [
    [[basic, third, first], "eaaef3346ef96123c56c5a8f2152cb7c35de1eca"],
    [[basic, third], "96bc9f60fd9b55cfcd04689e04c9b0f9eb633b85"],
    [[basic], "e7b97b8d22d1d234d60a755a9e710bd55d219366"],
  ];
  for (const [cookies, digest] of cases) {
    const result = await app.buildGoogleFetchAuthHeaders(entry(Object.freeze(cookies)), origin, target);
    assert.deepEqual(result, {
      Authorization: `SAPISIDHASH 1700000000_${digest} SAPISID1PHASH 1700000000_${digest} SAPISID3PHASH 1700000000_${digest}`,
      "X-Origin": origin, "X-Goog-AuthUser": "5",
    });
  }
});

test("Google fetch authentication scopes cookie reads to the preferred origin and falls back to target origin", async () => {
  const reads = [], e = entry([basic]);
  e.context.cookies = async (urls) => { reads.push(urls); return [basic]; };
  const preferred = await app.buildGoogleFetchAuthHeaders(e, ` ${origin}/u/2/app?fixture=yes `, target);
  const fallback = await app.buildGoogleFetchAuthHeaders(e, " ", target);
  assert.deepEqual(reads, [[origin, target], ["https://generativelanguage.googleapis.com", target]]);
  assert.equal(preferred["X-Origin"], origin);
  assert.equal(fallback["X-Origin"], "https://generativelanguage.googleapis.com");
});

test("Google fetch authentication without a nonempty identity returns before reading the page", async () => {
  for (const cookies of [[], [cookie("SID", "synthetic")], [cookie("SAPISID", "")]]) {
    const e = entry(cookies, { url: () => assert.fail("missing identity must not read page") });
    assert.deepEqual(await app.buildGoogleFetchAuthHeaders(e, origin, target), {});
  }
});

test("Google fetch authentication propagates invalid URLs and cookie-store failures", async () => {
  const error = new Error("synthetic cookie-store failure");
  const e = { context: { cookies: async () => { throw error; } } };
  await assert.rejects(app.buildGoogleFetchAuthHeaders(e, origin, "bad URL"), { code: "ERR_INVALID_URL" });
  await assert.rejects(app.buildGoogleFetchAuthHeaders(e, "bad base", target), { code: "ERR_INVALID_URL" });
  await assert.rejects(app.buildGoogleFetchAuthHeaders(e, origin, target), (actual) => actual === error);
});

test("Google fetch authentication tolerates absent page accessors and preserves account selection", async () => {
  for (const page of [null, {}, { url: () => "malformed" }]) {
    const result = await app.buildGoogleFetchAuthHeaders(entry([basic], page), origin, target);
    assert.equal(result["X-Goog-AuthUser"], "0");
  }
  const result = await app.buildGoogleFetchAuthHeaders(entry([basic], { url: () => `${origin}/u/8/app` }), origin, target);
  assert.equal(result["X-Goog-AuthUser"], "8");
});

test("Google fetch authentication retains cookie parser trimming and last-value precedence", async (t) => {
  t.mock.method(Date, "now", () => 1700000000123);
  const cookies = [cookie("SAPISID", "synthetic-obsolete"), cookie("SAPISID", " synthetic-basic "), cookie("__Secure-1PAPISID", " ")];
  const result = await app.buildGoogleFetchAuthHeaders(entry(cookies), origin, target);
  assert.equal(result.Authorization, "SAPISIDHASH 1700000000_e7b97b8d22d1d234d60a755a9e710bd55d219366 SAPISID1PHASH 1700000000_e7b97b8d22d1d234d60a755a9e710bd55d219366 SAPISID3PHASH 1700000000_e7b97b8d22d1d234d60a755a9e710bd55d219366");
});
