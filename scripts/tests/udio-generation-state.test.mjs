import test from "node:test";
import assert from "node:assert/strict";
import { runGeneration } from "../udio-browser/generation.mjs";
import { extractTrackIds, readSongs, songsReadyForTarget, requireJsonResponse } from "../udio-browser/responses.mjs";
const song = (id, finished = false) => ({ id, finished, ...(finished && { song_path: `https://asset.invalid/${id}` }) });
const response = body => ({ ok: true, status: 200, contentType: "application/json", text: JSON.stringify(body) });
function fixture(extra = {}) {
  let now = 0, submits = 0, polls = 0;
  const options = { now: () => now, deadline: 100, waitTimeoutMs: 50, pollIntervalMs: 10,
    targetAssetKind: "audio", waitAudio: true,
    wait: async ms => { now += ms; },
    submit: async () => { submits++; return response({ track_ids: ["A", "B"], songs: [] }); },
    poll: async () => { polls++; return response({ songs: [song("B", true), song("A", true)] }); }, ...extra };
  return { options, run: () => runGeneration(options), advance: n => { now += n; },
    get submits() { return submits; }, get polls() { return polls; } };
}
test("Udio strict submission rejects every bad ID, oversize set and conflicting alias", () => {
  for (const ids of [["A", "A"], ["A", 1], [" A"], [""], ["界".repeat(43)], Array.from({ length: 17 }, (_, i) => String(i))]) {
    assert.throws(() => extractTrackIds({ track_ids: ids }), { code: "udio_browser_song_identity" });
  }
  assert.throws(() => extractTrackIds({ track_ids: [], songs: [song("A")] }), { code: "udio_missing_track_ids" });
  assert.throws(() => extractTrackIds({ track_ids: ["A"], trackIds: ["B"] }), { code: "udio_browser_song_identity" });
  assert.deepEqual(extractTrackIds({ songs: [song("B"), song("A")] }), ["B", "A"]);
});
test("Udio songs reject invalid shapes and identity rather than silently filtering", () => {
  for (const songs of [null, [null], [[]], [{}], [song("A"), song("A")], Array.from({ length: 17 }, (_, i) => song(String(i)))]) {
    assert.throws(() => readSongs({ songs }), { code: "udio_browser_song_identity" });
  }
});
test("Udio song failure and malformed status reject before no-wait", async () => {
  for (const status of ["failed", "error", "CANCELLED", "canceled", null, 42, "", " finished"]) {
    const f = fixture({ waitAudio: false, submit: async () => response({ track_ids: ["A"], songs: [{ ...song("A", true), status }] }) });
    await assert.rejects(f.run());
  }
});
test("Udio stream readiness or early asset alone is not final completion", () => {
  assert.equal(songsReadyForTarget([{ id: "A", audio_url: "early", readyToStream: true }], "audio"), false);
  assert.equal(songsReadyForTarget([{ id: "A", status: "future", audio_url: "early" }], "audio"), false);
  assert.equal(songsReadyForTarget([{ id: "A", status: "finished" }], "audio"), false);
});
test("Udio polling uses only original IDs and returns original order", async () => {
  let seen;
  const f = fixture({ poll: async ids => { seen = ids; return response({ songs: [song("B", true), song("A", true)] }); } });
  const result = await f.run(); assert.deepEqual(seen, ["A", "B"]);
  assert.deepEqual(result.result.songs.map(s => s.id), ["A", "B"]); assert.equal(f.submits, 1);
});
test("Udio late submission and polling cannot escape the original deadline", async () => {
  for (const at of ["submit", "poll"]) {
    const f = fixture();
    f.options[at] = async () => { f.advance(200); return response({ track_ids: ["A"], songs: [song("A", true)] }); };
    await assert.rejects(f.run(), { status: 504 });
  }
});
test("Udio cumulative JSON admission stops before another poll when exhausted", async () => {
  const sized = body => { const text = JSON.stringify({ ...body, padding: "" }); return response({ ...body, padding: "x".repeat(128 * 1024 - Buffer.byteLength(text)) }); };
  const f = fixture({ deadline: 10000, waitTimeoutMs: 10000,
    submit: async () => sized({ track_ids: ["A"], songs: [] }) });
  let polls = 0; f.options.poll = async () => { polls++; return sized({ songs: [] }); };
  await assert.rejects(f.run(), { code: "udio_browser_metadata_limit" }); assert.equal(polls, 7);
});
test("Udio oversize, transport and HTTP documents are not replayed or echoed", async () => {
  for (const value of [response({ secret: "private".repeat(20000) }),
    { transportError: true, status: 504, text: "private transport" },
    { ok: false, status: 503, text: "private upstream" }]) {
    let calls = 0; const f = fixture({ submit: async () => { calls++; return value; } });
    await assert.rejects(f.run(), error => { assert.ok(!JSON.stringify(error).includes("private")); return true; });
    assert.equal(calls, 1);
  }
});
test("Udio successful HTML and malformed JSON omit original documents", () => {
  for (const value of [{ ok: true, status: 200, contentType: "text/html", text: "private" },
    { ok: true, status: 200, contentType: "application/json", text: "private {" }]) {
    assert.throws(() => requireJsonResponse(value, "bad_json", "Invalid Udio JSON"), error => {
      assert.ok(!JSON.stringify(error).includes("private")); return true;
    });
  }
});
