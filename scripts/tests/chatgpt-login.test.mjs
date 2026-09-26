import assert from "node:assert/strict";
import test from "node:test";
import vm from "node:vm";
import { loginExistingAccount, fillEmailVerificationCode, maybeCompleteEmailVerification } from "../chatgpt-web-session/account-login.mjs";
import { navigateToEmailSurface, waitForAuthCookieState, clickButtonByExactText } from "../chatgpt-web-session/auth-navigation.mjs";
import { collectPageState, pageStateIsCloudflareWait, pageStateRequiresAuthRecovery, pageStateRequiresEmailVerification, safePageUrl } from "../chatgpt-web-session/page-state.mjs";

test("page state executes its browser callback with bounded returned previews", async () => {
  const document = {
    title: "Fixture", readyState: "complete", body: { innerText: "x".repeat(800) },
    querySelector: () => null,
    querySelectorAll: () => Array.from({ length: 20 }, () => ({ innerText: "button", getAttribute: () => "/auth/login_with" })),
  };
  const result = await collectPageState({ evaluate: async (callback) => vm.runInNewContext(`(${callback})()`, {
    document, location: { href: "https://chatgpt.com/" },
  }) });
  assert.equal(result.bodyText.length, 400);
  assert.equal(result.buttonTexts.length, 12);
  assert.equal(result.authLinks.length, 8);
  assert.equal(result.hasEmail, false);
});

test("page classifiers retain existing recovery and verification markers", () => {
  assert.equal(pageStateIsCloudflareWait({ title: "Just a moment" }), true);
  assert.equal(pageStateRequiresAuthRecovery({ bodyText: "Your session has ended" }), true);
  assert.equal(pageStateRequiresEmailVerification({ bodyText: "Check your inbox" }), true);
  assert.equal(pageStateRequiresAuthRecovery({ href: "https://chatgpt.com/" }), false);
  assert.equal(safePageUrl({ url: () => { throw new Error("closed fixture"); } }), "");
});

test("account login rejects missing seed before accessing the page", async () => {
  await assert.rejects(loginExistingAccount(null, "https://chatgpt.com", null, {}, 100), {
    status: 401, code: "chatgpt_web_browser_login_seed_missing",
  });
});

test("OTP fill keeps six-digit validation and supports single and split inputs", async () => {
  await assert.rejects(fillEmailVerificationCode(null, "12345"), { code: "chatgpt_web_mailbox_code_invalid" });
  for (const count of [1, 6]) {
    const values = [];
    const field = { fill: async (value) => { values.push(value); } };
    const page = { locator: () => ({ count: async () => count, first: () => field, nth: () => field }) };
    await fillEmailVerificationCode(page, "123456");
    assert.deepEqual(values, count === 1 ? ["123456"] : ["1", "2", "3", "4", "5", "6"]);
  }
});

test("non-verification page skips mailbox interaction", async () => {
  const page = { evaluate: async () => ({ href: "https://chatgpt.com/", bodyText: "Hello" }) };
  assert.equal(await maybeCompleteEmailVerification(page, null, 100), false);
});

test("email navigation returns the first ready input surface", async () => {
  const visited = [];
  const state = { hasEmail: true };
  const page = { goto: async (url) => { visited.push(url); }, waitForTimeout: async () => {}, evaluate: async () => state };
  assert.equal(await navigateToEmailSurface(page, { baseUrl: "https://chatgpt.com", authUrl: "https://auth.openai.com/log-in", timeoutMs: 100 }), state);
  assert.deepEqual(visited, ["https://auth.openai.com/log-in"]);
});

test("auth readiness requires selected cookies and stable rg_context", async () => {
  const context = { cookies: async () => [{ name: "login_session", value: "synthetic" }, { name: "rg_context", value: "stb" }] };
  const result = await waitForAuthCookieState(context, { evaluate: async () => ({}) }, {
    urls: ["https://auth.openai.com"], timeoutMs: 100, cookieNames: ["login_session"], requireRgContextStb: true,
  });
  assert.equal(result.ready, true);
  assert.equal(result.rgContext, "stb");
});

test("exact button matching avoids partial labels and retains click fallback", async () => {
  const clicks = [];
  const buttons = [
    { innerText: async () => "Continue with another account", click: async () => { clicks.push("wrong"); } },
    { innerText: async () => " Continue ", click: async () => { throw new Error("fixture"); }, evaluate: async () => { clicks.push("fallback"); return true; } },
  ];
  assert.equal(await clickButtonByExactText({ locator: () => ({ elementHandles: async () => buttons }), waitForTimeout: async () => {} }, ["Continue"]), true);
  assert.deepEqual(clicks, ["fallback"]);
});
