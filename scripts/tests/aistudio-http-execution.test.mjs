import assert from "node:assert/strict";
import { createHash } from "node:crypto";
import { test } from "node:test";
import { runHttpRequest } from "../aistudio-http/execution.mjs";

const cookie = name => ({ name, value: "fixture-only", domain: ".google.com", path: "/", secure: true, expires: -1 });
const input = () => ({ cookies: { schemaVersion: 1, provider: "aistudio-web", authUser: "0", cookies:
  [cookie("SAPISID"), cookie("__Secure-1PAPISID"), cookie("__Secure-3PAPISID")] }, model: "fixture",
  body: { contents: [{ role: "user", parts: [{ text: "nonce" }] }] } });

function fixture(sendGeneration) {
  const calls = [];
  const interpreter = "fixture-interpreter";
  const challenge = [["message", [interpreter], [], createHash("sha256").update(interpreter).digest("base64url"), "program", "global", null, "state"]];
  const values = { bootstrap: '"WIu0Nc":"fixture-public-key"', models: JSON.stringify([[["models/fixture"]]]),
    preferences: "[]", logging: '[1,"context"]', challenge: JSON.stringify(challenge) };
  return { calls, send: async (stage, url, options) => {
    calls.push({ stage, url, options });
    return { headers: new Headers(), text: stage === "generate" ? await sendGeneration(options) : values[stage] };
  } };
}

test("one generation uses fresh proof and returns only completed real response", async () => {
  const fake = fixture(options => {
    assert.equal(JSON.parse(options.body)[4], "!fresh-fixture-proof");
    return JSON.stringify([[[[[[[[null, "nonce"]], "model"], 1]]]]]);
  });
  const response = await runHttpRequest(input(), { send: fake.send, makeProof: async value => {
    assert.equal(value.prompt, "nonce"); return "!fresh-fixture-proof";
  } });
  assert.equal(response.candidates[0].content.parts[0].text, "nonce");
  assert.equal(fake.calls.filter(call => call.stage === "generate").length, 1);
});

test("proof failure never sends a generation or falls back to a browser", async () => {
  const fake = fixture(() => { throw new Error("unexpected-generation"); });
  await assert.rejects(runHttpRequest(input(), { send: fake.send, makeProof: async () => {
    throw new Error("aistudio_http_waa_failed");
  } }), /waa_failed/);
  assert.equal(fake.calls.filter(call => call.stage === "generate").length, 0);
});

test("generation failure is terminal and never retried", async () => {
  const fake = fixture(() => { throw new Error("aistudio_http_permission_denied"); });
  await assert.rejects(runHttpRequest(input(), { send: fake.send, makeProof: async () => "!fresh" }), /permission_denied/);
  assert.equal(fake.calls.filter(call => call.stage === "generate").length, 1);
});

test("missing native dependency fails before any authenticated network request", async () => {
  const original = process.env.AISTUDIO_WAA_HELPER_PATH;
  delete process.env.AISTUDIO_WAA_HELPER_PATH;
  let requests = 0;
  try {
    await assert.rejects(runHttpRequest(input(), { send: async () => { requests++; } }), /waa_helper_missing/);
    assert.equal(requests, 0);
  } finally {
    if (original === undefined) delete process.env.AISTUDIO_WAA_HELPER_PATH;
    else process.env.AISTUDIO_WAA_HELPER_PATH = original;
  }
});

test("unverified interpreter bytes never reach the native engine", async () => {
  const fake = fixture(() => { throw new Error("unexpected-generation"); });
  const send = async (...args) => {
    const response = await fake.send(...args);
    if (args[0] === "challenge") {
      const challenge = JSON.parse(response.text);
      challenge[0][3] = "wrong-digest";
      response.text = JSON.stringify(challenge);
    }
    return response;
  };
  await assert.rejects(runHttpRequest(input(), { send, makeProof: async () => {
    throw new Error("unverified-interpreter-was-executed");
  } }), /interpreter_digest_mismatch/);
  assert.equal(fake.calls.filter(call => call.stage === "generate").length, 0);
});

test("media proof binding includes bytes, not only the visible prompt", async () => {
  const value = input();
  value.body.contents[0].parts.push({ inlineData: { mimeType: "application/pdf", data: "YQ==" } });
  const fake = fixture(() => JSON.stringify([[[[[[[[null, "nonce"]], "model"], 1]]]]]));
  const send = async (...args) => {
    if (args[0] === "attachment_token") throw new Error("aistudio_http_drive_authorization_required");
    return fake.send(...args);
  };
  await runHttpRequest(value, { send, makeProof: async proof => {
    assert.equal(proof.prompt, "nonce YQ=="); return "!fresh";
  } });
});
