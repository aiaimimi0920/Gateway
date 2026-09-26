import test from "node:test";
import assert from "node:assert/strict";
import { importTestableProbe } from "./gemini-canvas-browserless.fixtures.mjs";

const app = await importTestableProbe();
const partsBody = (...parts) => ({ candidates: [{ content: { parts } }] });
const binary = Buffer.from([0, 128, 255, 10]);
const encoded = binary.toString("base64");

test("browserless payload MIME extension ordering and unknown fallback", () => {
  const cases = [["IMAGE/PNG;type=mp4", ".png"], ["image/jpeg", ".jpg"], ["image/jpg", ".jpg"],
    ["image/webp", ".webp"], ["image/gif", ".gif"], ["audio/wav", ".wav"], ["audio/ogg", ".ogg"],
    ["audio/mpeg", ".mp3"], ["audio/mp3", ".mp3"], ["video/mp4", ".mp4"], ["video/webm", ".webm"],
    ["audio/L16;rate=24000", ".pcm"], ["audio/pcm", ".pcm"], [null, ".bin"], ["unknown", ".bin"]];
  for (const [mime, extension] of cases) assert.equal(app.mimeToExt(mime), extension);
});

test("browserless payload text body preserves prompt and allocates independent objects", () => {
  const prompt = "  fixture prompt\n";
  const body = app.buildTextRequestBody(prompt);
  assert.equal(JSON.stringify(body), '{"contents":[{"role":"user","parts":[{"text":"  fixture prompt\\n"}]}]}');
  body.contents[0].parts.push({ text: "mutated" });
  assert.equal(app.buildTextRequestBody(prompt).contents[0].parts.length, 1);
});

test("browserless payload TTS voice is optional and trimmed without changing prompt", () => {
  const contents = [{ role: "user", parts: [{ text: " fixture " }] }];
  for (const voice of [undefined, null, "", " \n", 42]) {
    assert.deepEqual(app.buildTtsRequestBody(" fixture ", voice), { contents, generationConfig: { responseModalities: ["AUDIO"] } });
  }
  assert.deepEqual(app.buildTtsRequestBody(" fixture ", "  Kore  "), { contents, generationConfig: {
    responseModalities: ["AUDIO"], speechConfig: { voiceConfig: { prebuiltVoiceConfig: { voiceName: "Kore" } } },
  } });
});

test("browserless payload image request retains modality and aspect values", () => {
  assert.deepEqual(app.buildImageRequestBody("fixture"), {
    contents: [{ role: "user", parts: [{ text: "fixture" }] }],
    generationConfig: { responseModalities: ["IMAGE"], imageConfig: { aspectRatio: "1:1" } },
  });
  for (const aspect of ["9:16", "", null]) assert.equal(app.buildImageRequestBody("fixture", aspect).generationConfig.imageConfig.aspectRatio, aspect);
});

test("browserless payload video includes only finite positive numeric duration", () => {
  assert.deepEqual(app.buildVideoCreateRequestBody("fixture"), { instances: [{ prompt: "fixture" }], parameters: { aspectRatio: "16:9" } });
  for (const duration of [null, undefined, 0, -1, NaN, Infinity, "8"]) {
    assert.deepEqual(app.buildVideoCreateRequestBody("fixture", "9:16", duration).parameters, { aspectRatio: "9:16" });
  }
  for (const duration of [0.5, 8]) assert.deepEqual(app.buildVideoCreateRequestBody("fixture", null, duration).parameters, { aspectRatio: null, durationSeconds: duration });
});

test("browserless payload text joins normalized parts from only the first candidate", () => {
  const body = partsBody(null, { text: " one " }, { text: " " }, { text: 7 }, { text: "two\nthree" });
  body.candidates.push({ content: { parts: [{ text: "ignored" }] } });
  const before = structuredClone(body);
  assert.equal(app.extractTextFromGenerateContentResponse(body), "one\ntwo\nthree");
  assert.deepEqual(body, before);
});

test("browserless payload text absent and malformed parts return null", () => {
  for (const body of [null, {}, { candidates: [] }, { candidates: [{ content: { parts: {} } }] }, partsBody({ text: " " })]) {
    assert.equal(app.extractTextFromGenerateContentResponse(body), null);
  }
});

for (const [inlineKey, mimeKey] of [["inlineData", "mimeType"], ["inline_data", "mime_type"]]) {
  test(`browserless payload audio decodes ${inlineKey} bytes without MIME filtering`, () => {
    const body = partsBody(null, { [inlineKey]: { data: " " } }, { [inlineKey]: { data: encoded, [mimeKey]: " image/png " } });
    const before = structuredClone(body);
    assert.deepEqual(app.extractAudioFromGenerateContentResponse(body), { mimeType: "image/png", bytes: binary });
    assert.deepEqual(body, before);
  });
}

test("browserless payload audio defaults MIME and keeps first-candidate restriction", () => {
  assert.deepEqual(app.extractAudioFromGenerateContentResponse(partsBody({ inlineData: { data: encoded } })), { mimeType: "audio/L16;codec=pcm;rate=24000", bytes: binary });
  const body = partsBody({ text: "empty" });
  body.candidates.push({ content: { parts: [{ inlineData: { data: encoded } }] } });
  for (const absent of [null, {}, body, { candidates: [{ content: { parts: {} } }] }]) assert.equal(app.extractAudioFromGenerateContentResponse(absent), null);
});

