import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { parseBoolean } from "./gemini-canvas-browser-pool-executable.mjs";
import { submitPrompt } from "./gemini-canvas-browser-pool-composer.mjs";
import { extractAudioBytes } from "./gemini-canvas-browser-pool-payload.mjs";

export function createTtsOperationOwner({
  DEFAULT_TIMEOUT_MS, operationConfig, isFixtureCanvasBaseUrl, resolveProgramPageUrl,
  resetConversation, startNetworkCapture, collectPageSnapshot, collectButtonSnapshot,
  selectAudioAsset, buildProgramHandleState, log,
}) {
  function buildTtsDiagnostics({
    captureState,
    lastSnapshot,
    lastButtonSnapshot,
    clicked = false,
    pollCount = null,
    asset = null,
    audio = null,
  }) {
    return {
      clicked,
      pollCount,
      pageUrl: lastSnapshot?.pageState?.url ?? null,
      title: lastSnapshot?.pageState?.title ?? null,
      bodyText: lastSnapshot?.pageState?.bodyText ?? null,
      buttons: Array.isArray(lastButtonSnapshot) ? lastButtonSnapshot.slice(0, 120) : [],
      mediaNodes: Array.isArray(lastSnapshot?.mediaNodes) ? lastSnapshot.mediaNodes.slice(0, 80) : [],
      anchorNodes: Array.isArray(lastSnapshot?.anchorNodes) ? lastSnapshot.anchorNodes.slice(0, 40) : [],
      streamGenerateRequestAt: captureState?.streamGenerateRequestAt ?? null,
      streamGenerateResponseAt: captureState?.streamGenerateResponseAt ?? null,
      audioUrls: Array.isArray(captureState?.audioUrls) ? captureState.audioUrls.slice(-12) : [],
      events: Array.isArray(captureState?.events) ? captureState.events.slice(-120) : [],
      selectedAsset: asset
        ? {
          url: asset.url ?? null,
          mimeType: asset.mimeType ?? null,
          durationSeconds: asset.durationSeconds ?? null,
        }
        : null,
      audioResult: audio
        ? {
          mimeType: audio.mimeType ?? null,
          bodyBase64Length: typeof audio.bodyBase64 === "string" ? audio.bodyBase64.length : 0,
        }
        : null,
    };
  }

  function listenControlCandidates(page) {
    return [
      page.getByRole("button", { name: /听回答|收听回答|朗读|播放语音|播放回答|Listen|Play response|Read aloud|Listen to response/i }).last(),
      page.locator(
        'button[aria-label*="Listen"], button[aria-label*="听"], button[aria-label*="朗读"], button[aria-label*="播放"], button[title*="Listen"], button[title*="听"], button[title*="朗读"], button[title*="播放"]',
      ).last(),
      page.locator(
        'button:has-text("听"), button:has-text("朗读"), button:has-text("播放"), button:has-text("Listen"), button:has-text("Read aloud"), button:has-text("Play")',
      ).last(),
    ];
  }

  async function clickListenControlWithFallback(page, candidate, timeoutMs) {
    try {
      await candidate.waitFor({ state: "visible", timeout: 1200 });
    } catch {
      return false;
    }

    try {
      await candidate.scrollIntoViewIfNeeded().catch(() => undefined);
      await candidate.click({ timeout: Math.min(timeoutMs, 10_000) });
      return true;
    } catch {
      // Fall back to a DOM-dispatched click when Gemini's presented-response layer
      // intercepts pointer events above the button.
    }

    try {
      const elementHandle = await candidate.elementHandle();
      if (!elementHandle) {
        return false;
      }
      await elementHandle.evaluate((node) => {
        node.scrollIntoView({ block: "center", inline: "nearest" });
        for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
          node.dispatchEvent(
            new MouseEvent(type, {
              bubbles: true,
              cancelable: true,
              composed: true,
              view: window,
            }),
          );
        }
        node.click();
      });
      return true;
    } catch {
      return false;
    }
  }

  async function runTtsOperation(entry, args) {
    const prompt = normalizeString(args.prompt);
    if (!prompt) {
      throw Object.assign(
        new Error("Gemini Canvas TTS invocation requires a non-empty prompt."),
        {
          status: 400,
          code: "gemini_canvas_invalid_tts_prompt",
        },
      );
    }

    const timeoutMs = Math.max(
      Number(args.timeoutMs || DEFAULT_TIMEOUT_MS),
      operationConfig("tts").resultTimeoutMs,
    );
    const fixtureBaseUrl = normalizeString(args.baseUrl);
    if (args.requireAppPage === false && isFixtureCanvasBaseUrl(fixtureBaseUrl)) {
      const ttsBaseUrl = fixtureBaseUrl.replace(/\/+$/, "");
      const fixtureAudio = Buffer.from(
        (`OggSgemini canvas fixture tts::${prompt}`).padEnd(128, "~"),
        "utf8",
      );
      return {
        operation: "tts",
        pageUrl: `${ttsBaseUrl}/gemini-canvas-fixture`,
        bodyText: "gemini canvas fixture tts ok",
        bodyBase64: fixtureAudio.toString("base64"),
        mimeType: "audio/ogg",
        media: [],
      };
    }
    const includeDiagnostics = parseBoolean(args.includeDiagnostics, false);
    const page = entry.page;
    const preferredProgramPageUrl = resolveProgramPageUrl(
      normalizeString(args.baseUrl) ?? "https://gemini.google.com",
      args,
    );
    await resetConversation(
      page,
      normalizeString(args.baseUrl) ?? "https://gemini.google.com",
      timeoutMs,
      preferredProgramPageUrl,
      {
        skipInitialNavigationWhenAppSurfaceReady: entry.attachedCdp === true,
      },
    );
    const capture = startNetworkCapture(page, "tts");

    try {
      await capture.ready;
      await submitPrompt(page, prompt, timeoutMs);
      log("tts prompt submitted", prompt.slice(0, 120));

      const deadline = Date.now() + timeoutMs;
      let lastSnapshot = null;
      let clicked = false;
      let lastButtonSnapshot = [];
      let pollCount = 0;
      while (Date.now() < deadline && !clicked) {
        pollCount += 1;
        lastSnapshot = await collectPageSnapshot(page);
        lastButtonSnapshot = await collectButtonSnapshot(page).catch(() => []);
        if (pollCount <= 3 || pollCount % 10 === 0) {
          log(
            "tts awaiting listen control",
            JSON.stringify({
              pollCount,
              buttonCount: lastButtonSnapshot.length,
              bodyPreview: (lastSnapshot?.pageState?.bodyText ?? "").slice(0, 240),
            }),
          );
        }
        for (const candidate of listenControlCandidates(page)) {
          try {
            if (await clickListenControlWithFallback(page, candidate, timeoutMs)) {
              clicked = true;
              log("tts listen control clicked", `poll=${pollCount}`);
              break;
            }
          } catch {
            // keep trying other selectors / later polling iterations
          }
        }
        if (!clicked) {
          try {
            const domClicked = await page.evaluate(() => {
              const selectors = [
                'button[aria-label*="听回答"]',
                'button[aria-label*="收听回答"]',
                'button[aria-label*="朗读"]',
                'button[aria-label*="播放语音"]',
                'button[aria-label*="播放回答"]',
                'button[aria-label*="Listen"]',
                'button[aria-label*="Play response"]',
                'button[aria-label*="Read aloud"]',
                'button[aria-label*="Listen to response"]',
                'button[title*="听回答"]',
                'button[title*="朗读"]',
                'button[title*="播放"]',
                'button[title*="Listen"]',
                'button[title*="Read aloud"]',
                'button[title*="Play"]',
              ];
              for (const selector of selectors) {
                const matches = Array.from(document.querySelectorAll(selector));
                const target = matches.at(-1);
                if (!target) {
                  continue;
                }
                target.scrollIntoView({ block: "center", inline: "nearest" });
                for (const type of ["pointerdown", "mousedown", "mouseup", "click"]) {
                  target.dispatchEvent(
                    new MouseEvent(type, {
                      bubbles: true,
                      cancelable: true,
                      composed: true,
                      view: window,
                    }),
                  );
                }
                target.click();
                return true;
              }
              return false;
            });
            if (domClicked) {
              clicked = true;
              break;
            }
          } catch {
            // keep trying later polling iterations
          }
        }
        if (!clicked) {
          await page.waitForTimeout(1500);
        }
      }
      if (!clicked) {
        throw Object.assign(
          new Error(
            "Gemini Canvas did not expose a visible TTS/Listen control for the current response.",
          ),
          {
            status: 500,
            code: "gemini_canvas_tts_control_missing",
            bodyText: JSON.stringify(
              buildTtsDiagnostics({
                captureState: capture.state,
                lastSnapshot,
                lastButtonSnapshot,
                clicked,
                pollCount,
              }),
              null,
              2,
            ),
          },
        );
      }

      while (Date.now() < deadline) {
        lastSnapshot = await collectPageSnapshot(page);
        const asset = selectAudioAsset(lastSnapshot, capture.state);
        if (!asset && (pollCount <= 3 || pollCount % 10 === 0)) {
          log(
            "tts awaiting audio asset",
            JSON.stringify({
              pollCount,
              audioUrls: capture.state.audioUrls,
              streamGenerateRequestAt: capture.state.streamGenerateRequestAt,
              streamGenerateResponseAt: capture.state.streamGenerateResponseAt,
            }),
          );
        }
        if (asset) {
          let audio = null;
          try {
            audio = await extractAudioBytes(page, asset);
          } catch (error) {
            if (error instanceof Error && error.code === "gemini_canvas_tts_non_audio_asset") {
              log(
                "tts skipping non-audio asset",
                JSON.stringify({
                  url: asset.url,
                  mimeType: error.mimeType ?? null,
                }),
              );
              await page.waitForTimeout(1200);
              continue;
            }
            throw error;
          }
          const result = {
            operation: "tts",
            pageUrl: lastSnapshot.pageState?.url ?? page.url(),
            bodyText: lastSnapshot.pageState?.bodyText ?? null,
            mimeType: audio.mimeType,
            bodyBase64: audio.bodyBase64,
            ...buildProgramHandleState(
              normalizeString(args.baseUrl) ?? "https://gemini.google.com",
              args,
              lastSnapshot.pageState?.url ?? page.url(),
              capture.state,
            ),
            media: [
              {
                kind: "audio",
                url: asset.url,
                mimeType: audio.mimeType,
                durationSeconds: asset.durationSeconds,
              },
            ],
          };
          if (includeDiagnostics) {
            result.diagnostics = buildTtsDiagnostics({
              captureState: capture.state,
              lastSnapshot,
              lastButtonSnapshot,
              clicked,
              pollCount,
              asset,
              audio,
            });
          }
          return result;
        }
        await page.waitForTimeout(2000);
      }

      throw Object.assign(new Error("Timed out waiting for Gemini Canvas TTS audio output."), {
        status: 504,
        code: "gemini_canvas_tts_timeout",
        bodyText: JSON.stringify(
          buildTtsDiagnostics({
            captureState: capture.state,
            lastSnapshot,
            lastButtonSnapshot,
            clicked,
            pollCount,
          }),
          null,
          2,
        ),
      });
    } finally {
      await capture.stop();
    }
  }

  return { buildTtsDiagnostics, listenControlCandidates, clickListenControlWithFallback, runTtsOperation };
}
