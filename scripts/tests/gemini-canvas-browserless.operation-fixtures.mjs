import path from "node:path";
import vm from "node:vm";

export const plain = (value) => structuredClone(value);
export const assetBytes = Buffer.from([0, 128, 255, 10]);
export const inlineBody = (mimeType = "image/png") => ({ candidates: [{ content: {
  parts: [{ inlineData: { mimeType, data: assetBytes.toString("base64") } }],
} }] });
export const doneVideo = (uri = null) => ({ name: "operations/fixture", done: true,
  response: { generatedVideos: [{ video: { uri } }] },
});
export function operationResult(responseJson = {}, options = {}) {
  return { ok: options.ok ?? true, responseJson,
    response: { status: options.status ?? 200, text: options.text ?? JSON.stringify(responseJson),
      headers: options.headers ?? {}, bytes: options.bytes ?? assetBytes }, ...options };
}

export function operationHarness(app, options = {}) {
  const calls = [], writes = [], sleeps = [], counts = {};
  const failure = new Error("fixture operation boundary failed");
  const context = {
    apiBaseUrl: "https://api.fixture.test/v1///", baseUrl: "https://page.fixture.test/",
    pageOrigin: "https://page.fixture.test", pageReferer: "https://page.fixture.test/share/fallback",
    browserState: { shareUrl: "https://page.fixture.test/share/one", canvasProgramUrl: "https://page.fixture.test/app/one" },
    outDir: "fixture-output", model: "fixture-model", prompt: " fixture prompt ", voiceName: " Kore ",
    aspectRatio: "9:16", durationSeconds: 8, timeoutMs: 45000, videoPollTimeoutMs: 150000,
    ...options.context,
  };
  let now = 1000;
  const boundary = (kind, details) => {
    counts[kind] = (counts[kind] ?? 0) + 1;
    const call = { kind, ...details };
    calls.push(call);
    if (options.failAt === `${kind}:${counts[kind]}`) throw failure;
    return call;
  };
  const respond = (kind, args) => {
    const [receivedContext, label, target, bodyOrTimeout, timeout] = args;
    const custom = kind === "custom", get = kind === "poll" || kind === "bytes";
    const call = boundary(kind, { context: receivedContext, label, url: custom ? target[0].requestUrl : target,
      body: custom ? target[0].requestBody : get ? undefined : bodyOrTimeout,
      timeout: custom || get ? bodyOrTimeout : timeout, attempts: custom ? target : undefined });
    const result = options[kind]?.[counts[kind] - 1] ?? operationResult();
    result.request ??= { url: call.url, body: call.body };
    call.result = result;
    return result;
  };
  const dependencies = { ...app, path, Buffer, Date: { now: () => now },
    sendJsonWithAuthAttempts: async (...args) => respond("json", args),
    sendExactMinimalApiKeyOnlyJson: async (...args) => respond("exact", args),
    sendJsonWithCustomAttempts: async (...args) => respond("custom", args),
    sendGetJsonWithAuthAttempts: async (...args) => respond("poll", args),
    sendGetBytesWithAuthAttempts: async (...args) => respond("bytes", args),
    async writeBuffer(filePath, bytes) {
      boundary("write", { filePath, bytes });
      writes.push({ filePath, bytes });
    },
    async sleep(ms) {
      boundary("sleep", { ms }); sleeps.push(ms); now += options.sleepAdvance ?? ms;
    },
  };
  const names = ["probeText", "probeTts", "probeImage", "probeVideoCreate"];
  const operations = Object.fromEntries(names.map((name) => [name, vm.runInNewContext(`(${app[name].toString()})`, dependencies, { timeout: 1000 })]));
  return { context, calls, writes, sleeps, failure, counts, run: (name) => operations[name](context) };
}
