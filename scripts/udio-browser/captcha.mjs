import process from "node:process";
import { normalizeString } from "./request.mjs";
import { safePageUrl, openChallengeSurface } from "./browser.mjs";
import { debugLog } from "./diagnostics.mjs";
import { trimBody } from "./responses.mjs";

const DEFAULT_HCAPTCHA_SITEKEY = "2945592b-1928-43a9-8473-7e7fed3d752e";
const DEFAULT_HCAPTCHA_API_SRC =
  "https://js.hcaptcha.com/1/api.js?render=explicit&sentry=false&uj=false";

export async function refreshCaptchaToken(page, options) {
  const {
    baseUrl,
    referer,
    navigationTimeoutMs,
    manualChallengeWaitMs,
    debugLogPath,
    allowNavigation,
  } = options;
  const sitekey =
    normalizeString(process.env.UDIO_HCAPTCHA_SITEKEY) ?? DEFAULT_HCAPTCHA_SITEKEY;
  const apiSrc =
    normalizeString(process.env.UDIO_HCAPTCHA_API_SRC) ?? DEFAULT_HCAPTCHA_API_SRC;
  await debugLog(debugLogPath, "captcha_refresh_start", {
    url: safePageUrl(page),
    manualChallengeWaitMs,
  });
  try {
    const token = await obtainHCaptchaToken(page, sitekey, apiSrc, manualChallengeWaitMs);
    await debugLog(debugLogPath, "captcha_refresh_ready", {
      tokenLength: token?.length ?? 0,
      mode: "current_page",
    });
    return token;
  } catch (error) {
    await debugLog(debugLogPath, "captcha_refresh_retry_surface", {
      message: error?.message ?? String(error),
    });
    if (!allowNavigation) {
      throw error;
    }
    await openChallengeSurface(page, baseUrl, referer, navigationTimeoutMs, {
      allowNavigation,
    });
    const token = await obtainHCaptchaToken(page, sitekey, apiSrc, manualChallengeWaitMs);
    await debugLog(debugLogPath, "captcha_refresh_ready", {
      tokenLength: token?.length ?? 0,
      mode: "challenge_surface",
    });
    return token;
  }
}

