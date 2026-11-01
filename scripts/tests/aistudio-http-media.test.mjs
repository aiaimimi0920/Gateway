import assert from "node:assert/strict";
import { test } from "node:test";
import { buildWire, decodeResponse } from "../aistudio-http/wire.mjs";

const encoded = Buffer.from("fixture-only-media").toString("base64");
const request = parts => ({ contents: [{ role: "user", parts }], generationConfig: { maxOutputTokens: 128 } });
const frame = (parts, finish = null) => [[[[parts, "model"], finish]]];

test("Playground preserves image, audio, video and PDF inline inputs without URL fetches", () => {
  for (const mimeType of ["image/png", "audio/wav", "video/mp4", "application/pdf"]) {
    const result = buildWire(request([{ text: "describe" }, { inlineData: { mimeType, data: encoded } }]), "fixture");
    assert.deepEqual(result.wire[1][0][0][1], [null, null, [mimeType, encoded]]);
    assert.equal(result.binding, `describe ${encoded}`);
  }
  const result = buildWire(request([{ text: "read" }, { fileData: { mimeType: "application/pdf", fileUri: `data:application/pdf;base64,${encoded}` } }]), "fixture");
  assert.equal(result.wire[1][0][0][1][2][0], "application/pdf");
});

test("media admission rejects remote references, mixed parts, unsupported MIME and invalid base64", () => {
  for (const part of [
    { fileData: { mimeType: "image/png", fileUri: "http://127.0.0.1/private" } },
    { inlineData: { mimeType: "text/html", data: encoded } },
    { inlineData: { mimeType: "image/png", data: "@@@" } },
    { text: "mixed", inlineData: { mimeType: "image/png", data: encoded } },
  ]) assert.throws(() => buildWire(request([{ text: "test" }, part]), "fixture"), /aistudio_http_/);
  assert.throws(() => buildWire(request([{ text: "test" }, { inlineData: {
    mimeType: "application/pdf", data: Buffer.alloc(6 * 1024 * 1024 + 1).toString("base64"),
  } }]), "fixture"), /limit/);
});

test("image and speech generation config maps to the observed Playground slots", () => {
  const body = request([{ text: "generate" }]);
  body.generationConfig = { responseModalities: ["TEXT", "IMAGE"], imageConfig: { aspectRatio: "1:1", imageSize: "1K" } };
  let wire = buildWire(body, "fixture").wire;
  assert.deepEqual(wire[3][14], [2, 1]);
  assert.deepEqual(wire[3][26], ["1:1", "1K"]);
  assert.equal(wire[3][13], null);
  body.generationConfig = { responseModalities: ["AUDIO"], speechConfig: { voiceConfig: { prebuiltVoiceConfig: { voiceName: "Kore" } } } };
  wire = buildWire(body, "fixture").wire;
  assert.deepEqual(wire[3][14], [3]);
  assert.deepEqual(wire[3][15], [[["Kore"]]]);
  assert.equal(wire[3][3], null);
});

test("completed inline media is returned without inventing a required text answer", () => {
  const part = [null, null, ["image/png", encoded]];
  const result = decodeResponse(JSON.stringify([[frame([part], 1)]]));
  assert.deepEqual(result.candidates[0].content.parts, [{ inlineData: { mimeType: "image/png", data: encoded } }]);
  assert.throws(() => decodeResponse(JSON.stringify([[frame([part])]])), /incomplete/);
});

test("PCM deltas merge losslessly while image boundaries and thought privacy are preserved", () => {
  const pcm = value => [null, null, ["audio/l16; rate=24000; channels=1", Buffer.from(value).toString("base64")]];
  const thought = pcm("private"); thought[12] = true;
  const result = decodeResponse(JSON.stringify([[frame([pcm("one"), thought]), frame([pcm("two")], 1)]]));
  const parts = result.candidates[0].content.parts;
  assert.equal(parts.length, 1);
  assert.equal(Buffer.from(parts[0].inlineData.data, "base64").toString(), "onetwo");
});

test("canonical aliases and OpenAI media parts normalize without local or remote resolution", () => {
  const variants = [
    { inline_data: { mime_type: "audio/wav", data: encoded } },
    { type: "input_audio", input_audio: { format: "wav", data: encoded } },
    { type: "file", file: { filename: "test.pdf", file_data: `data:application/pdf;base64,${encoded}` } },
    { type: "input_file", filename: "test.pdf", file_data: `data:application/pdf;base64,${encoded}` },
    { fileData: { mimeType: "image/png", fileUri: `data:image/jpeg;base64,${encoded}` } },
  ];
  for (const part of variants) {
    const body = request([{ text: "read" }, part]); body.system_instruction = { parts: [{ text: "system" }] };
    const { wire } = buildWire(body, "fixture");
    assert.equal(wire[1][0][0][1][2][1], encoded); assert.equal(wire[5][0][0][1], "system");
  }
  assert.throws(() => buildWire(request([{ type: "file", file: { file_id: "foreign-file" } }]), "fixture"), /aistudio_http_/);
});

test("dedicated transcription text is decoded once and never leaks thought metadata", () => {
  const part = Array(23).fill(null); part[22] = ["spoken words"];
  let response = decodeResponse(JSON.stringify([[frame([part], 1)]]));
  assert.equal(response.candidates[0].content.parts[0].text, "spoken words");
  part[1] = "spoken words";
  response = decodeResponse(JSON.stringify([[frame([part], 1)]]));
  assert.equal(response.candidates[0].content.parts[0].text, "spoken words");
});

test("unsupported candidate batching fails before any generation", () => {
  const body = request([{ text: "hello" }]); body.generationConfig.candidateCount = 2;
  assert.throws(() => buildWire(body, "fixture"), /unsupported/);
});
