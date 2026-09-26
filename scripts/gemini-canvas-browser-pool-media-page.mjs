import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { findAttachedGeminiAppPage } from "./gemini-canvas-browser-pool-app.mjs";

export function createMediaPageOwner({ log }) {
  async function closePageSafely(page, contextLabel, timeoutMs = 3_000) {
    if (!page) {
      return;
    }
    let settled = false;
    const closing = page.close().catch(() => undefined).finally(() => {
      settled = true;
    });
    let timeoutHandle;
    try {
      await Promise.race([
        closing,
        new Promise((resolve) => {
          timeoutHandle = setTimeout(resolve, timeoutMs);
        }),
      ]);
    } finally {
      clearTimeout(timeoutHandle);
    }
    if (!settled) {
      log("page close timed out", JSON.stringify({ contextLabel, timeoutMs }));
    }
  }

  async function acquireMediaOperationPage(entry, baseUrl) {
    const attachedPage = await findAttachedGeminiAppPage(entry, baseUrl);
    if (attachedPage) {
      await attachedPage.bringToFront().catch(() => undefined);
      log(
        "reusing attached Gemini app page for media operation",
        JSON.stringify({
          pageUrl: normalizeString(attachedPage.url()) ?? "",
          runtimeStatePath: entry.runtimeStatePath,
        }),
      );
      return {
        page: attachedPage,
        closeWhenDone: false,
      };
    }

    const page = await entry.context.newPage();
    await page.bringToFront().catch(() => undefined);
    return {
      page,
      closeWhenDone: true,
    };
  }

  return { closePageSafely, acquireMediaOperationPage };
}
