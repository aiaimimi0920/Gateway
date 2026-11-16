import { createError } from "./pure.mjs";
import { snapshot, complete } from "./clip-state.mjs";
import { maybeClassifyVideoSubscriptionRequired } from "./browser.mjs";

export async function waitForGeneration(page, responses, body, options) {
  const { targetAssetKind, waitCompletion, waitTimeoutMs, deadline } = options;
  let clips = snapshot(body);
  const ids = clips.map((clip) => clip.id);
  const result = (completed) => ({ ok: true, status: 200, result: { clips, completed, message: null } });
  if (!waitCompletion || complete(clips, ids, targetAssetKind)) {
    return result(complete(clips, ids, targetAssetKind));
  }
  if (targetAssetKind === "video" && await maybeClassifyVideoSubscriptionRequired(page)) {
    throw createError(402, "suno_video_subscription_required", "Suno video generation requires an entitled account.");
  }
  const until = Math.min(deadline, Date.now() + waitTimeoutMs);
  while (Date.now() < until) {
    // Never union snapshots: A complete at t1 and B complete at t2 do not prove A+B complete.
    const body = await responses.next("feed", until - Date.now());
    clips = snapshot(body, ids);
    if (complete(clips, ids, targetAssetKind)) return result(true);
  }
  throw createError(504, "suno_browser_wait_timeout", "Suno generation wait timed out; remote generation may still exist. Do not resubmit automatically.");
}
