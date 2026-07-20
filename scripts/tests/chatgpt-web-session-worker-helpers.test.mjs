import assert from "node:assert/strict";
import { test } from "node:test";

import {
  isRelayAuthRecoveryState,
  selectNewAssistantTextFromValues,
} from "../chatgpt-web-session-worker-helpers.mjs";

test("selectNewAssistantTextFromValues does not accept stale assistant text", () => {
  const baseline = ["こんにちは！どうしましたか？"];
  const values = ["こんにちは！どうしましたか？"];

  assert.equal(selectNewAssistantTextFromValues(values, baseline), "");
});

test("selectNewAssistantTextFromValues returns the newest assistant delta", () => {
  const baseline = ["旧回复"];
  const values = ["旧回复", "巴黎"];

  assert.equal(selectNewAssistantTextFromValues(values, baseline), "巴黎");
});

test("selectNewAssistantTextFromValues deduplicates repeated DOM matches", () => {
  const baseline = ["旧回复"];
  const values = ["旧回复", "巴黎", "巴黎"];

  assert.equal(selectNewAssistantTextFromValues(values, baseline), "巴黎");
});

test("isRelayAuthRecoveryState detects login and auth-error pages", () => {
  assert.equal(
    isRelayAuthRecoveryState({
      href: "https://chatgpt.com/auth/login_with?callback_path=/",
      bodyText: "Your session has ended. Log in to continue.",
      hasEmail: false,
      hasPassword: false,
    }),
    true,
  );
  assert.equal(
    isRelayAuthRecoveryState({
      href: "https://auth.openai.com/log-in",
      hasEmail: true,
      hasPassword: false,
    }),
    true,
  );
});

test("isRelayAuthRecoveryState ignores a normal chat page", () => {
  assert.equal(
    isRelayAuthRecoveryState({
      href: "https://chatgpt.com/",
      title: "ChatGPT",
      bodyText: "Message ChatGPT",
      hasEmail: false,
      hasPassword: false,
    }),
    false,
  );
});
