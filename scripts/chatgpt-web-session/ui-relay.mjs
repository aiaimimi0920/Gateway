import { normalizeString } from "./configuration.mjs";
import { collectPageState } from "./page-state.mjs";
import { loginExistingAccount } from "./account-login.mjs";
import { navigateWithChallengeSettle } from "./navigation.mjs";
import { createChatGptUiRequestCapture, finalizeChatGptUiRelayCapture } from "./ui-capture.mjs";
import { isRelayAuthRecoveryState, selectNewAssistantTextFromValues } from "../chatgpt-web-session-worker-helpers.mjs";

export function extractRelayPrompt(requestBody) {
  const messages = Array.isArray(requestBody?.messages) ? requestBody.messages : [];
  const firstMessage = messages.find((item) => item?.author?.role === "user") ?? messages[0];
  const parts = firstMessage?.content?.parts;
  if (Array.isArray(parts)) {
    const text = parts
      .map((part) => (typeof part === "string" ? part.trim() : ""))
      .filter(Boolean)
      .join("\n");
    if (text) {
      return text;
    }
  }
  return null;
}

export async function waitForVisibleChatGptEditor(page) {
  await page.waitForFunction(
    () => {
      const selectors = [
        "#prompt-textarea",
        'div.ProseMirror[contenteditable="true"]',
        '[contenteditable="true"]',
        "textarea",
      ];
      return selectors.some((selector) =>
        Array.from(document.querySelectorAll(selector)).some((node) => {
          const style = window.getComputedStyle(node);
          const rect = node.getBoundingClientRect();
          return (
            style.display !== "none" &&
            style.visibility !== "hidden" &&
            rect.width > 0 &&
            rect.height > 0
          );
        }),
      );
    },
    undefined,
    { timeout: 120_000 },
  );
  return page.locator(
    '#prompt-textarea:visible, div.ProseMirror[contenteditable="true"]:visible, [contenteditable="true"]:visible, textarea:visible',
  );
}

export async function dismissChatGptBlockingOverlays(page) {
  await page.keyboard.press("Escape").catch(() => {});
  const selectors = [
    '#modal-onboarding button[aria-label*="Close"]',
    '#modal-onboarding button[aria-label*="关闭"]',
    '#modal-onboarding button:has-text("开始")',
    '#modal-onboarding button:has-text("继续")',
    '#modal-onboarding button:has-text("稍后")',
    '#modal-onboarding button:has-text("跳过")',
    '#modal-onboarding button:has-text("Done")',
    '#modal-onboarding button:has-text("Continue")',
    '#modal-onboarding button:has-text("Skip")',
    '#modal-onboarding button:has-text("Maybe later")',
    '[data-testid="modal-onboarding"] button[aria-label*="Close"]',
    '[data-testid="modal-onboarding"] button:has-text("开始")',
    '[data-testid="modal-onboarding"] button:has-text("继续")',
    '[data-testid="modal-onboarding"] button:has-text("稍后")',
    '[data-testid="modal-onboarding"] button:has-text("跳过")',
    '[data-testid="modal-onboarding"] button:has-text("Continue")',
    '[data-testid="modal-onboarding"] button:has-text("Skip")',
    '[data-testid="modal-onboarding"] button:has-text("Done")',
  ];
  for (const selector of selectors) {
    const button = page.locator(selector).first();
    if ((await button.count().catch(() => 0)) <= 0) {
      continue;
    }
    await button.click({ timeout: 2_000, force: true }).catch(() => {});
    await page.waitForTimeout(500);
  }
  await page
    .evaluate(() => {
      const overlays = Array.from(
        document.querySelectorAll('#modal-onboarding, [data-testid="modal-onboarding"]'),
      );
      for (const overlay of overlays) {
        overlay.remove();
      }
    })
    .catch(() => {});
}

export async function collectChatGptAssistantTexts(page) {
  return page.evaluate(() => {
    const selectors = [
      '[data-message-author-role="assistant"]',
      'article [data-message-author-role="assistant"]',
      '[data-testid^="conversation-turn-"] [data-message-author-role="assistant"]',
    ];
    const values = [];
    for (const selector of selectors) {
      for (const node of document.querySelectorAll(selector)) {
        const text = String(node.innerText || node.textContent || "").trim();
        if (text) {
          values.push(text);
        }
      }
    }
    return Array.from(new Set(values));
  });
}

export async function recoverChatGptUiRelayAuthIfNeeded(page, relayInput) {
  const pageState = await collectPageState(page).catch(() => null);
  if (!isRelayAuthRecoveryState(pageState)) {
    return false;
  }
  if (!relayInput?.authSeed?.email || !relayInput?.authSeed?.password) {
    return false;
  }
  await loginExistingAccount(
    page,
    relayInput.baseUrl,
    relayInput.authUrl,
    relayInput.authSeed,
    relayInput.timeoutMs ?? 120_000,
    () => {},
    relayInput.mailboxContext ?? null,
  );
  await navigateWithChallengeSettle(
    page,
    `${relayInput.baseUrl}/`,
    relayInput.timeoutMs ?? 120_000,
  );
  await page.waitForTimeout(3_000);
  await dismissChatGptBlockingOverlays(page);
  return true;
}

export async function prepareChatGptUiRelayPage(page, relayInput) {
  await page.bringToFront().catch(() => {});
  await page.waitForLoadState("domcontentloaded").catch(() => {});
  await page.waitForTimeout(2_000);
  await dismissChatGptBlockingOverlays(page);
  await recoverChatGptUiRelayAuthIfNeeded(page, relayInput);
  await dismissChatGptBlockingOverlays(page);
}

