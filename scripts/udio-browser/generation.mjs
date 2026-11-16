// One submission owns one immutable track set. Polling never authorizes resubmission.
import { extractTrackIds, readSongs, songsReadyForTarget, requireJsonResponse, isChallengeOutcome } from "./responses.mjs";
const fail = (status, code, message) => Object.assign(new Error(message), { status, code });
export async function runGeneration(options) {
  const { submit, poll, wait, now = Date.now, deadline, waitTimeoutMs, pollIntervalMs, targetAssetKind, waitAudio } = options;
  const remaining = (until = deadline) => {
    const value = Math.min(deadline, until) - now();
    if (value <= 0) throw fail(504, "udio_browser_wait_timeout", "Udio generation timed out; remote task may still exist. Do not resubmit automatically.");
    return value;
  };
  let total = 0;
  function parse(outcome) {
    if (outcome.transportError) throw fail(outcome.status ?? 504, "udio_browser_transport_failed", "Udio browser transport failed; details omitted. Do not resubmit automatically.");
    if (isChallengeOutcome(outcome)) throw fail(outcome.status ?? 403, "udio_browser_challenge_required", "Complete the Udio security check before an explicit new request. Generation was not replayed.");
    const bytes = Buffer.byteLength(String(outcome.text ?? ""));
    if (bytes > 128 * 1024 || bytes > 1024 * 1024 - total) {
      throw fail(502, "udio_browser_metadata_limit", "Udio metadata exceeded its byte budget.");
    }
    total += bytes; // Cumulative JSON admission, not a browser heap/wire cap.
    return requireJsonResponse(outcome, "udio_invalid_generation_json", "Udio metadata JSON is invalid.");
  }
  const submitted = await submit(remaining());
  remaining();
  const body = parse(submitted);
  const trackIds = extractTrackIds(body);
  function select(value) {
    const songs = readSongs(value);
    if (songs.some(song => !trackIds.includes(song.id))) {
      throw fail(502, "udio_browser_song_identity", "Udio returned a song outside the submitted track set.");
    }
    return trackIds.flatMap(id => songs.filter(song => song.id === id));
  }
  let songs = select(body);
  const complete = () => songs.length === trackIds.length && songsReadyForTarget(songs, targetAssetKind);
  const result = () => {
    remaining();
    return { ok: true, status: 200, result: { trackIds, songs, completed: complete(), message: null } };
  };
  if (!waitAudio || complete()) return result();
  const until = Math.min(deadline, now() + waitTimeoutMs);
  while (true) {
    await wait(Math.min(pollIntervalMs, remaining(until)));
    if (total >= 1024 * 1024) throw fail(502, "udio_browser_metadata_limit", "Udio cumulative metadata budget is exhausted.");
    const outcome = await poll(trackIds, remaining(until));
    remaining(until);
    songs = select(parse(outcome)); // Never union disjoint partial snapshots.
    if (complete()) return result();
  }
}
