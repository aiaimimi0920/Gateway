import test from "node:test";
import assert from "node:assert/strict";

import {
  DEFAULT_TARGET_URL,
  STREAM_GENERATE_PATH,
  extractAccountIndex,
  extractGeminiWebRuntime,
  isGeminiWebStreamGenerateUrl,
  requestContainsValidationPrompt,
} from "../export-gemini-web-runtime.mjs";

test("targets the second Gemini account", () => {
  assert.equal(DEFAULT_TARGET_URL, "https://gemini.google.com/u/1/");
  assert.equal(extractAccountIndex(DEFAULT_TARGET_URL), "1");
});

test("matches only Gemini Web StreamGenerate requests", () => {
  assert.equal(
    isGeminiWebStreamGenerateUrl(`https://gemini.google.com${STREAM_GENERATE_PATH}?authuser=1`),
    true,
  );
  assert.equal(
    isGeminiWebStreamGenerateUrl(
      "https://biz-discoveryengine.googleapis.com/v1alpha/locations/global/widgetStreamAssist",
    ),
    false,
  );
});

test("requires the exact validation prompt in f.req", () => {
  const matching = new URLSearchParams({ "f.req": '[null,"[[\\"法国的首都在哪里\\",0]]"]' });
  const other = new URLSearchParams({ "f.req": '[null,"[[\\"法国首都\\",0]]"]' });
  assert.equal(requestContainsValidationPrompt(matching.toString()), true);
  assert.equal(requestContainsValidationPrompt(other.toString()), false);
});

test("extracts account-scoped Gemini Web runtime without Business fields", () => {
  const postData = new URLSearchParams({ at: "access-1", "f.req": "payload" }).toString();
  const runtime = extractGeminiWebRuntime({
    requestUrl: `https://gemini.google.com${STREAM_GENERATE_PATH}?bl=boq_assistant-bard-web-server_1&f.sid=-123&hl=zh-CN&authuser=1`,
    postData,
    headers: {
      referer: "https://gemini.google.com/u/1/app",
      "x-goog-authuser": "1",
      "x-goog-ext-525001261-jspb": '[1,null,null,null,"model"]',
    },
    cookies: [
      { name: "__Secure-1PSID", value: "primary-cookie", domain: ".google.com" },
      { name: "__Secure-1PSIDTS", value: "secondary-cookie", domain: ".google.com" },
    ],
    currentUrl: "https://gemini.google.com/u/1/app",
  });

  assert.equal(runtime.apiKey, "primary-cookie");
  assert.equal(runtime.authToken, "secondary-cookie");
  assert.equal(runtime.accountIndex, "1");
  assert.equal(runtime.appPagePath, "/u/1/app");
  assert.equal(runtime.accessToken, "access-1");
  assert.equal(runtime.buildLabel, "boq_assistant-bard-web-server_1");
  assert.equal(runtime.sessionId, "-123");
  assert.equal(runtime.endpointPath, STREAM_GENERATE_PATH);
  assert.equal("configId" in runtime, false);
  assert.equal("session" in runtime, false);
});