export async function obtainHCaptchaToken(page, sitekey, apiSrc, manualChallengeWaitMs = 0) {
  try {
    return await page.evaluate(async ({ sitekey, apiSrc, manualChallengeWaitMs }) => {
    const scriptId = "__udio-hcaptcha-script";
    const containerId = "__udio-hcaptcha-container";
    const manualContainerId = "__udio-hcaptcha-manual-container";
    const manualWrapperId = "__udio-hcaptcha-manual-wrapper";
    const executeTimeoutMs = 15_000;

    const waitForHCaptcha = async () => {
      if (window.hcaptcha) {
        return window.hcaptcha;
      }

      await new Promise((resolve, reject) => {
        const existing = document.getElementById(scriptId);
        if (existing) {
          const startedAt = Date.now();
          const poll = () => {
            if (window.hcaptcha) {
              resolve();
              return;
            }
            if (Date.now() - startedAt > 20_000) {
              reject(new Error("Timed out waiting for hCaptcha to load."));
              return;
            }
            setTimeout(poll, 100);
          };
          poll();
          return;
        }

        const script = document.createElement("script");
        script.id = scriptId;
        script.async = true;
        script.src = apiSrc;
        script.onload = () => resolve();
        script.onerror = () => reject(new Error("Failed to load hCaptcha API."));
        document.head.appendChild(script);
      });

      if (!window.hcaptcha) {
        throw new Error("hCaptcha API did not become available.");
      }
      return window.hcaptcha;
    };

    const ensureContainer = (id, styleText, textContent = "") => {
      let node = document.getElementById(id);
      if (!node) {
        node = document.createElement("div");
        node.id = id;
        if (textContent) {
          node.textContent = textContent;
        }
        document.body.appendChild(node);
      }
      node.style.cssText = styleText;
      return node;
    };

    const withTimeout = async (promise, timeoutMs, label) => {
      let timer = null;
      try {
        return await Promise.race([
          promise,
          new Promise((_, reject) => {
            timer = setTimeout(() => {
              reject(new Error(`${label} timed out after ${timeoutMs}ms.`));
            }, timeoutMs);
          }),
        ]);
      } finally {
        if (timer) {
          clearTimeout(timer);
        }
      }
    };

    const readVisibleChallengeFrame = () => {
      const frames = Array.from(document.querySelectorAll('iframe[src*="hcaptcha"]'));
      for (const frame of frames) {
        if (!(frame instanceof HTMLIFrameElement)) {
          continue;
        }
        const rect = frame.getBoundingClientRect();
        const style = window.getComputedStyle(frame);
        if (
          rect.width >= 120 &&
          rect.height >= 120 &&
          style.display !== "none" &&
          style.visibility !== "hidden" &&
          style.opacity !== "0"
        ) {
          return {
            title: frame.getAttribute("title") || null,
            src: frame.getAttribute("src") || null,
            width: Math.round(rect.width),
            height: Math.round(rect.height),
          };
        }
      }
      return null;
    };

    const throwVisibleChallengeRequired = (frame) => {
      const title = typeof frame?.title === "string" && frame.title.trim() ? frame.title.trim() : "visible hCaptcha challenge";
      throw new Error(`challenge required: ${title}`);
    };

    const waitForSolvedToken = async (hcaptcha, widgetId, timeoutMs) => {
      if (!timeoutMs || timeoutMs <= 0) {
        return null;
      }
      const deadline = Date.now() + timeoutMs;
      while (Date.now() < deadline) {
        const token = readWidgetToken(hcaptcha, widgetId);
        if (token) {
          return token;
        }
        await new Promise((resolve) => setTimeout(resolve, 250));
      }
      return null;
    };

    const validWidgetId = (widgetId) =>
      typeof widgetId === "number" ||
      (typeof widgetId === "string" && Boolean(widgetId.trim()));

    const readWidgetToken = (hcaptcha, widgetId) => {
      const callbackToken = window.__udioManualHcaptchaResolvedToken;
      if (typeof callbackToken === "string" && callbackToken.trim()) {
        return callbackToken.trim();
      }
      if (!validWidgetId(widgetId) || typeof hcaptcha?.getResponse !== "function") {
        return null;
      }
      try {
        const token = hcaptcha.getResponse(widgetId);
        return typeof token === "string" && token.trim() ? token.trim() : null;
      } catch {
        return null;
      }
    };

    const extractExecuteToken = (execution, hcaptcha, widgetId) => {
      const direct =
        (typeof execution === "object" && typeof execution?.response === "string"
          ? execution.response
          : typeof execution === "string"
            ? execution
            : null) || readWidgetToken(hcaptcha, widgetId);
      return typeof direct === "string" && direct.trim() ? direct.trim() : null;
    };

    const hcaptcha = await waitForHCaptcha();
    const removeWidget = (widgetId, container) => {
      if (validWidgetId(widgetId) && typeof hcaptcha.remove === "function") {
        try {
          hcaptcha.remove(widgetId);
        } catch {
          // Ignore stale widget ids.
        }
      }
      container?.replaceChildren();
    };

    if (manualChallengeWaitMs > 0) {
      ensureContainer(
        manualWrapperId,
        [
          "position:fixed",
          "right:24px",
          "bottom:24px",
          "z-index:2147483647",
          "max-width:360px",
          "padding:12px 14px",
          "border-radius:12px",
          "background:rgba(16,18,24,0.94)",
          "color:#fff",
          "font:500 14px/1.45 Inter, Arial, sans-serif",
          "box-shadow:0 20px 60px rgba(0,0,0,0.45)",
        ].join(";"),
        "Complete the captcha in this window to continue the Udio generation request.",
      );

      const manualContainer = ensureContainer(
        manualContainerId,
        [
          "position:fixed",
          "right:24px",
          "bottom:96px",
          "z-index:2147483647",
          "min-width:304px",
          "min-height:78px",
          "padding:4px",
          "border-radius:12px",
          "background:rgba(255,255,255,0.98)",
          "box-shadow:0 20px 60px rgba(0,0,0,0.35)",
        ].join(";"),
      );
      removeWidget(window.__udioManualHcaptchaWidgetId, manualContainer);
      window.__udioManualHcaptchaResolvedToken = null;
      const manualWidgetId = hcaptcha.render(manualContainer, {
        sitekey,
        size: "normal",
        theme: "light",
        callback: (token) => {
          window.__udioManualHcaptchaResolvedToken =
            typeof token === "string" && token.trim() ? token.trim() : null;
        },
        "expired-callback": () => {
          window.__udioManualHcaptchaResolvedToken = null;
        },
        "error-callback": () => {
          window.__udioManualHcaptchaResolvedToken = null;
        },
      });
      window.__udioManualHcaptchaWidgetId = manualWidgetId;

      const manualToken = await waitForSolvedToken(hcaptcha, manualWidgetId, manualChallengeWaitMs);
      if (manualToken) {
        window.__udioManualHcaptchaWidgetId = null;
        window.__udioManualHcaptchaResolvedToken = null;
        removeWidget(manualWidgetId, manualContainer);
        return manualToken;
      }
      throwVisibleChallengeRequired(
        readVisibleChallengeFrame() ?? { title: "hCaptcha challenge was not completed" },
      );
    }

    const container = ensureContainer(
      containerId,
      "position:fixed;left:-9999px;top:-9999px;width:1px;height:1px;opacity:0;pointer-events:none;",
    );

    removeWidget(window.__udioHcaptchaWidgetId, container);
    const widgetId = hcaptcha.render(container, {
      sitekey,
      size: "invisible",
      theme: "dark",
    });
    window.__udioHcaptchaWidgetId = widgetId;

    const execution = await withTimeout(
      hcaptcha.execute(widgetId, { async: true }),
      executeTimeoutMs,
      "hCaptcha helper execute",
    );
    const token =
      (typeof execution === "object" && typeof execution?.response === "string"
        ? execution.response
        : typeof execution === "string"
          ? execution
          : null) || readWidgetToken(hcaptcha, widgetId);

    if (!token || typeof token !== "string") {
      const visibleChallengeAfterHelperExecute = readVisibleChallengeFrame();
      if (visibleChallengeAfterHelperExecute) {
        throwVisibleChallengeRequired(visibleChallengeAfterHelperExecute);
      }
      throw new Error("hCaptcha execute completed without returning a token.");
    }

    window.__udioHcaptchaWidgetId = null;
    removeWidget(widgetId, container);
    return token;
    }, { sitekey, apiSrc, manualChallengeWaitMs });
  } catch (error) {
    const message = typeof error?.message === "string" ? error.message : String(error);
    const lower = message.toLowerCase();
    if (
      lower.includes("challenge-expired") ||
      lower.includes("challenge-closed") ||
      lower.includes("challenge required") ||
      (lower.includes("hcaptcha") && lower.includes("timed out"))
    ) {
      throw Object.assign(
        new Error(
          "Udio requires a browser security check or captcha challenge before generation can continue.",
        ),
        {
          status: 429,
          code: "udio_browser_challenge_required",
          body: trimBody(message),
        },
      );
    }
    throw error;
  }
}
