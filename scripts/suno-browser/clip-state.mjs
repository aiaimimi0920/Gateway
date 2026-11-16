// A browser account feed may include other songs. Only the original complete ID set can finish.
import { createError, clipsReadyForTarget } from "./pure.mjs";
const identityError = () => createError(502, "suno_browser_clip_identity", "Suno clip identity is missing, duplicated or malformed.");
export function snapshot(body, expected = null) {
  const clips = body?.clips;
  if (!Array.isArray(clips) || clips.length > (expected ? 128 : 16) || (!expected && !clips.length)) {
    throw identityError();
  }
  const seen = new Set();
  for (const clip of clips) {
    const id = clip?.id;
    if (typeof id !== "string" || !id || id.trim() !== id || Buffer.byteLength(id) > 128 || seen.has(id)) {
      throw identityError();
    }
    seen.add(id);
    if (clip.status !== undefined && (typeof clip.status !== "string" || !clip.status ||
        clip.status.trim() !== clip.status || Buffer.byteLength(clip.status) > 64)) {
      throw createError(502, "suno_browser_clip_status", "Suno clip status is malformed.");
    }
  }
  const selected = expected ? expected.flatMap((id) => clips.filter((clip) => clip.id === id)) : clips;
  if (selected.some((clip) => ["error", "failed", "canceled", "cancelled"].includes(clip.status?.toLowerCase()))) {
    throw createError(502, "suno_browser_generation_failed", "Suno generation failed; remote detail omitted. Do not resubmit automatically.");
  }
  return selected;
}
export function complete(clips, expected, target) {
  return clips.length === expected.length && clipsReadyForTarget(clips, target);
}
