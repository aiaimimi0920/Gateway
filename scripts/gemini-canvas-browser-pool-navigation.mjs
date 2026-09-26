import { normalizeString, scopeGeminiUrlToAuthUser } from "./gemini-canvas-browser-pool-input.mjs";
import { hasPromptTextbox, bodyTextIndicatesGeminiAppSurface } from "./gemini-canvas-browser-pool-app.mjs";

export function createProgramNavigationOwner({ log, collectButtonSnapshot }) {
  function resolveProgramPageUrl(baseUrl, args) {
    const isConcreteProgramUrl = (candidate) => {
      const value = normalizeString(candidate);
      if (!value) {
        return false;
      }
      try {
        const parsed = new URL(value);
        return /\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(parsed.pathname);
      } catch {
        return /^\/app\/(?:[0-9a-f]{8,}|\d{13,})$/i.test(value);
      }
    };
    const explicitProgramUrl =
      normalizeString(args?.canvasProgramUrl) ?? normalizeString(args?.programUrl);
    if (isConcreteProgramUrl(explicitProgramUrl)) {
      return explicitProgramUrl;
    }
    const appPath = normalizeString(args?.appPath);
    if (appPath && /^\/app\//i.test(appPath)) {
      return `${baseUrl.replace(/\/+$/, "")}${appPath}`;
    }
    const conversationId = normalizeString(args?.conversationId);
    if (conversationId && /^c_[0-9a-f]{8,}$/i.test(conversationId)) {
      return `${baseUrl.replace(/\/+$/, "")}/app/${conversationId.replace(/^c_/, "")}`;
    }
    const pageUrl = normalizeString(args?.pageUrl);
    if (isConcreteProgramUrl(pageUrl)) {
      return pageUrl;
    }
    return null;
  }

  async function ensureProgramPage(entry, baseUrl, programPageUrl, timeoutMs) {
    const targetUrl = normalizeString(programPageUrl);
    if (!targetUrl) {
      return false;
    }
    if (entry.page.isClosed()) {
      throw Object.assign(new Error("Gemini Canvas browser page was unexpectedly closed."), {
        status: 500,
        code: "gemini_canvas_page_closed",
      });
    }

    let shouldNavigate = true;
    try {
      const current = new URL(entry.page.url() || targetUrl);
      const target = new URL(targetUrl);
      shouldNavigate =
        current.origin !== target.origin ||
        current.pathname.replace(/\/+$/, "") !== target.pathname.replace(/\/+$/, "");
    } catch {
      shouldNavigate = entry.page.url() !== targetUrl;
    }

    if (shouldNavigate) {
      log("navigating to concrete canvas program page", targetUrl);
      const navigationTimeoutMs = Math.min(timeoutMs, 25_000);
      try {
        await entry.page.goto(targetUrl, {
          waitUntil: "domcontentloaded",
          timeout: navigationTimeoutMs,
        });
      } catch (error) {
        const message = error instanceof Error ? error.message : String(error);
        const isNavigationTimeout =
          error?.name === "TimeoutError" ||
          /page\.goto: Timeout|Navigation timeout|Timeout .* exceeded/i.test(message);
        if (!isNavigationTimeout) {
          throw error;
        }
        log(
          "concrete canvas program navigation timed out before domcontentloaded; continuing with current page state",
          JSON.stringify({
            targetUrl,
            timeoutMs: navigationTimeoutMs,
            message,
          }),
        );
        await entry.page.waitForLoadState("commit", { timeout: 1_500 }).catch(() => undefined);
      }
    }

    await entry.page.waitForTimeout(1800);
    const finalUrl = entry.page.url();
    const loweredUrl = finalUrl.toLowerCase();
    if (
      loweredUrl.includes("signin") ||
      loweredUrl.includes("servicelogin") ||
      loweredUrl.includes("accounts.google.com")
    ) {
      throw Object.assign(
        new Error(`Gemini Canvas program navigation redirected to ${finalUrl}.`),
        {
          status: 401,
          code: "gemini_canvas_program_auth_redirect",
        },
      );
    }
    return true;
  }

  async function ensureSharePage(entry, baseUrl, shareId, timeoutMs) {
    if (entry.page.isClosed()) {
      throw Object.assign(new Error("Gemini Canvas browser page was unexpectedly closed."), {
        status: 500,
        code: "gemini_canvas_page_closed",
      });
    }

    const targetShareId = normalizeString(shareId);
    if (!targetShareId) {
      throw Object.assign(new Error("Gemini Canvas shareId is required for connected fetch mode."), {
        status: 400,
        code: "gemini_canvas_missing_share_id",
      });
    }

    const configuredAuthUser = (() => {
      try {
        const parsed = new URL(baseUrl);
        return (
          parsed.pathname.match(/^\/u\/(\d+)(?:\/|$)/i)?.[1] ??
          parsed.searchParams.get("authuser")
        );
      } catch {
        return null;
      }
    })();
    const unscopedShareUrl = (() => {
      try {
        return `${new URL(baseUrl).origin}/share/${targetShareId}`;
      } catch {
        return `${baseUrl.replace(/\/+$/, "")}/share/${targetShareId}`;
      }
    })();
    const shareUrl =
      scopeGeminiUrlToAuthUser(unscopedShareUrl, configuredAuthUser) ?? unscopedShareUrl;
    let shouldNavigate = true;
    try {
      const current = new URL(entry.page.url() || shareUrl);
      const target = new URL(shareUrl);
      shouldNavigate =
        current.origin !== target.origin ||
        current.pathname.replace(/\/+$/, "") !== target.pathname.replace(/\/+$/, "");
    } catch {
      shouldNavigate = !entry.page.url().startsWith(shareUrl);
    }

    if (shouldNavigate) {
      log("navigating to share page", shareUrl);
      for (let attempt = 0; attempt < 2; attempt += 1) {
        try {
          await entry.page.goto(shareUrl, {
            waitUntil: "domcontentloaded",
            timeout: timeoutMs,
          });
          break;
        } catch (error) {
          const message = error instanceof Error ? error.message : String(error);
          const retryableNavigationFailure =
            attempt === 0 &&
            /net::ERR_(?:CONNECTION_CLOSED|CONNECTION_RESET|NETWORK_CHANGED|HTTP2_PROTOCOL_ERROR)/i.test(
              message,
            );
          if (!retryableNavigationFailure) {
            throw error;
          }
          log(
            "share page navigation failed transiently; retrying once",
            JSON.stringify({ shareUrl, message }),
          );
          await entry.page.waitForTimeout(1200);
        }
      }
    }

    await waitForShareSurface(entry.page, Math.min(timeoutMs, 20_000));
    await entry.page.waitForTimeout(1500);
    const finalUrl = entry.page.url();
    const loweredUrl = finalUrl.toLowerCase();
    if (
      loweredUrl.includes("signin") ||
      loweredUrl.includes("servicelogin") ||
      loweredUrl.includes("accounts.google.com")
    ) {
      throw Object.assign(new Error(`Gemini Canvas share navigation redirected to ${finalUrl}.`), {
        status: 401,
        code: "gemini_canvas_auth_redirect",
      });
    }

    const bodyText = await entry.page
      .evaluate(() => document.body?.innerText ?? "")
      .catch(() => "");
    const shareSurfaceReady = /试用 Gemini Canvas|Try Gemini Canvas|在新窗口中打开|Open in new window|报告不安全的内容/i.test(
      bodyText,
    );
    if (/sign in|登录|登入|继续登录/i.test(bodyText) && !shareSurfaceReady) {
      throw Object.assign(
        new Error("Gemini Canvas connected page appears to require a fresh sign-in."),
        {
          status: 401,
          code: "gemini_canvas_auth_required",
        },
      );
    }
    log("share page ready", finalUrl);
  }

  async function waitForShareSurface(page, deadlineMs) {
    const deadline = Date.now() + deadlineMs;
    while (Date.now() < deadline) {
      const bodyText = await page.evaluate(() => (document.body?.innerText ?? "").slice(0, 4000));
      if (
        /继续|Continue|试用 Gemini Canvas|Try Gemini Canvas|在新窗口中打开|Open in new window|不要公开个人信息|Keep your personal info private/i.test(
          bodyText,
        )
      ) {
        return true;
      }
      await page.waitForTimeout(1200);
    }
    return false;
  }

  async function tryFollowShareEntryPoint(page) {
    const candidateFactories = [
      {
        kind: "continue",
        create: () =>
          page.locator("a,button,[role=\"button\"]").filter({ hasText: /继续|Continue/i }).first(),
      },
      {
        kind: "copy_canvas",
        create: () => page.locator('button[data-test-id="copy-canvas-button"]').first(),
      },
      {
        kind: "try_canvas",
        create: () =>
          page.locator("a,button,[role=\"button\"]").filter({ hasText: /试用 Gemini Canvas|Try Gemini Canvas/i }).first(),
      },
      {
        kind: "create_image",
        create: () =>
          page.locator("a,button,[role=\"button\"]").filter({ hasText: /制作图片|Create image|Create images|Make image/i }).first(),
      },
      {
        kind: "open_new_window",
        create: () =>
          page.locator("a,button,[role=\"button\"]").filter({ hasText: /在新窗口中打开|Open in new window/i }).first(),
      },
    ];
    const attemptedKinds = new Set();
    const appSurfaceMaterialized = async (targetPage) => {
      const currentUrl = targetPage.url();
      if (/\/app(?:\/|$)/i.test(currentUrl)) {
        return true;
      }
      const [promptVisible, bodyText] = await Promise.all([
        hasPromptTextbox(targetPage).catch(() => false),
        targetPage.evaluate(() => document.body?.innerText ?? "").catch(() => ""),
      ]);
      if (promptVisible && bodyTextIndicatesGeminiAppSurface(bodyText)) {
        return true;
      }
      return (
        !/\/share\//i.test(String(currentUrl || "")) &&
        bodyTextIndicatesGeminiAppSurface(bodyText)
      );
    };
    const waitForAppSurfaceMaterialization = async (targetPage, timeoutMs = 10_000) => {
      const deadline = Date.now() + Math.max(timeoutMs, 1_000);
      while (Date.now() < deadline) {
        if (await appSurfaceMaterialized(targetPage)) {
          return true;
        }
        await targetPage.waitForLoadState("domcontentloaded", { timeout: 1_500 }).catch(() => undefined);
        await targetPage.waitForTimeout(1_000);
      }
      return await appSurfaceMaterialized(targetPage);
    };

    for (let pass = 0; pass < candidateFactories.length; pass += 1) {
      let clicked = false;
      for (const factory of candidateFactories) {
        if (attemptedKinds.has(factory.kind)) {
          continue;
        }
        const candidate = factory.create();
        try {
          if ((await candidate.count()) === 0) {
            continue;
          }
          await candidate.waitFor({ state: "visible", timeout: 5000 });
          const candidateSummary =
            typeof candidate.evaluate === "function"
              ? await candidate
                  .evaluate((node) => ({
                    text: node.textContent || "",
                    ariaLabel: node.getAttribute?.("aria-label") || null,
                    title: node.getAttribute?.("title") || null,
                  }))
                  .catch(() => null)
              : null;
          log(
            "share entry point candidate click",
            JSON.stringify({
              kind: factory.kind,
              pageUrl: page.url(),
              candidate: candidateSummary,
            }),
          );
          const popupPromise = page.waitForEvent("popup", { timeout: 6000 }).catch(() => null);
          await candidate.click({ timeout: 12000, force: true });
          attemptedKinds.add(factory.kind);
          clicked = true;
          const popup = await popupPromise;
          if (popup) {
            await popup.waitForLoadState("domcontentloaded", { timeout: 20000 }).catch(() => undefined);
            const popupMaterialized = await waitForAppSurfaceMaterialization(popup, 10_000).catch(() => false);
            log(
              "share entry point popup result",
              JSON.stringify({
                kind: factory.kind,
                popupUrl: popup.url(),
                materialized: popupMaterialized,
              }),
            );
            return { kind: "popup", page: popup };
          }
          const samePageMaterialized = await waitForAppSurfaceMaterialization(page, 10_000);
          log(
            "share entry point same-page result",
            JSON.stringify({
              kind: factory.kind,
              pageUrl: page.url(),
              materialized: samePageMaterialized,
            }),
          );
          if (samePageMaterialized) {
            return { kind: "same_page", page };
          }
          break;
        } catch {
          // try next candidate
        }
      }
      if (!clicked) {
        break;
      }
    }

    const [bodyText, buttons] = await Promise.all([
      page.evaluate(() => document.body?.innerText ?? "").catch(() => ""),
      collectButtonSnapshot(page).catch(() => []),
    ]);
    log(
      "share entry point follow failed",
      JSON.stringify({
        pageUrl: page.url(),
        attemptedKinds: [...attemptedKinds],
        bodyPreview: String(bodyText || "").slice(0, 1200),
        buttons: Array.isArray(buttons) ? buttons.slice(0, 80) : [],
      }),
    );
    return { kind: "none", page };
  }

  return { resolveProgramPageUrl, ensureProgramPage, ensureSharePage, waitForShareSurface, tryFollowShareEntryPoint };
}
