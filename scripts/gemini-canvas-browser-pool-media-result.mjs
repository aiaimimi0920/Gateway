import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import {
  extractImageBytes as defaultExtractImageBytes,
  extractAudioBytes as defaultExtractAudioBytes,
} from "./gemini-canvas-browser-pool-payload.mjs";
import { isAudioLikeUrl, isAudioLikeMimeType } from "./gemini-canvas-browser-pool-media-urls.mjs";

export function createMediaResultOwner({
  mergeInvokeContract,
  buildCanvasProgramInvokeContract,
  selectMediaAssetsForOperation,
  log,
  bodyIndicatesMusicPendingOrBusy,
  buildProgramHandleState,
  extractImageBytes = defaultExtractImageBytes,
  extractAudioBytes = defaultExtractAudioBytes,
}) {
  async function finalizeMediaOperation({
    operation,
    prompt,
    lastSnapshot,
    page,
    deadline,
    pollCount,
    media,
    capture,
    args,
    now,
  }) {
    const finalInvokeContract = mergeInvokeContract(
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
    capture.state.invokeContract = finalInvokeContract;
    const refreshedMedia = selectMediaAssetsForOperation(operation, lastSnapshot, capture.state);
    let resolvedMedia = refreshedMedia.length > 0 ? refreshedMedia : media;
    if (operation === "image" || operation === "music") {
      resolvedMedia = [];
      for (const asset of (refreshedMedia.length > 0 ? refreshedMedia : media)) {
        if (operation === "image" && asset.kind !== "image") {
          resolvedMedia.push(asset);
          continue;
        }
        if (operation === "music" && asset.kind !== "audio") {
          resolvedMedia.push(asset);
          continue;
        }
        try {
          if (operation === "image") {
            const image = await extractImageBytes(page, asset);
            resolvedMedia.push({
              ...asset,
              mimeType: image.mimeType,
              bodyBase64: image.bodyBase64,
            });
          } else {
            const audio = await extractAudioBytes(page, asset);
            resolvedMedia.push({
              ...asset,
              mimeType: audio.mimeType,
              bodyBase64: audio.bodyBase64,
            });
          }
        } catch (error) {
          const imageDeferred =
            operation === "image"
            && error instanceof Error
            && (
              error.code === "gemini_canvas_non_image_asset"
              || error.code === "gemini_canvas_image_fetch_failed"
            );
          const musicDeferred =
            operation === "music"
            && error instanceof Error
            && (
              error.code === "gemini_canvas_tts_non_audio_asset"
              || error.code === "gemini_canvas_tts_audio_fetch_failed"
            );
          if (imageDeferred || musicDeferred) {
            log(
              `${operation} asset extraction deferred to gateway`,
              JSON.stringify({
                url: asset.url,
                code: error.code ?? null,
                mimeType: error.mimeType ?? null,
              }),
            );
            resolvedMedia.push(asset);
            continue;
          }
          throw error;
        }
      }
    }
    if (operation === "music") {
      const hasInlineAudio = resolvedMedia.some(
        (asset) =>
          asset.kind === "audio"
          && typeof asset.bodyBase64 === "string"
          && asset.bodyBase64.length > 0,
      );
      const hasAudioTargetCandidate = (finalInvokeContract?.targetCandidates || []).some(
        (candidate) =>
          (normalizeString(candidate?.kind) === "audio")
          || isAudioLikeUrl(candidate?.url)
          || isAudioLikeMimeType(candidate?.mimeType),
      );
      const musicPendingOrBusy = bodyIndicatesMusicPendingOrBusy(
        lastSnapshot?.pageState?.bodyText ?? "",
      );
      if (
        !hasInlineAudio
        && !hasAudioTargetCandidate
        && !musicPendingOrBusy
        && now() + 1600 < deadline
      ) {
        log(
          "music media awaiting streamgenerate settlement",
          JSON.stringify({
            pollCount,
            media: resolvedMedia,
          }),
        );
        await page.waitForTimeout(1600);
        return null;
      }
    }
    log(
      "media operation resolved",
      JSON.stringify({
        operation,
        pollCount,
        media: resolvedMedia,
      }),
    );
    return {
      operation,
      pageUrl: lastSnapshot.pageState?.url ?? page.url(),
      bodyText: lastSnapshot.pageState?.bodyText ?? null,
      ...buildProgramHandleState(
        normalizeString(args.baseUrl) ?? "https://gemini.google.com",
        args,
        lastSnapshot.pageState?.url ?? page.url(),
        capture.state,
      ),
      canvasProgramInvokeContract: finalInvokeContract,
      media: resolvedMedia,
      networkEvents: capture.state.events,
      rpcCaptures: capture.state.rpcCaptures,
    };
  }

  return { finalizeMediaOperation };
}
