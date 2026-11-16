const MAX_ERROR_BODY_LENGTH = 8_000;

export function normalizeError(error) {
  const status =
    typeof error?.status === "number" && Number.isFinite(error.status) ? error.status : 500;
  return {
    status,
    code:
      typeof error?.code === "string" && error.code.trim()
        ? error.code.trim()
        : "udio_browser_worker_failed",
    message:
      typeof error?.message === "string" && error.message.trim()
        ? error.message.trim()
        : String(error),
    body:
      typeof error?.body === "string" && error.body.trim()
        ? trimBody(error.body)
        : undefined,
  };
}

export function trimBody(value) {
  const text = String(value ?? "").trim();
  if (!text) {
    return "";
  }
  return text.slice(0, MAX_ERROR_BODY_LENGTH);
}

export function isChallengeOutcome(outcome) {
  const contentType = String(outcome.contentType ?? "").toLowerCase();
  const mitigated = String(outcome.mitigated ?? "").toLowerCase();
  const lower = String(outcome.text ?? "").toLowerCase();
  const userDisallowed = lower.includes("user disallowed");
  return (
    (Number(outcome.status ?? 0) === 403 && userDisallowed) ||
    ([403, 429].includes(Number(outcome.status ?? 0)) &&
      (mitigated.includes("challenge") ||
        contentType.includes("text/html") ||
        lower.includes("<!doctype html") ||
        lower.includes("<html")) &&
      (lower.includes("vercel security checkpoint") ||
        lower.includes("x-vercel-challenge-token") ||
        lower.includes("x-vercel-mitigated") ||
        lower.includes("security checkpoint") ||
        lower.includes("data-astro-cid")))
  );
}

export function requireJsonResponse(outcome, invalidJsonCode, invalidJsonMessage) {
  if (outcome.ok) {
    if (typeof outcome.contentType === "string" && outcome.contentType.split(";")[0].trim().toLowerCase() !== "application/json") {
      throw Object.assign(new Error("Udio response is not JSON; body omitted."), { status: 502, code: "udio_browser_content_type" });
    }
    try {
      return JSON.parse(outcome.text);
    } catch (error) {
      throw Object.assign(new Error(invalidJsonMessage), {
        status: 502,
        code: invalidJsonCode,
      });
    }
  }

  if (Number(outcome.status) === 401) {
    throw Object.assign(new Error("Udio browser session is not authenticated."), {
      status: 401,
      code: "udio_session_unauthorized",
    });
  }

  const lower = String(outcome.text ?? "").toLowerCase();
  if (
    [400, 403, 422].includes(Number(outcome.status)) &&
    (lower.includes("captcha") || lower.includes("hcaptcha"))
  ) {
    throw Object.assign(new Error("Udio captcha verification failed."), {
      status: Number(outcome.status) || 400,
      code: "udio_captcha_verification_failed",
    });
  }

  if (Number(outcome.status) === 403 && lower.includes("user disallowed")) {
    throw Object.assign(
      new Error(
        "Udio requires a security check; generation is not replayed automatically.",
      ),
      {
        status: 403,
        code: "udio_browser_challenge_required",
      },
    );
  }

  throw Object.assign(new Error(`Udio request failed with HTTP ${outcome.status}.`), {
    status: Number(outcome.status) || 500,
    code: "udio_browser_request_failed",
  });
}

export function extractTrackIds(body) {
  const direct = Array.isArray(body?.track_ids)
    ? body.track_ids
    : Array.isArray(body?.trackIds)
      ? body.trackIds
      : null;
  if ((body?.track_ids !== undefined || body?.trackIds !== undefined) && !direct) throw identityError();
  const ids = direct ?? readSongs(body).map(song => song.id);
  if (!ids.length) throw Object.assign(new Error("Udio generation response missing track ids."), {
    status: 502, code: "udio_missing_track_ids",
  });
  validateIds(ids);
  if (body.track_ids !== undefined && body.trackIds !== undefined &&
      JSON.stringify(body.track_ids) !== JSON.stringify(body.trackIds)) throw identityError();
  return ids;
}

export function readSongs(body) {
  if (body?.songs === undefined) return [];
  const songs = body.songs;
  if (!Array.isArray(songs) || songs.some(song => !song || typeof song !== "object" || Array.isArray(song))) throw identityError();
  validateIds(songs.map(song => song.id));
  for (const song of songs) {
    for (const status of [song.status, song.state]) {
      if (status !== undefined && (typeof status !== "string" || !status || status.trim() !== status || Buffer.byteLength(status) > 64)) throw identityError();
      if (["error", "failed", "canceled", "cancelled"].includes(status?.toLowerCase())) {
        throw Object.assign(new Error("Udio generation failed; remote detail omitted. Do not resubmit automatically."), {
          status: 502, code: "udio_browser_generation_failed",
        });
      }
    }
  }
  return songs;
}

const identityError = () => Object.assign(new Error("Udio track identity is malformed, duplicated or exceeds its bound."), {
  status: 502, code: "udio_browser_song_identity",
});
function validateIds(ids) {
  if (ids.length > 16 || new Set(ids).size !== ids.length || ids.some(id =>
    typeof id !== "string" || !id || id.trim() !== id || Buffer.byteLength(id) > 128)) throw identityError();
}

export function songsReadyForTarget(songs, targetAssetKind) {
  return (
    Array.isArray(songs) &&
    songs.length > 0 &&
    songs.every((song) => songHasTargetAsset(song, targetAssetKind) &&
      (song.finished === true || ["finished", "complete", "completed"].includes(String(song.status ?? song.state ?? "").toLowerCase())))
  );
}

export function songHasTargetAsset(song, targetAssetKind) {
  if (!song || typeof song !== "object") {
    return false;
  }
  if (targetAssetKind === "image") {
    return hasNonEmptyString(
      song?.image_url ??
        song?.imageUrl ??
        song?.image_path ??
        song?.imagePath ??
        song?.cover_image_url ??
        song?.coverImageUrl ??
        song?.cover_art_url ??
        song?.coverArtUrl,
    );
  }
  if (targetAssetKind === "video") {
    return hasNonEmptyString(song?.video_url ?? song?.videoUrl ?? song?.video_path ?? song?.videoPath);
  }
  return hasNonEmptyString(song?.song_path ?? song?.songPath ?? song?.audio_url ?? song?.audioUrl);
}

export function hasNonEmptyString(value) {
  return typeof value === "string" && value.trim().length > 0;
}

export function timeoutMessageForTarget(targetAssetKind) {
  switch (targetAssetKind) {
    case "image":
      return "Timed out waiting for Udio cover art to reach a terminal state.";
    case "video":
      return "Timed out waiting for Udio video to reach a terminal state.";
    default:
      return "Timed out waiting for Udio audio to reach a terminal state.";
  }
}
