import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";

export function createMediaTargetOwner({ inferMimeTypeFromUrl, isBlobLikeUrl, isAudioLikeUrl, isVideoLikeUrl }) {
  function inferProgramInvokeMediaTarget(operation, snapshot, captureState) {
    return collectPlayerReadyTargetCandidates(
      operation,
      snapshot || { mediaNodes: [], anchorNodes: [] },
      captureState || { imageUrls: [], audioUrls: [], videoUrls: [] },
    )[0]?.url ?? null;
  }

  function scorePlayerReadyTargetCandidate(operation, candidate) {
    const url = normalizeString(candidate?.url);
    if (!url) {
      return -1;
    }
    const source = String(candidate?.source || "");
    const mimeType = String(candidate?.mimeType || "");
    let score = 0;
    if (/^https?:/i.test(url)) {
      score += 160;
    } else if (isBlobLikeUrl(url)) {
      score += 90;
    }
    if (/network/i.test(source)) {
      score += 120;
    } else if (/anchor/i.test(source)) {
      score += 100;
    } else if (/media_node/i.test(source)) {
      score += 80;
    }
    if (/download/i.test(source)) {
      score += 24;
    }
    if (
      /googlevideo\.com|gvt1\.com|contribution\.usercontent\.google\.com|googleusercontent\.com/i.test(url)
    ) {
      score += 45;
    }
    if (operation === "music") {
      if (/^audio\//i.test(mimeType) || isAudioLikeUrl(url)) {
        score += 70;
      } else if (/^video\//i.test(mimeType) || isVideoLikeUrl(url)) {
        score += 48;
      }
    } else if (operation === "video") {
      if (/^video\//i.test(mimeType) || isVideoLikeUrl(url)) {
        score += 70;
      }
    }
    return score;
  }

  function summarizePlayerReadyTargetCandidate(operation, candidate) {
    const url = normalizeString(candidate?.url);
    if (!url) {
      return null;
    }
    return {
      url,
      source: normalizeString(candidate?.source) ?? null,
      mimeType: normalizeString(candidate?.mimeType) ?? inferMimeTypeFromUrl(url, null),
      kind: normalizeString(candidate?.kind) ?? (operation === "music" ? "audio" : operation),
      download: Boolean(candidate?.download),
      score: scorePlayerReadyTargetCandidate(operation, candidate),
    };
  }

  function pushPlayerReadyTargetCandidate(store, operation, candidate) {
    const summarized = summarizePlayerReadyTargetCandidate(operation, candidate);
    if (!summarized?.url) {
      return;
    }
    const existingIndex = store.findIndex((entry) => entry.url === summarized.url);
    if (existingIndex === -1) {
      store.push(summarized);
      return;
    }
    if ((store[existingIndex]?.score ?? -1) <= summarized.score) {
      store[existingIndex] = summarized;
    }
  }

  function collectRpcBodyDownloadCandidates(operation, captureState) {
    const candidates = [];
    const rpcBodies = [...(captureState?.rpcCaptures || [])]
      .reverse()
      .filter(
        (capture) =>
          capture?.type === "response"
          && (
            normalizeString(capture?.label) === "StreamGenerate"
            || /\/StreamGenerate/i.test(String(capture?.url || ""))
          ),
      )
      .map((capture) => String(capture?.bodyText || ""))
      .filter(Boolean);
    for (const bodyText of rpcBodies) {
      const rawMatches =
        bodyText.match(/https:\/\/contribution\.usercontent\.google\.com\/download\?[^"]+/g) || [];
      for (const rawMatch of rawMatches) {
        const decodedUrl = String(rawMatch)
          .replace(/\\u003d/g, "=")
          .replace(/\\u0026/g, "&");
        const inferredMimeType = inferMimeTypeFromUrl(decodedUrl, null);
        const inferredKind =
          /^audio\//i.test(inferredMimeType) || isAudioLikeUrl(decodedUrl)
            ? "audio"
            : /^video\//i.test(inferredMimeType) || isVideoLikeUrl(decodedUrl)
              ? "video"
              : /^text\//i.test(inferredMimeType)
                ? "text"
                : null;
        if (
          !inferredKind
          || (operation === "music" && inferredKind !== "audio" && inferredKind !== "video")
          || (operation === "video" && inferredKind !== "video")
        ) {
          continue;
        }
        candidates.push({
          url: decodedUrl,
          mimeType: inferredMimeType,
          source: "stream_generate_response_body",
          kind: inferredKind,
          download: true,
        });
      }
    }
    return candidates;
  }

  function collectPlayerReadyTargetCandidates(operation, snapshot, captureState) {
    const candidates = [];

    if (operation === "music") {
      for (const entry of [...(captureState?.audioUrls || [])].reverse()) {
        pushPlayerReadyTargetCandidate(candidates, operation, {
          url: entry?.url,
          mimeType: entry?.mimeType || inferMimeTypeFromUrl(entry?.url, "audio/wav"),
          source: "network_audio_response",
          kind: "audio",
        });
      }
      for (const entry of [...(captureState?.videoUrls || [])].reverse()) {
        pushPlayerReadyTargetCandidate(candidates, operation, {
          url: entry?.url,
          mimeType: entry?.mimeType || inferMimeTypeFromUrl(entry?.url, "video/mp4"),
          source: "network_video_response",
          kind: "video",
        });
      }
      for (const entry of collectRpcBodyDownloadCandidates(operation, captureState)) {
        pushPlayerReadyTargetCandidate(candidates, operation, entry);
      }
    } else if (operation === "video") {
      for (const entry of [...(captureState?.videoUrls || [])].reverse()) {
        pushPlayerReadyTargetCandidate(candidates, operation, {
          url: entry?.url,
          mimeType: entry?.mimeType || inferMimeTypeFromUrl(entry?.url, "video/mp4"),
          source: "network_video_response",
          kind: "video",
        });
      }
      for (const entry of collectRpcBodyDownloadCandidates(operation, captureState)) {
        pushPlayerReadyTargetCandidate(candidates, operation, entry);
      }
    }

    for (const anchor of snapshot?.anchorNodes || snapshot?.anchors || []) {
      const href = normalizeString(anchor?.href);
      if (!href) {
        continue;
      }
      const download = normalizeString(anchor?.download);
      const label = [anchor?.text, anchor?.ariaLabel, anchor?.title].filter(Boolean).join("\n");
      if (operation === "music") {
        if (!download && !isAudioLikeUrl(href) && !isVideoLikeUrl(href) && !/下载音乐作品|Download music|播放|Play/i.test(label)) {
          continue;
        }
        pushPlayerReadyTargetCandidate(candidates, operation, {
          url: href,
          mimeType: inferMimeTypeFromUrl(href, isAudioLikeUrl(href) ? "audio/wav" : "video/mp4"),
          source: download ? "anchor_download_href" : "anchor_media_href",
          kind: isAudioLikeUrl(href) ? "audio" : "video",
          download: Boolean(download),
        });
        continue;
      }
      if (operation === "video") {
        if (!download && !isVideoLikeUrl(href) && !/下载视频|Download video|播放视频|Play video/i.test(label)) {
          continue;
        }
        pushPlayerReadyTargetCandidate(candidates, operation, {
          url: href,
          mimeType: inferMimeTypeFromUrl(href, "video/mp4"),
          source: download ? "anchor_download_href" : "anchor_media_href",
          kind: "video",
          download: Boolean(download),
        });
      }
    }

    for (const node of snapshot?.mediaNodes || []) {
      const url = normalizeString(node?.currentSrc) ?? normalizeString(node?.src);
      if (!url) {
        continue;
      }
      if (operation === "music") {
        if (node.kind === "audio" && (isAudioLikeUrl(url) || isBlobLikeUrl(url))) {
          pushPlayerReadyTargetCandidate(candidates, operation, {
            url,
            mimeType: inferMimeTypeFromUrl(url, "audio/wav"),
            source: "media_node_current_src",
            kind: "audio",
          });
        } else if (node.kind === "video" && (isVideoLikeUrl(url) || isBlobLikeUrl(url))) {
          pushPlayerReadyTargetCandidate(candidates, operation, {
            url,
            mimeType: inferMimeTypeFromUrl(url, "video/mp4"),
            source: "media_node_current_src",
            kind: "video",
          });
        }
        continue;
      }
      if (operation === "video" && node.kind === "video" && (isVideoLikeUrl(url) || isBlobLikeUrl(url))) {
        pushPlayerReadyTargetCandidate(candidates, operation, {
          url,
          mimeType: inferMimeTypeFromUrl(url, "video/mp4"),
          source: "media_node_current_src",
          kind: "video",
        });
      }
    }

    return candidates.sort((left, right) => (right.score ?? 0) - (left.score ?? 0));
  }

  return { collectPlayerReadyTargetCandidates, scorePlayerReadyTargetCandidate };
}
