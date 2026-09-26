import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { submitPrompt } from "./gemini-canvas-browser-pool-composer.mjs";
import { collectButtonSnapshot, collectPageSnapshot } from "./gemini-canvas-browser-pool-page-snapshot.mjs";

export function shouldExitCanvasProgramSurfaceForMediaMode(operation, snapshot = {}) {
  if (normalizeString(operation) !== "image") {
    return false;
  }
  const bodyText = String(snapshot?.bodyText || "");
  const buttons = Array.isArray(snapshot?.buttons) ? snapshot.buttons : [];
  const flattenedButtons = buttons
    .flatMap((button) => [button?.text, button?.ariaLabel, button?.title])
    .filter(Boolean)
    .join("\n");
  return (
    /Browser API Proxy Client/i.test(bodyText) ||
    /Try again without Canvas|不使用应用，再试一次/i.test(flattenedButtons)
  );
}

export function createMediaExecutionOwner({
  DEFAULT_TIMEOUT_MS,
  operationConfig,
  isFixtureCanvasBaseUrl,
  acquireMediaOperationPage,
  resolveProgramPageUrl,
  resetConversation,
  startNetworkCapture,
  clickOperationMode,
  log,
  pollMediaOperation,
  closePageSafely,
}) {
  async function tryExitCanvasProgramSurfaceForMediaMode(page, operation, timeoutMs) {
    if (normalizeString(operation) !== "image") {
      return false;
    }
    const bodyText = await page.evaluate(() => document.body?.innerText ?? "").catch(() => "");
    const buttons = await collectButtonSnapshot(page).catch(() => []);
    if (!shouldExitCanvasProgramSurfaceForMediaMode(operation, { bodyText, buttons })) {
      return false;
    }

    const candidates = [
      page.getByRole("button", { name: /Try again without Canvas|不使用应用，再试一次/i }).first(),
      page.getByRole("link", { name: /Try again without Canvas|不使用应用，再试一次/i }).first(),
      page.locator('button,[role="button"],a').filter({ hasText: /Try again without Canvas|不使用应用，再试一次/i }).first(),
    ];
    for (const candidate of candidates) {
      try {
        await candidate.waitFor({ state: "visible", timeout: Math.min(timeoutMs, 2_000) });
        await candidate.click({ timeout: Math.min(timeoutMs, 8_000), force: true });
        await page.waitForTimeout(1_500);
        return true;
      } catch {
        // try next candidate
      }
    }
    return false;
  }

  async function runMediaOperation(entry, args) {
    const operation = normalizeString(args.operation);
    const prompt = normalizeString(args.prompt);
    if (!operation || !prompt) {
      throw Object.assign(
        new Error("Gemini Canvas browser invocation requires operation and prompt."),
        {
          status: 400,
          code: "gemini_canvas_invalid_request",
        },
      );
    }

    const { resultTimeoutMs } = operationConfig(operation);
    const timeoutMs = Math.max(
      Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
      resultTimeoutMs,
    );
    const fixtureBaseUrl = normalizeString(args.baseUrl);
    const resumeExistingMedia = args.resumeExistingMedia === true;
    if (args.requireAppPage === false && isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
      const mediaBaseUrl = fixtureBaseUrl.replace(/\/+$/, "");
      if (operation === "image") {
        return {
          operation,
          pageUrl: `${mediaBaseUrl}/gemini-canvas-fixture`,
          bodyText: "gemini canvas fixture image ok",
          media: [
            {
              kind: "image",
              url: `${mediaBaseUrl}/fixtures/gemini-canvas/image.png`,
              mimeType: "image/png",
              alt: "Gemini Canvas fixture image",
              width: 1024,
              height: 1024,
              durationSeconds: null,
            },
          ],
        };
      }
      if (operation === "music") {
        return {
          operation,
          pageUrl: `${mediaBaseUrl}/gemini-canvas-fixture`,
          bodyText: "gemini canvas fixture music ok",
          media: [
            {
              kind: "audio",
              url: `${mediaBaseUrl}/fixtures/gemini-canvas/music.wav`,
              mimeType: "audio/wav",
              alt: "Gemini Canvas fixture music",
              width: null,
              height: null,
              durationSeconds: 3.2,
            },
          ],
        };
      }
      if (operation === "video") {
        return {
          operation,
          pageUrl: `${mediaBaseUrl}/gemini-canvas-fixture`,
          bodyText: "gemini canvas fixture video ok",
          media: [
            {
              kind: "video",
              url: `${mediaBaseUrl}/fixtures/gemini-canvas/video.mp4`,
              mimeType: "video/mp4",
              alt: "Gemini Canvas fixture video",
              width: 1280,
              height: 720,
              durationSeconds: 5.4,
            },
          ],
        };
      }
    }
    const baseUrl = fixtureBaseUrl ?? "https://gemini.google.com";
    const pageLease = await acquireMediaOperationPage(entry, baseUrl);
    const page = pageLease.page;
    let capture = null;

    try {
      const preferredProgramPageUrl = resolveProgramPageUrl(
        baseUrl,
        args,
      );
      await resetConversation(
        page,
        baseUrl,
        timeoutMs,
        preferredProgramPageUrl,
      );
      capture = startNetworkCapture(page, operation);
      await capture.ready;
      if (!resumeExistingMedia) {
        let modeSelected = await clickOperationMode(page, operation, timeoutMs);
        if (!modeSelected) {
          const exitedCanvasProgramSurface = await tryExitCanvasProgramSurfaceForMediaMode(
            page,
            operation,
            timeoutMs,
          );
          if (exitedCanvasProgramSurface || operation === "image") {
            await resetConversation(page, baseUrl, timeoutMs, null, {
              skipInitialNavigationWhenAppSurfaceReady: true,
            });
            modeSelected = await clickOperationMode(page, operation, timeoutMs);
          }
        }
        if (!modeSelected) {
          const modeUnavailableSnapshot = await collectPageSnapshot(page).catch(() => null);
          const modeUnavailableButtons = await collectButtonSnapshot(page).catch(() => []);
          log(
            "media operation mode unavailable",
            JSON.stringify({
              operation,
              pageUrl: page.url(),
              bodyPreview: String(modeUnavailableSnapshot?.pageState?.bodyText ?? "").slice(0, 1200),
              buttons: modeUnavailableButtons.slice(0, 120),
            }),
          );
          throw Object.assign(
            new Error(`Gemini Canvas ${operation} mode could not be activated.`),
            {
              status: 409,
              code: `gemini_canvas_${operation}_mode_unavailable`,
              bodyText: JSON.stringify(
                {
                  operation,
                  pageUrl: page.url(),
                  bodyText: modeUnavailableSnapshot?.pageState?.bodyText ?? null,
                  buttons: modeUnavailableButtons.slice(0, 120),
                  mediaNodes: modeUnavailableSnapshot?.mediaNodes?.slice(0, 40) ?? [],
                  anchorNodes: modeUnavailableSnapshot?.anchorNodes?.slice(0, 40) ?? [],
                  networkEvents: capture.state.events.slice(-80),
                  rpcCaptures: capture.state.rpcCaptures.slice(-40),
                },
                null,
                2,
              ),
            },
          );
        }
        log("media operation mode selected", operation);
        await submitPrompt(page, prompt, timeoutMs);
        log("media prompt submitted", `${operation}: ${String(prompt).slice(0, 180)}`);
      } else {
        log("resuming existing media result", JSON.stringify({ operation, pageUrl: page.url() }));
      }

      return await pollMediaOperation({
        operation, prompt, timeoutMs, page, capture, baseUrl, args,
      });
    } finally {
      log("runMediaOperation finally start", JSON.stringify({ operation }));
      await capture?.stop();
      if (pageLease.closeWhenDone) {
        await closePageSafely(page, `runMediaOperation:${operation}`);
      }
      log("runMediaOperation finally done", JSON.stringify({ operation }));
    }
  }

  return { runMediaOperation };
}
