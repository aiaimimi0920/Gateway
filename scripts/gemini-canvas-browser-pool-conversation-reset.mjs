import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { clickFirstVisible, bodyTextIndicatesGeminiAppSurface, bodyIndicatesGeminiSignedOutLanding } from "./gemini-canvas-browser-pool-app.mjs";

export function createConversationResetOwner({ log }) {
  async function clickNewChat(page) {
    const candidates = [
      page.getByRole("button", { name: /发起新对话|New chat/i }).first(),
      page.locator('button[aria-label*="发起新对话"], button[aria-label*="New chat"]').first(),
      page.getByRole("link", { name: /发起新对话|New chat/i }).first(),
    ];
    const clicked = await clickFirstVisible(candidates);
    if (clicked) {
      await page.waitForTimeout(1200);
    }
    return clicked;
  }

  async function resetConversation(
    page,
    baseUrl,
    timeoutMs,
    preferredEntryUrl = null,
    options = {},
  ) {
    const appUrl = normalizeString(preferredEntryUrl) || `${baseUrl.replace(/\/+$/, "")}/app`;
    const preserveProgramContext =
      Boolean(normalizeString(preferredEntryUrl)) &&
      /\/app\/|\/canvas(?:\/|$)/i.test(String(preferredEntryUrl));

    const skipInitialNavigationWhenAppSurfaceReady =
      options?.skipInitialNavigationWhenAppSurfaceReady === true;
    let currentPageUrl = normalizeString(page.url()) ?? "";
    let currentBodyText = "";
    if (skipInitialNavigationWhenAppSurfaceReady) {
      currentBodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
    }
    const currentPageLooksReusable =
      skipInitialNavigationWhenAppSurfaceReady &&
      currentPageUrl.startsWith(baseUrl.replace(/\/+$/, "")) &&
      /\/app(?:\/|$|\?)/i.test(currentPageUrl) &&
      bodyTextIndicatesGeminiAppSurface(currentBodyText) &&
      !bodyIndicatesGeminiSignedOutLanding(currentBodyText);

    if (!currentPageLooksReusable) {
      await page.goto(appUrl, {
        waitUntil: "domcontentloaded",
        timeout: timeoutMs,
      });
      await page.waitForTimeout(1200);
      await dismissGeminiAppInterstitials(page, timeoutMs);
    } else {
      log(
        "resetConversation reusing attached app surface without hard reload",
        JSON.stringify({
          appUrl,
          currentPageUrl,
        }),
      );
    }

    if (preserveProgramContext) {
      return;
    }

    const newChatCandidates = [
      page.getByRole("button", { name: /发起新对话|New chat/i }).first(),
      page.locator('button[aria-label*="发起新对话"], button[aria-label*="New chat"]').first(),
      page.getByRole("link", { name: /发起新对话|New chat/i }).first(),
      page.getByRole("button", { name: /^Gemini$/i }).first(),
      page.getByRole("link", { name: /Gemini/i }).first(),
    ];

    for (const candidate of newChatCandidates) {
      try {
        if ((await candidate.count()) > 0) {
          await candidate.click({
            timeout: Math.min(timeoutMs, 10_000),
            force: true,
          });
          await page.waitForTimeout(1200);
          await dismissGeminiAppInterstitials(page, timeoutMs);
          break;
        }
      } catch {
        // Best effort only; navigation to /app already gives us a fresh-enough baseline.
      }
    }
  }

  async function dismissGeminiAppInterstitials(page, timeoutMs) {
    const candidates = [
      page.getByRole("button", { name: /以后再说|稍后再说|Not now|Maybe later/i }).first(),
      page.getByRole("link", { name: /以后再说|稍后再说|Not now|Maybe later/i }).first(),
      page.getByRole("button", { name: /知道了|Got it|Close/i }).first(),
      page.getByRole("button", { name: /跳过|Skip/i }).first(),
    ];

    for (const candidate of candidates) {
      try {
        await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 2_000) });
        await candidate.click({ timeout: Math.min(timeoutMs, 6_000), force: true });
        await page.waitForTimeout(800);
      } catch {
        // Best effort only.
      }
    }
  }

  return { clickNewChat, resetConversation, dismissGeminiAppInterstitials };
}
