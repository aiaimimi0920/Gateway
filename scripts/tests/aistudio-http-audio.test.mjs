import assert from "node:assert/strict";
import { test } from "node:test";
import { formatAudio, audioFormat } from "../aistudio-http/audio.mjs";
const pcm = Buffer.from([1, 0, 255, 127, 0, 128, 255, 255]);
const response = () => ({ candidates: [{ content: { parts: [{ inlineData: {
  mimeType: "audio/l16; rate=24000; channels=1", data: pcm.toString("base64"),
} }] }, finishReason: "STOP" }] });
test("AI Studio PCM bytes stay little-endian when wrapped as WAV", async () => {
  const result = await formatAudio(response(), "wav");
  const media = result.candidates[0].content.parts[0].inlineData, bytes = Buffer.from(media.data, "base64");
  assert.equal(media.mimeType, "audio/wav");
  assert.equal(bytes.toString("ascii", 0, 4), "RIFF");
  assert.equal(bytes.readUInt32LE(24), 24000);
  assert.deepEqual(bytes.subarray(44), pcm);
  assert.equal(bytes.readUInt32LE(40), pcm.length);
});
test("PCM output is not silently converted or mislabeled as MP3", async () => {
  const media = (await formatAudio(response(), "pcm")).candidates[0].content.parts[0].inlineData;
  assert.deepEqual(Buffer.from(media.data, "base64"), pcm);
  assert.equal(media.mimeType, "audio/pcm;rate=24000;channels=1");
  assert.throws(() => audioFormat("bad-format"), /unsupported/);
  assert.throws(() => audioFormat("mp3", ""), /encoder_missing/);
});
test("invalid PCM geometry is rejected before building an audio container", async () => {
  const body = response(); body.candidates[0].content.parts[0].inlineData.mimeType = "audio/l16; rate=0; channels=99999";
  await assert.rejects(formatAudio(body, "wav"), /invalid/);
});

test("truncated speech is never published as completed playable output", async () => {
  const body = response(); body.candidates[0].finishReason = "MAX_TOKENS";
  await assert.rejects(formatAudio(body, "wav"), /incomplete/);
});
