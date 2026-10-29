import assert from "node:assert/strict";
import test from "node:test";
import { validateImportInput, selectChatgptTabs, validateDebuggerUrl, materialFromCapture } from "../chatgpt-web-session/browser-import.mjs";

test("browser import is loopback-port-only and requires explicit tab selection", () => {
  assert.deepEqual(validateImportInput({ operation: "list", port: 9222 }), { operation: "list", port: 9222 });
  for (const input of [{ port: 0 }, { operation: "list", port: 4200 }, { operation: "prepare", port: 9222 },
    { operation: "list", port: 9222, host: "evil.test" }]) assert.throws(() => validateImportInput(input));
});
test("only exact HTTPS ChatGPT tabs are offered and remote debugger URLs are rejected", () => {
  const tabs = selectChatgptTabs([{ id: "one", type: "page", url: "https://chatgpt.com/c/private", title: "Account tab" },
    { id: "two", type: "page", url: "https://chatgpt.com.evil.test/" }, { id: "three", type: "page", url: "http://chatgpt.com/" }]);
  assert.deepEqual(tabs, [{ id: "one", title: "Account tab" }]);
  assert.equal(validateDebuggerUrl("ws://127.0.0.1:9222/devtools/page/one", 9222), "ws://127.0.0.1:9222/devtools/page/one");
  for (const url of ["ws://evil.test:9222/devtools/page/one", "ws://127.0.0.1:9223/devtools/page/one", "ws://user@127.0.0.1:9222/devtools/page/one"]) {
    assert.throws(() => validateDebuggerUrl(url, 9222));
  }
});
test("capture exposes only Web material and uses real access-token expiry", () => {
  const token = `e30.${Buffer.from(JSON.stringify({ exp: 4070908800 })).toString("base64url")}.sig`;
  const result = materialFromCapture({ accessToken: token, userAgent: "Browser", language: "zh-CN", accountName: "a@example.test" },
    [{ name: "oai-did", value: "device", domain: ".chatgpt.com" }, { name: "session", value: "secret", domain: "chatgpt.com" },
      { name: "other", value: "unrelated", domain: "example.test" }]);
  assert.equal(result.material.expiresAt, "2099-01-01T00:00:00.000Z");
  assert.equal(result.material.deviceId, "device");
  assert.equal(result.accountName, "a@example.test");
  assert.ok(!JSON.stringify(result).includes("unrelated"));
  assert.throws(() => materialFromCapture({ accessToken: "not-a-jwt", userAgent: "Browser" }, []));
});

test("missing, malformed and expired tokens have distinct non-secret failures", () => {
  const token = (exp) => `e30.${Buffer.from(JSON.stringify({ exp })).toString("base64url")}.sig`;
  for (const [accessToken, code] of [[undefined, "login_required"], ["private-not-jwt", "session_token_invalid"],
    [token(1), "session_expired"], [token("invalid"), "session_token_invalid"]]) {
    assert.throws(() => materialFromCapture({ accessToken }, []), (error) => error.message === code);
  }
});
