import { collectPageSnapshot } from "./gemini-canvas-browser-pool-page-snapshot.mjs";
import { mergeActionContract, extractCanvasProgramActionContractFromText } from "./gemini-canvas-browser-pool-action.mjs";
import { submitPrompt } from "./gemini-canvas-browser-pool-composer.mjs";
import {
  bodyTextSuggestsVideoTemplateSelection, trySelectVideoTemplateCard, tryClickVideoCreateAction,
} from "./gemini-canvas-browser-pool-video-ui.mjs";
import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import { createMediaResultOwner } from "./gemini-canvas-browser-pool-media-result.mjs";

export function createMediaPollingOwner({
  mergeInvokeContract,
  buildCanvasProgramInvokeContract,
  selectMediaAssetsForOperation,
  log,
  shouldRetryMediaPromptSubmission,
  retryMediaPromptSubmission,
  resetConversation,
  clickOperationMode,
  detectMediaProviderGate,
  recentMediaProviderGateText,
  shouldBlockMediaProviderGate,
  clickMediaActionButton,
  bodyIndicatesMusicPendingOrBusy,
  buildProgramHandleState,
  extractImageBytes,
  extractAudioBytes,
}) {
  const { finalizeMediaOperation } = createMediaResultOwner({
    mergeInvokeContract,
    buildCanvasProgramInvokeContract,
    selectMediaAssetsForOperation,
    log,
    bodyIndicatesMusicPendingOrBusy,
    buildProgramHandleState,
    extractImageBytes,
    extractAudioBytes,
  });

  async function pollMediaOperation({
    operation,
    prompt,
    timeoutMs,
    page,
    capture,
    baseUrl,
    args,
  }) {
    const deadline = Date.now() + timeoutMs;
    let lastSnapshot = null;
    let pollCount = 0;
    let videoTemplateSelectionAttempts = 0;
    let videoTemplateSelectedAt = null;
    let videoCreateClickAttempts = 0;
    let videoCreateClickedAt = null;
    let videoTransientFailureRetryCount = 0;
    let mediaPromptSubmissionRetryCount = 0;
    let videoComposerResetCount = 0;
    let playerReadyPlayAttempted = false;
    let playerReadyDownloadAttempted = false;
    while (Date.now() < deadline) {
      pollCount += 1;
      lastSnapshot = await collectPageSnapshot(page);
      mergeActionContract(
        capture.state.actionContract,
        extractCanvasProgramActionContractFromText(lastSnapshot?.pageState?.bodyText ?? ""),
      );
      capture.state.invokeContract = mergeInvokeContract(
        capture.state.invokeContract,
        buildCanvasProgramInvokeContract(
          operation,
          capture.state.actionContract,
          capture.state.transportHints,
          {
            bodyText: lastSnapshot?.pageState?.bodyText ?? "",
            buttons: lastSnapshot?.buttons ?? [],
            anchorNodes: lastSnapshot?.anchorNodes ?? [],
            mediaNodes: lastSnapshot?.mediaNodes ?? [],
          },
          prompt,
          capture.state,
        ),
      );
      const media = selectMediaAssetsForOperation(operation, lastSnapshot, capture.state);

        let quotaGateText = null;
        if (operation === "video") {
          const quotaCandidates = [
            page.getByText(/已达到视频生成数量上限/i).first(),
            page.getByText(/方可继续生成视频/i).first(),
            page.getByText(/出了点问题\s*\(1053\)/i).first(),
          ];
          for (const candidate of quotaCandidates) {
            try {
              if (await candidate.isVisible({ timeout: 50 })) {
                quotaGateText = (await candidate.textContent()) || "Gemini Canvas video quota gate";
                break;
              }
            } catch {
              // try next candidate
            }
          }
        }

        if (pollCount <= 3 || pollCount % 10 === 0) {
          log(
            "media poll snapshot",
            JSON.stringify({
              operation,
              pollCount,
              imageUrls: capture.state.imageUrls.map((entry) => entry.url).slice(-3),
              audioUrls: capture.state.audioUrls.map((entry) => entry.url).slice(-3),
              videoUrls: capture.state.videoUrls.map((entry) => entry.url).slice(-3),
              mediaNodeKinds: (lastSnapshot?.mediaNodes || []).map((node) => node.kind).slice(0, 12),
              bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
              events: operation === "image" ? capture.state.events.slice(-4) : undefined,
            }),
          );
        }

        if (
          mediaPromptSubmissionRetryCount < 2
          && shouldRetryMediaPromptSubmission(operation, {
            bodyText: lastSnapshot?.pageState?.bodyText ?? "",
            events: capture.state.events,
            prompt,
          })
        ) {
          mediaPromptSubmissionRetryCount += 1;
          log(
            "retrying media prompt submission after template-only stall",
            JSON.stringify({
              operation,
              pollCount,
              attempt: mediaPromptSubmissionRetryCount,
              bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
            }),
          );
          await retryMediaPromptSubmission(page, prompt, timeoutMs);
          await page.waitForTimeout(1_200);
          continue;
        }

        if (
          operation === "video"
          && mediaPromptSubmissionRetryCount >= 2
          && videoComposerResetCount < 3
          && shouldRetryMediaPromptSubmission(operation, {
            bodyText: lastSnapshot?.pageState?.bodyText ?? "",
            events: capture.state.events,
            prompt,
          })
        ) {
          videoComposerResetCount += 1;
          log(
            "resetting stalled video composer",
            JSON.stringify({
              operation,
              pollCount,
              attempt: videoComposerResetCount,
            }),
          );
          await resetConversation(page, baseUrl, timeoutMs, null, {
            skipInitialNavigationWhenAppSurfaceReady: true,
          });
          const retryModeSelected = await clickOperationMode(page, operation, timeoutMs);
          if (!retryModeSelected) {
            throw Object.assign(new Error("Gemini Canvas video mode could not be reactivated."), {
              status: 409,
              code: "gemini_canvas_video_mode_unavailable",
            });
          }
          await submitPrompt(page, prompt, timeoutMs);
          mediaPromptSubmissionRetryCount = 0;
          continue;
        }

        if (
          operation === "video" &&
          media.length === 0 &&
          bodyTextSuggestsVideoTemplateSelection(lastSnapshot?.pageState?.bodyText) &&
          videoTemplateSelectionAttempts < 3
        ) {
          const templateAttemptIndex = videoTemplateSelectionAttempts;
          videoTemplateSelectionAttempts += 1;
          const templateSelection = await trySelectVideoTemplateCard(
            page,
            timeoutMs,
            templateAttemptIndex,
          ).catch((error) => ({
            clicked: false,
            reason: "template_selection_error",
            errorMessage: error instanceof Error ? error.message : String(error),
          }));
          log(
            "video template selection attempt",
            JSON.stringify({
              operation,
              pollCount,
              attempt: videoTemplateSelectionAttempts,
              result: templateSelection,
              bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
            }),
          );
          if (templateSelection?.clicked) {
            videoTemplateSelectedAt = Date.now();
            if (templateSelection?.videoCreateClicked) {
              videoCreateClickedAt = Date.now();
              videoCreateClickAttempts += 1;
            }
            continue;
          }
        }

        if (
          operation === "video" &&
          media.length === 0 &&
          videoTemplateSelectedAt &&
          Date.now() - videoTemplateSelectedAt < 20_000 &&
          /创作视频|制作视频|Create video/i.test(String(lastSnapshot?.pageState?.bodyText || "")) &&
          videoCreateClickAttempts < 3 &&
          (!videoCreateClickedAt || Date.now() - videoCreateClickedAt > 3_000)
        ) {
          const videoCreateClicked = await tryClickVideoCreateAction(page, timeoutMs).catch(
            () => false,
          );
          videoCreateClickAttempts += 1;
          if (videoCreateClicked) {
            videoCreateClickedAt = Date.now();
          }
          log(
            "video create action attempt",
            JSON.stringify({
              operation,
              pollCount,
              attempt: videoCreateClickAttempts,
              clicked: videoCreateClicked,
              bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
            }),
          );
          if (videoCreateClicked) {
            await page.waitForTimeout(1_500);
            continue;
          }
        }

        const videoTransientFailure =
          operation === "video"
          && /something went wrong\s*\(1155\)/i.test(
            String(lastSnapshot?.pageState?.bodyText || ""),
          );
        if (videoTransientFailure && videoTransientFailureRetryCount < 3) {
          videoTransientFailureRetryCount += 1;
          log(
            "retrying video prompt after transient provider failure",
            JSON.stringify({
              operation,
              pollCount,
              attempt: videoTransientFailureRetryCount,
            }),
          );
          await page.waitForTimeout(2_000);
          await resetConversation(page, baseUrl, timeoutMs, null, {
            skipInitialNavigationWhenAppSurfaceReady: true,
          });
          const retryModeSelected = await clickOperationMode(page, operation, timeoutMs);
          if (!retryModeSelected) {
            throw Object.assign(new Error("Gemini Canvas video mode could not be reactivated."), {
              status: 409,
              code: "gemini_canvas_video_mode_unavailable",
            });
          }
          await submitPrompt(page, prompt, timeoutMs);
          videoTemplateSelectionAttempts = 0;
          videoTemplateSelectedAt = null;
          videoCreateClickAttempts = 0;
          videoCreateClickedAt = null;
          continue;
        }

        const providerGate =
          detectMediaProviderGate(operation, quotaGateText)
          || detectMediaProviderGate(operation, lastSnapshot?.pageState?.bodyText)
          || detectMediaProviderGate(operation, recentMediaProviderGateText(capture.state));
        if (
          shouldBlockMediaProviderGate(
            operation,
            providerGate,
            media,
            capture.state.invokeContract,
            lastSnapshot?.pageState?.bodyText,
          )
        ) {
          if (
            operation === "video" &&
            videoTemplateSelectedAt &&
            Date.now() - videoTemplateSelectedAt < 20_000
          ) {
            log(
              "video provider gate deferred after template selection",
              JSON.stringify({
                operation,
                pollCount,
                code: providerGate.code,
                msSinceTemplateSelection: Date.now() - videoTemplateSelectedAt,
                bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
              }),
            );
            await page.waitForTimeout(2_000);
            continue;
          }
          log(
            "media provider gate detected",
            JSON.stringify({
              operation,
              pollCount,
              code: providerGate.code,
              bodyPreview: String(lastSnapshot?.pageState?.bodyText || "").slice(0, 400),
            }),
          );
          throw Object.assign(new Error(providerGate.message), {
            status: providerGate.status,
            code: providerGate.code,
            bodyText: lastSnapshot?.pageState?.bodyText ?? null,
            captureState: capture.state,
          });
        }

        const needsPlayerReadyTargetBoost =
          media.length === 0 &&
          (operation === "music" || operation === "video") &&
          /_player_ready$/i.test(String(capture.state.invokeContract?.uiState || "")) &&
          !normalizeString(capture.state.invokeContract?.targetSource);
        if (needsPlayerReadyTargetBoost && !playerReadyPlayAttempted) {
          playerReadyPlayAttempted = true;
          const playClicked = await clickMediaActionButton(page, operation, "play", timeoutMs).catch(
            () => false,
          );
          log(
            "player-ready play target probe",
            JSON.stringify({
              operation,
              pollCount,
              clicked: playClicked,
            }),
          );
          if (playClicked) {
            await page.waitForTimeout(1800);
            continue;
          }
        }
        if (needsPlayerReadyTargetBoost && !playerReadyDownloadAttempted) {
          playerReadyDownloadAttempted = true;
          const downloadClicked = await clickMediaActionButton(
            page,
            operation,
            "download",
            timeoutMs,
          ).catch(() => false);
          log(
            "player-ready download target probe",
            JSON.stringify({
              operation,
              pollCount,
              clicked: downloadClicked,
            }),
          );
          if (downloadClicked) {
            await page.waitForTimeout(2200);
            continue;
          }
        }

        if (media.length > 0) {
          const result = await finalizeMediaOperation({
            operation,
            prompt,
            lastSnapshot,
            page,
            deadline,
            pollCount,
            media,
            capture,
            args,
            now: () => Date.now(),
          });
          if (result !== null) return result;
          // Music settlement needs another snapshot before its result is final.
          continue;
        }

        await page.waitForTimeout(2000);
      }

      throw Object.assign(
        new Error(`Timed out waiting for Gemini Canvas ${operation} result.`),
        {
          status: 504,
          code: "gemini_canvas_media_timeout",
          bodyText: lastSnapshot?.pageState?.bodyText ?? null,
          captureState: capture.state,
        },
      );
  }

  return { pollMediaOperation, finalizeMediaOperation };
}
