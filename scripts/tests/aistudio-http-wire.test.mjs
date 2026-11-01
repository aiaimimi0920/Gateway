import assert from "node:assert/strict";
import { test } from "node:test";
import { buildWire, decodeResponse, parseWire } from "../aistudio-http/wire.mjs";
import { validateBundle, cookieHeader, authorization } from "../aistudio-http/auth.mjs";

const cookie = name => ({ name, value: "fixture-only", domain: ".google.com", path: "/", secure: true, expires: -1 });
const bundle = () => ({ schemaVersion: 1, provider: "aistudio-web", authUser: "0", cookies:
  [cookie("SAPISID"), cookie("__Secure-1PAPISID"), cookie("__Secure-3PAPISID")] });

test("HTTP material is bounded, scoped and signed without browser state", () => {
  const auth = validateBundle(bundle());
  assert.equal(cookieHeader(auth, "https://evil.example/"), "");
  assert.equal(cookieHeader(auth, "http://aistudio.google.com/"), "");
  assert.equal(authorization(auth, 1000).split(" ").length, 6);
  assert.throws(() => validateBundle({ ...bundle(), authUser: "not-index" }));
  assert.throws(() => validateBundle({ ...bundle(), cookies: [cookie("SAPISID")] }));
});

test("Playground wire preserves conversation and binds every text part in order", () => {
  const result = buildWire({ contents: [{ role: "user", parts: [{ text: "one" }] },
    { role: "model", parts: [{ text: "two" }] }, { role: "user", parts: [{ text: "three" }] }],
    systemInstruction: { parts: [{ text: "system" }] }, generationConfig: { maxOutputTokens: 256 } }, "gemini-fixture");
  assert.equal(result.binding, "one two three");
  assert.equal(result.wire[0], "models/gemini-fixture");
  assert.equal(result.wire[5][0][0][1], "system");
  assert.equal(result.wire[3][3], 256);
  assert.equal(result.wire[4], null);
  assert.throws(() => buildWire({ contents: [{ parts: [{ inlineData: { data: "fixture" } }] }] }, "fixture"));
});

test("response requires a real terminal frame and excludes private thought text", () => {
  const candidate = [[[[null, "answer"], [...Array(12).fill(null), true]], "model"], 1];
  const root = [[[[candidate]]]];
  assert.equal(decodeResponse(JSON.stringify(root)).candidates[0].content.parts[0].text, "answer");
  candidate[1] = null;
  assert.throws(() => decodeResponse(JSON.stringify(root)), /incomplete/);
  assert.deepEqual(parseWire(")]}'\n[,1]"), [null, 1]);
  assert.throws(() => parseWire("[] []"));
});

test("equal text deltas are preserved, not mistaken for cumulative prefixes", () => {
  const frame = (text, finish = null) => {
    const content = [[[null, text]], "model"];
    return [[[content, finish]]];
  };
  const response = decodeResponse(JSON.stringify([[frame("ha"), frame("ha"), frame("!", 1)]]));
  assert.equal(response.candidates[0].content.parts[0].text, "haha!");
});