test("browserless payload inline camel field retains nullish precedence", () => {
  const body = partsBody({ inlineData: {}, inline_data: { data: encoded } });
  assert.equal(app.extractAudioFromGenerateContentResponse(body), null);
  assert.equal(app.extractInlineImageFromGenerateContentResponse(body), null);
});

test("browserless payload image scans candidates and skips non-image and empty data", () => {
  const body = { candidates: [null, { content: { parts: {} } }, { content: { parts: [
    { inlineData: { mimeType: "audio/pcm", data: encoded } },
    { inlineData: { mimeType: "IMAGE/PNG", data: encoded } }, { inlineData: { data: " " } },
  ] } }, { content: { parts: [{ inline_data: { data: encoded, mime_type: " image/webp " } }, { inlineData: { data: "AQ==" } }] } }] };
  const before = structuredClone(body);
  assert.deepEqual(app.extractInlineImageFromGenerateContentResponse(body), { mimeType: "image/webp", bytes: binary });
  assert.deepEqual(body, before);
});

test("browserless payload image default MIME and absent results remain stable", () => {
  assert.deepEqual(app.extractInlineImageFromGenerateContentResponse(partsBody({ inlineData: { data: encoded } })), { mimeType: "image/png", bytes: binary });
  for (const body of [null, {}, { candidates: {} }, partsBody({ inlineData: { mimeType: "audio/pcm", data: encoded } })]) assert.equal(app.extractInlineImageFromGenerateContentResponse(body), null);
});

for (const field of ["imageBytes", "bytesBase64Encoded", "b64_json", "b64Json"]) {
  test(`browserless payload Imagen decodes ${field} in nested records`, () => {
    assert.deepEqual(app.extractImagenImages({ generatedImages: [{ image: { [field]: encoded } }] }), [{ mimeType: "image/png", bytes: binary }]);
  });
}

test("browserless payload Imagen preserves collection and field priority without deduplication", () => {
  const body = { generatedImages: [null, { image: { imageBytes: encoded, bytesBase64Encoded: "AQ==", mime_type: " image/jpeg " } }],
    predictions: [{ imageBytes: " ", bytesBase64Encoded: "AQ==", b64_json: encoded }, { b64Json: encoded }, { image: {}, imageBytes: encoded }] };
  const before = structuredClone(body);
  assert.deepEqual(app.extractImagenImages(body), [
    { mimeType: "image/jpeg", bytes: binary }, { mimeType: "image/png", bytes: Buffer.from([1]) }, { mimeType: "image/png", bytes: binary },
  ]);
  assert.deepEqual(body, before);
});

test("browserless payload Imagen ignores non-array collections and empty records", () => {
  for (const body of [null, {}, { generatedImages: {}, predictions: "x" }, { predictions: [null, {}, { imageBytes: " " }] }]) assert.deepEqual(app.extractImagenImages(body), []);
});

test("browserless payload video URI ordered fallback uses only first samples", () => {
  const response = { generateVideoResponse: { generatedSamples: [{ video: { uri: " first " } }] }, generatedVideos: [{ video: { uri: " second " } }], generated_videos: [{ video: { uri: " third " } }] };
  assert.equal(app.extractVideoUriFromOperation({ response }), "first");
  response.generateVideoResponse.generatedSamples[0].video.uri = " ";
  assert.equal(app.extractVideoUriFromOperation({ response }), "second");
  response.generatedVideos[0].video.uri = null;
  assert.equal(app.extractVideoUriFromOperation({ response }), "third");
  response.generated_videos[0].video.uri = "";
  response.generated_videos.push({ video: { uri: "ignored" } });
  assert.equal(app.extractVideoUriFromOperation({ response }), null);
  assert.equal(app.extractVideoUriFromOperation(null), null);
});

test("browserless payload summary retains error identity counts and strict completion", () => {
  const error = { code: 429, details: ["fixture"] };
  const body = { name: " op ", done: true, error, candidates: [1, 2], generatedImages: [], predictions: [3], response: { generatedVideos: [{ video: { uri: " video " } }] } };
  const summary = app.summarizeJsonBody(body);
  assert.deepEqual(summary, { name: "op", done: true, error, hasCandidates: 2, hasGeneratedImages: 0, hasPredictions: 1, videoUri: "video" });
  assert.equal(summary.error, error);
  for (const done of [false, 1, "true", null]) assert.equal(app.summarizeJsonBody({ ...body, done }).done, false);
});

test("browserless payload summary handles arrays and non-object bodies", () => {
  for (const body of [null, undefined, "x", 2, true]) assert.equal(app.summarizeJsonBody(body), null);
  assert.deepEqual(app.summarizeJsonBody([]), { name: null, done: false, error: null, hasCandidates: 0, hasGeneratedImages: 0, hasPredictions: 0, videoUri: null });
  assert.equal(app.summarizeJsonBody({ error: false, candidates: {} }).error, false);
});
