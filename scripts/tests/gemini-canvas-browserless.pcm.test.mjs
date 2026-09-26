import test from "node:test";
import assert from "node:assert/strict";
import { importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";

const app = await importTestableProbe();

test("browserless PCM parameters default on missing malformed nonpositive and nonfinite values", () => {
  for (const mime of [null, "audio/pcm", "audio/pcm;rate=0;channels=-1", "audio/pcm;rate=NaN;channels=Infinity", "audio/pcm;RATE=8000;CHANNELS=2"]) {
    assert.equal(app.parsePcmSampleRate(mime), 24000);
    assert.equal(app.parsePcmChannels(mime), 1);
  }
});

test("browserless PCM parameters choose first valid value and preserve Number conversion", () => {
  const mime = "audio/pcm; rate=invalid; channels=0; rate=8e3; channels=2; rate=16000; channels=1";
  assert.equal(app.parsePcmSampleRate(mime), 8000);
  assert.equal(app.parsePcmChannels(mime), 2);
  assert.equal(app.parsePcmSampleRate("rate=2.5"), 2.5);
  assert.equal(app.parsePcmChannels("channels=1.5"), 1.5);
});

function assertWav(wav, pcm, sampleRate, channels) {
  assert.equal(wav.length, 44 + pcm.length);
  assert.equal(wav.toString("ascii", 0, 4), "RIFF");
  assert.equal(wav.readUInt32LE(4), 36 + pcm.length);
  assert.equal(wav.toString("ascii", 8, 16), "WAVEfmt ");
  assert.equal(wav.readUInt32LE(16), 16);
  assert.equal(wav.readUInt16LE(20), 1);
  assert.equal(wav.readUInt16LE(22), channels);
  assert.equal(wav.readUInt32LE(24), sampleRate);
  assert.equal(wav.readUInt32LE(28), sampleRate * channels * 2);
  assert.equal(wav.readUInt16LE(32), channels * 2);
  assert.equal(wav.readUInt16LE(34), 16);
  assert.equal(wav.toString("ascii", 36, 40), "data");
  assert.equal(wav.readUInt32LE(40), pcm.length);
  assert.deepEqual(wav.subarray(44), Buffer.from(pcm));
}

test("browserless PCM default WAV header retains signed sample bytes", () => {
  const pcm = Buffer.from([0, 128, 255, 127]);
  const before = Buffer.from(pcm);
  const wav = app.pcmAudioToWavBytes(pcm, "audio/pcm");
  assertWav(wav, before, 24000, 1);
  assert.deepEqual(pcm, before);
  wav[44] = 99;
  assert.deepEqual(pcm, before);
});

test("browserless PCM L16 swaps complete samples on a copy and retains odd trailing byte", () => {
  const pcm = Buffer.from([1, 2, 3, 4, 255]);
  const before = Buffer.from(pcm);
  assertWav(app.pcmAudioToWavBytes(pcm, "AUDIO/L16;rate=8000;channels=2"), [2, 1, 4, 3, 255], 8000, 2);
  assert.deepEqual(pcm, before);
});

test("browserless PCM accepts typed bytes and emits an empty data chunk", () => {
  assertWav(app.pcmAudioToWavBytes(Uint8Array.from([0, 255]), "audio/pcm;rate=16000;channels=2"), [0, 255], 16000, 2);
  assertWav(app.pcmAudioToWavBytes(Buffer.alloc(0), null), [], 24000, 1);
});

test("browserless PCM native range errors propagate without mutating input", () => {
  const pcm = Buffer.from([1, 2]);
  for (const mime of ["audio/l16;rate=4294967296", "audio/l16;channels=65536"]) {
    assert.throws(() => app.pcmAudioToWavBytes(pcm, mime), { code: "ERR_OUT_OF_RANGE" });
    assert.deepEqual(pcm, Buffer.from([1, 2]));
  }
});