export async function submitChatGptUiPrompt(page, prompt) {
  await dismissChatGptBlockingOverlays(page);
  const editor = await waitForVisibleChatGptEditor(page);
  const editorTag = await editor
    .first()
    .evaluate((node) => ({
      tagName: node.tagName,
      contentEditable: node.getAttribute("contenteditable"),
    }));
  if (
    String(editorTag?.tagName || "").toLowerCase() === "textarea" ||
    String(editorTag?.contentEditable || "").toLowerCase() !== "true"
  ) {
    await editor.first().fill(prompt);
  } else {
    await editor.first().click({ force: true });
    await page.keyboard.press("Control+A").catch(() => {});
    await page.keyboard.type(prompt, { delay: 5 });
  }
  const sendButton = page.locator(
    'button[data-testid="send-button"]:visible, button[aria-label*="Send"]:visible, button[aria-label*="发送"]:visible, button[aria-label*="送信"]:visible',
  );
  if (
    (await sendButton.count().catch(() => 0)) > 0 &&
    (await sendButton.first().isEnabled().catch(() => false))
  ) {
    await sendButton.first().click();
  } else {
    await editor.first().press("Enter");
  }
  await page.waitForLoadState("domcontentloaded").catch(() => {});
}

export async function runChatGptBrowserUiRelay(page, relayInput) {
  const prompt = extractRelayPrompt(relayInput?.requestBody);
  if (!prompt) {
    return {
      requirementsStatus: 200,
      requirementsContentType: "application/json",
      requirementsPreview: "{\"detail\":\"ui relay skipped: missing prompt\"}",
      status: 500,
      contentType: "application/json",
      bodyText: "{\"detail\":\"ui relay missing prompt\"}",
      bodyPreview: "{\"detail\":\"ui relay missing prompt\"}",
    };
  }

  await prepareChatGptUiRelayPage(page, relayInput);
  const beforeAssistantTexts = await collectChatGptAssistantTexts(page);
  const uiRequestCapture = createChatGptUiRequestCapture(page);

  try {
    await submitChatGptUiPrompt(page, prompt);

    const startedAt = Date.now();
    let stableCount = 0;
    let lastText = "";
    let recoveryAttempted = false;
    while (Date.now() - startedAt < 90_000) {
      await page.waitForTimeout(1_500);
      if (!recoveryAttempted) {
        const pageState = await collectPageState(page).catch(() => null);
        const authPage = isRelayAuthRecoveryState(pageState);
        if (
          authPage &&
          relayInput?.authSeed?.email &&
          relayInput?.authSeed?.password
        ) {
          recoveryAttempted = true;
          await recoverChatGptUiRelayAuthIfNeeded(page, relayInput);
          await submitChatGptUiPrompt(page, prompt);
          continue;
        }
      }
      let snapshot;
      try {
        snapshot = await page.evaluate(() => {
          const stopVisible = Array.from(document.querySelectorAll("button"))
          .some((node) => {
            const label = String(
              node.getAttribute("aria-label") ||
                node.textContent ||
                "",
            ).toLowerCase();
            return label.includes("stop") || label.includes("停止");
          });
          return {
            stopVisible,
          };
        });
        snapshot.assistantText = selectNewAssistantTextFromValues(
          await collectChatGptAssistantTexts(page),
          beforeAssistantTexts,
        );
      } catch (error) {
        if (String(error?.message || error).toLowerCase().includes("execution context was destroyed")) {
          await page.waitForLoadState("domcontentloaded").catch(() => {});
          continue;
        }
        throw error;
      }

      const candidate = normalizeString(snapshot.assistantText) ?? "";
      if (candidate && candidate === lastText && !snapshot.stopVisible) {
        stableCount += 1;
      } else if (candidate) {
        stableCount = 1;
        lastText = candidate;
      }
      if (candidate && stableCount >= 2) {
        const syntheticSse = `data: ${candidate}\n\ndata: [DONE]\n\n`;
        return finalizeChatGptUiRelayCapture(uiRequestCapture, {
          requirementsStatus: 200,
          requirementsContentType: "application/json",
          requirementsPreview: "{\"detail\":\"ui relay fallback used\"}",
          status: 200,
          contentType: "text/event-stream",
          bodyText: syntheticSse,
          bodyPreview: syntheticSse.slice(0, 1000),
        });
      }
    }

    const timeoutState = await page
      .evaluate(() => ({
        href: location.href,
        title: document.title,
        bodyPreview: String(document.body?.innerText || "").slice(0, 2000),
      }))
      .catch(() => ({
        href: null,
        title: null,
        bodyPreview: "",
      }));

    return finalizeChatGptUiRelayCapture(uiRequestCapture, {
      requirementsStatus: 200,
      requirementsContentType: "application/json",
      requirementsPreview: "{\"detail\":\"ui relay timeout\"}",
      status: 504,
      contentType: "application/json",
      bodyText: JSON.stringify({
        detail: "ui relay timed out waiting for assistant output",
        state: timeoutState,
      }),
      bodyPreview: JSON.stringify({
        detail: "ui relay timed out waiting for assistant output",
        state: timeoutState,
      }).slice(0, 1000),
    });
  } finally {
    uiRequestCapture.detach();
  }
}
