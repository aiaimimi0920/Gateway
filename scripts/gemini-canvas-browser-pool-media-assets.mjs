import { normalizeString } from "./gemini-canvas-browser-pool-input.mjs";
import {
  isAudioLikeUrl, isAudioLikeMimeType, isBlobLikeUrl, inferMimeTypeFromUrl,
  isLikelyAvatarUrl, isLikelyNoiseMediaUrl, normalizeGeminiBrowserAssetUrl, isVideoLikeUrl,
} from "./gemini-canvas-browser-pool-media-urls.mjs";

export function createMediaAssetSelectionOwner({ scorePlayerReadyTargetCandidate }) {
  function selectAudioAsset(snapshot, captureState) {
    const audioNode = snapshot.mediaNodes.find((node) => {
      const url = node.currentSrc || node.src;
      return (
        node.kind === "audio" &&
        typeof url === "string" &&
        (/blob:/i.test(url) || isAudioLikeUrl(url))
      );
    });

    const networkCandidate = [...captureState.audioUrls]
      .reverse()
      .find(
        (candidate) =>
          candidate.url &&
          (isAudioLikeMimeType(candidate.mimeType) || isAudioLikeUrl(candidate.url)),
      );
    if (networkCandidate) {
      return {
        url: networkCandidate.url,
        mimeType: networkCandidate.mimeType || "audio/wav",
        bodyBase64: networkCandidate.bodyBase64 || null,
        durationSeconds: audioNode?.duration ?? null,
      };
    }

    const anchorCandidate = [...(snapshot.anchorNodes || [])].reverse().find((entry) => {
      const href = String(entry?.href || "");
      const label = `${entry?.text || ""}\n${entry?.ariaLabel || ""}\n${entry?.title || ""}`;
      return (
        href &&
        (isAudioLikeUrl(href) ||
          isBlobLikeUrl(href) ||
          /下载音乐作品|Download music|播放|Play/i.test(label))
      );
    });
    if (anchorCandidate?.href) {
      return {
        url: anchorCandidate.href,
        mimeType: inferMimeTypeFromUrl(anchorCandidate.href, "audio/wav"),
        durationSeconds: audioNode?.duration ?? null,
      };
    }

    const streamGenerateCandidate = [...(captureState?.rpcCaptures || [])]
      .reverse()
      .find(
        (capture) =>
          capture?.type === "response"
          && (
            capture?.label === "StreamGenerate"
            || /\/StreamGenerate/i.test(String(capture?.url || ""))
          )
          && normalizeString(capture?.bodyText),
      );
    if (streamGenerateCandidate?.bodyText) {
      const rawMatches =
        String(streamGenerateCandidate.bodyText).match(
          /https:\/\/contribution\.usercontent\.google\.com\/download\?[^"]+/g,
        ) || [];
      for (const rawMatch of rawMatches) {
        const decodedUrl = String(rawMatch)
          .replace(/\\u003d/g, "=")
          .replace(/\\u0026/g, "&");
        if (!isAudioLikeUrl(decodedUrl)) {
          continue;
        }
        return {
          url: decodedUrl,
          mimeType: inferMimeTypeFromUrl(decodedUrl, "audio/mpeg"),
          durationSeconds: audioNode?.duration ?? null,
        };
      }
    }

    if (!audioNode) {
      return null;
    }
    const url = audioNode.currentSrc || audioNode.src;
    return {
      url,
      mimeType: inferMimeTypeFromUrl(url, "audio/wav"),
      durationSeconds:
        typeof audioNode.duration === "number" && Number.isFinite(audioNode.duration)
          ? audioNode.duration
          : null,
    };
  }

  function selectAudioAssetFromInvokeContract(invokeContract) {
    const candidates = [];
    const directTarget = normalizeString(invokeContract?.target);
    if (directTarget && (isAudioLikeUrl(directTarget) || isAudioLikeMimeType(invokeContract?.targetMimeType))) {
      candidates.push({
        url: directTarget,
        mimeType: normalizeString(invokeContract?.targetMimeType) || inferMimeTypeFromUrl(directTarget, "audio/mpeg"),
        source: normalizeString(invokeContract?.targetSource) || "invoke_contract_target",
        kind: "audio",
        score: scorePlayerReadyTargetCandidate("music", {
          url: directTarget,
          mimeType: normalizeString(invokeContract?.targetMimeType) || null,
          source: normalizeString(invokeContract?.targetSource) || "invoke_contract_target",
          kind: "audio",
          download: /download/i.test(String(invokeContract?.targetSource || "")),
        }),
      });
    }
    for (const candidate of invokeContract?.targetCandidates || []) {
      const url = normalizeString(candidate?.url);
      const mimeType = normalizeString(candidate?.mimeType) || inferMimeTypeFromUrl(url, null);
      if (!url || (!isAudioLikeUrl(url) && !isAudioLikeMimeType(mimeType))) {
        continue;
      }
      candidates.push({
        url,
        mimeType: mimeType || "audio/mpeg",
        source: normalizeString(candidate?.source) || "invoke_contract_target_candidate",
        kind: "audio",
        score:
          Number.isFinite(candidate?.score)
            ? Number(candidate.score)
            : scorePlayerReadyTargetCandidate("music", {
              url,
              mimeType,
              source: normalizeString(candidate?.source) || "invoke_contract_target_candidate",
              kind: "audio",
              download: Boolean(candidate?.download),
            }),
      });
    }
    const best = candidates
      .sort((left, right) => (Number(right?.score || 0) - Number(left?.score || 0)))[0];
    if (!best?.url) {
      return null;
    }
    return {
      url: best.url,
      mimeType: best.mimeType || "audio/mpeg",
      durationSeconds: null,
    };
  }

  function isInterestingImageNode(node) {
    const src = `${node.currentSrc ?? ""} ${node.src ?? ""}`;
    return (
      node.kind === "image" &&
      (
        /AI 生成/i.test(node.alt ?? "") ||
        (
          /blob:|googleusercontent|googlevideo|gvt1|gg-dl|rd-gg-dl/i.test(src) &&
          !isLikelyAvatarUrl(src) &&
          !isLikelyNoiseMediaUrl(src) &&
          ((node.width ?? 0) >= 256 || (node.height ?? 0) >= 256)
        )
      )
    );
  }

  function isLikelyGeneratedImageUrl(url) {
    return /blob:|googleusercontent|contribution\.usercontent\.google\.com|gg-dl|rd-gg-dl|googlevideo|gvt1/i.test(
      String(url || ""),
    );
  }

  function selectImageAssets(snapshot, captureState) {
    const assets = [];
    const imageNode = snapshot.mediaNodes.find((node) => isInterestingImageNode(node)) ?? null;

    const networkCandidate = [...captureState.imageUrls]
      .reverse()
      .find((candidate) => candidate.url && isLikelyGeneratedImageUrl(candidate.url));
    if (networkCandidate) {
      assets.push({
        kind: "image",
        url: normalizeGeminiBrowserAssetUrl(networkCandidate.url) ?? networkCandidate.url,
        mimeType: inferMimeTypeFromUrl(
          normalizeGeminiBrowserAssetUrl(networkCandidate.url) ?? networkCandidate.url,
          networkCandidate.mimeType || "image/png",
        ),
        bodyBase64: networkCandidate.bodyBase64 || null,
        alt: imageNode?.alt ?? null,
        width: imageNode?.width ?? null,
        height: imageNode?.height ?? null,
        durationSeconds: null,
      });
      return assets;
    }

    if (imageNode && imageNode.src) {
      const normalizedImageUrl = normalizeGeminiBrowserAssetUrl(imageNode.src) ?? imageNode.src;
      assets.push({
        kind: "image",
        url: normalizedImageUrl,
        mimeType: inferMimeTypeFromUrl(normalizedImageUrl, "image/png"),
        bodyBase64: null,
        alt: imageNode.alt ?? null,
        width: imageNode.width ?? null,
        height: imageNode.height ?? null,
        durationSeconds: null,
      });
    }
    return assets;
  }

  function selectVideoAsset(snapshot) {
    const anchorCandidate = [...(snapshot.anchorNodes || [])].reverse().find((entry) => {
      const href = String(entry?.href || "");
      const label = `${entry?.text || ""}\n${entry?.ariaLabel || ""}\n${entry?.title || ""}`;
      return (
        href &&
        (isVideoLikeUrl(href) ||
          isBlobLikeUrl(href) ||
          /下载视频|Download video|播放视频|Play video/i.test(label))
      );
    });
    if (anchorCandidate?.href) {
      return [
        {
          kind: "video",
          url: anchorCandidate.href,
          mimeType: inferMimeTypeFromUrl(anchorCandidate.href, "video/mp4"),
          alt: null,
          width: null,
          height: null,
          durationSeconds: null,
        },
      ];
    }
    const videoNode = snapshot.mediaNodes.find((node) => {
      const url = node.currentSrc || node.src;
      return (
        node.kind === "video" &&
        typeof url === "string" &&
        /contribution\.usercontent\.google\.com|googlevideo\.com|gvt1\.com|\.mp4(\?|$)|\.webm(\?|$)/i.test(
          url,
        )
      );
    });
    if (!videoNode) {
      return [];
    }
    const url = videoNode.currentSrc || videoNode.src;
    return [
      {
        kind: "video",
        url,
        mimeType: inferMimeTypeFromUrl(url, "video/mp4"),
        alt: null,
        width: Number.isFinite(videoNode.width) ? videoNode.width : null,
        height: Number.isFinite(videoNode.height) ? videoNode.height : null,
        durationSeconds:
          typeof videoNode.duration === "number" && Number.isFinite(videoNode.duration)
            ? videoNode.duration
            : null,
      },
    ];
  }

  function selectMediaAssetsForOperation(operation, snapshot, captureState) {
    if (operation === "image") {
      return selectImageAssets(snapshot, captureState);
    }

    if (operation === "music") {
      const audioAsset = selectAudioAsset(snapshot, captureState);
      if (audioAsset) {
        return [
          {
            kind: "audio",
            url: audioAsset.url,
            mimeType: audioAsset.mimeType,
            alt: null,
            width: null,
            height: null,
            durationSeconds: audioAsset.durationSeconds ?? null,
          },
        ];
      }
      const contractAudioAsset = selectAudioAssetFromInvokeContract(captureState?.invokeContract);
      if (contractAudioAsset) {
        return [
          {
            kind: "audio",
            url: contractAudioAsset.url,
            mimeType: contractAudioAsset.mimeType,
            alt: null,
            width: null,
            height: null,
            durationSeconds: contractAudioAsset.durationSeconds ?? null,
          },
        ];
      }
      const networkVideo = [...captureState.videoUrls].reverse().find((candidate) => candidate.url);
      if (networkVideo) {
        return [
          {
            kind: "video",
            url: networkVideo.url,
            mimeType: networkVideo.mimeType || "video/mp4",
            alt: null,
            width: null,
            height: null,
            durationSeconds: null,
          },
        ];
      }
    }

    const domVideo = selectVideoAsset(snapshot);
    if (domVideo.length > 0) {
      return domVideo;
    }

    const networkVideo = [...captureState.videoUrls].reverse().find((candidate) => candidate.url);
    if (networkVideo) {
      return [
        {
          kind: "video",
          url: networkVideo.url,
          mimeType: networkVideo.mimeType || "video/mp4",
          alt: null,
          width: null,
          height: null,
          durationSeconds: null,
        },
      ];
    }

    return [];
  }

  return { selectAudioAsset, selectAudioAssetFromInvokeContract, selectImageAssets, selectVideoAsset, selectMediaAssetsForOperation };
}
