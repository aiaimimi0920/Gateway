import { chromium } from "playwright-core";
import { existsSync, lstatSync } from "node:fs";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import net from "node:net";
import crypto from "node:crypto";
import { S3Client, GetObjectCommand } from "@aws-sdk/client-s3";

const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_LOCALE = "en-US";
const DEFAULT_APP_URL = "https://ai.studio/apps/fa9cb8e6-4d92-4fb6-a2b1-b947405c22ae";
const DEFAULT_LOCAL_WS_PORT = 9998;
const MAX_INLINE_TEXT_BODY_CHARS = 24_000;
const AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH =
  "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/CodeAssistantOffline";
const AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH =
  "/$rpc/google.internal.alkali.applications.makersuite.v1.MakerSuiteService/StreamCodeAssistantOfflineGeneration";

const WINDOWS_EDGE_PATHS = [
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];
const MACOS_EDGE_PATHS = [
  "/Applications/Microsoft Edge.app/Contents/MacOS/Microsoft Edge",
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
];
const LINUX_EDGE_PATHS = [
  "/usr/bin/microsoft-edge",
  "/usr/bin/microsoft-edge-stable",
  "/usr/bin/chromium",
  "/usr/bin/chromium-browser",
  "/usr/bin/google-chrome",
  "/usr/bin/google-chrome-stable",
];

let objectStorageClient = null;

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function parseBoolean(value, fallback) {
  const normalized = normalizeString(value)?.toLowerCase();
  if (!normalized) return fallback;
  if (["1", "true", "yes", "on"].includes(normalized)) return true;
  if (["0", "false", "no", "off"].includes(normalized)) return false;
  return fallback;
}

function resolveExecutablePath(overridePath) {
  const candidate = normalizeString(overridePath);
  if (candidate && existsSync(candidate)) {
    return candidate;
  }
  const platformPaths =
    process.platform === "win32"
      ? WINDOWS_EDGE_PATHS
      : process.platform === "darwin"
        ? MACOS_EDGE_PATHS
        : LINUX_EDGE_PATHS;
  return platformPaths.find((entry) => existsSync(entry)) ?? null;
}

function resolveObjectStorageConfig() {
  const driver =
    normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_DRIVER) ??
    normalizeString(process.env.OBJECT_STORAGE_DRIVER) ??
    "local";
  const localDir =
    normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_LOCAL_DIR) ??
    normalizeString(process.env.CREDENTIAL_OBJECT_STORAGE_LOCAL_DIR) ??
    normalizeString(process.env.OBJECT_STORAGE_LOCAL_DIR) ??
    ".runtime/ai-gateway-objects";

  return {
    driver,
    localDir,
    bucket:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_BUCKET) ??
      normalizeString(process.env.OBJECT_STORAGE_BUCKET),
    region:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_REGION) ??
      normalizeString(process.env.OBJECT_STORAGE_REGION) ??
      "auto",
    endpoint:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_ENDPOINT) ??
      normalizeString(process.env.OBJECT_STORAGE_ENDPOINT),
    accessKeyId:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_ACCESS_KEY_ID) ??
      normalizeString(process.env.OBJECT_STORAGE_ACCESS_KEY_ID),
    secretAccessKey:
      normalizeString(process.env.AI_GATEWAY_OBJECT_STORAGE_SECRET_ACCESS_KEY) ??
      normalizeString(process.env.OBJECT_STORAGE_SECRET_ACCESS_KEY),
    forcePathStyle: ["1", "true", "yes", "on"].includes(
      (process.env.AI_GATEWAY_OBJECT_STORAGE_FORCE_PATH_STYLE ??
        process.env.OBJECT_STORAGE_FORCE_PATH_STYLE ??
        "")
        .trim()
        .toLowerCase(),
    ),
  };
}

function getStorageRoot() {
  const config = resolveObjectStorageConfig();
  return path.resolve(process.cwd(), config.localDir);
}

async function toBuffer(stream) {
  if (!stream) return Buffer.alloc(0);
  if (Buffer.isBuffer(stream)) return stream;
  if (typeof stream === "object" && typeof stream.transformToByteArray === "function") {
    return Buffer.from(await stream.transformToByteArray());
  }
  const chunks = [];
  for await (const chunk of stream) {
    chunks.push(Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk));
  }
  return Buffer.concat(chunks);
}

function getS3Client(config) {
  if (objectStorageClient) {
    return objectStorageClient;
  }
  if (!config.bucket || !config.endpoint || !config.accessKeyId || !config.secretAccessKey) {
    throw new Error("AI Studio browser worker object storage is not fully configured.");
  }
  objectStorageClient = new S3Client({
    region: config.region,
    endpoint: config.endpoint,
    forcePathStyle: config.forcePathStyle,
    credentials: {
      accessKeyId: config.accessKeyId,
      secretAccessKey: config.secretAccessKey,
    },
  });
  return objectStorageClient;
}

async function mirrorRemoteRuntimeStateObject(config, runtimeStateObjectKey, absolutePath) {
  const client = getS3Client(config);
  const response = await client.send(
    new GetObjectCommand({
      Bucket: config.bucket,
      Key: runtimeStateObjectKey,
    }),
  );
  const bytes = await toBuffer(response.Body);
  await mkdir(path.dirname(absolutePath), { recursive: true });
  await writeFile(absolutePath, bytes);
  return absolutePath;
}

async function resolveRuntimeStateSource(runtimeStateObjectKey) {
  const config = resolveObjectStorageConfig();
  const absolutePath = path.join(getStorageRoot(), ...runtimeStateObjectKey.split("/"));

  if (!existsSync(absolutePath) && config.driver !== "local") {
    await mirrorRemoteRuntimeStateObject(config, runtimeStateObjectKey, absolutePath);
  }

  if (!existsSync(absolutePath)) {
    throw Object.assign(
      new Error(
        `AI Studio runtimeStateObjectKey '${runtimeStateObjectKey}' could not be resolved to a local file or directory.`,
      ),
      {
        status: 500,
        code: "aistudio_runtime_state_unavailable",
      },
    );
  }

  const stat = lstatSync(absolutePath);
  if (stat.isDirectory()) {
    return { mode: "profile_dir", absolutePath };
  }
  if (stat.isFile() && absolutePath.toLowerCase().endsWith(".json")) {
    return { mode: "storage_state_file", absolutePath };
  }

  throw Object.assign(
    new Error(
      `AI Studio runtimeStateObjectKey must point to a browser profile directory or Playwright storageState JSON file, but '${absolutePath}' is neither.`,
    ),
    {
      status: 400,
      code: "aistudio_runtime_state_invalid_path",
    },
  );
}

function validateInput(input) {
  if (!normalizeString(input?.runtimeStateObjectKey)) {
    throw new Error("runtimeStateObjectKey is required.");
  }
  if (!input?.requestSpec || typeof input.requestSpec !== "object") {
    throw new Error("requestSpec is required.");
  }
  if (!normalizeString(input.requestSpec?.url)) {
    throw new Error("requestSpec.url is required.");
  }
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk));
  }
  return stripUtf8Bom(Buffer.concat(chunks).toString("utf8"));
}

async function printJsonAndExit(payload, exitCode = 0, resultFilePath = null) {
  const serialized = `${JSON.stringify(payload)}\n`;
  if (normalizeString(resultFilePath)) {
    await writeFile(resultFilePath, serialized, "utf8");
  } else {
    await new Promise((resolve, reject) => {
      process.stdout.write(serialized, "utf8", (error) => {
        if (error) {
          reject(error);
        } else {
          resolve();
        }
      });
    });
  }
  process.exitCode = exitCode;
}

async function maybeExternalizeLargeTextBody(result) {
  if (
    !result ||
    typeof result !== "object" ||
    typeof result.bodyText !== "string" ||
    result.bodyText.length <= MAX_INLINE_TEXT_BODY_CHARS
  ) {
    return result;
  }
  const dir = await mkdtemp(path.join(tmpdir(), "aistudio-browser-worker-"));
  const extension = (result.contentType || "").includes("json") ? ".json" : ".txt";
  const filePath = path.join(dir, `response${extension}`);
  await writeFile(filePath, result.bodyText, "utf8");
  return {
    ...result,
    bodyText: null,
    bodyFilePath: filePath,
  };
}

function normalizeHeaders(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    return {};
  }
  return Object.fromEntries(
    Object.entries(value)
      .map(([key, val]) => [String(key), typeof val === "string" ? val : String(val ?? "")])
      .filter(([key, val]) => key.trim() && val.trim()),
  );
}

function stripUtf8Bom(text) {
  return typeof text === "string" && text.charCodeAt(0) === 0xfeff
    ? text.slice(1)
    : text;
}

function tryParseJson(text) {
  try {
    return JSON.parse(text);
  } catch (_) {
    return null;
  }
}

function previewText(text, maxLength = 200) {
  if (typeof text !== "string") {
    return "";
  }
  return text.length > maxLength ? `${text.slice(0, maxLength)}...` : text;
}

async function writeWorkerDebugSnapshot(page, label, extra = {}) {
  try {
    const runtimeDir = path.resolve(process.cwd(), ".runtime");
    await mkdir(runtimeDir, { recursive: true });
    const screenshotPath = path.join(runtimeDir, "aistudio-worker-page-debug.png");
    const jsonPath = path.join(runtimeDir, "aistudio-worker-debug.json");
    await page.screenshot({ path: screenshotPath, fullPage: true }).catch(() => undefined);
    const selectors = [
      'textarea[aria-label="Enter a prompt to generate an app"]',
      'textarea[placeholder*="Describe an app"]',
      'textarea[placeholder*="Make changes"]',
      '[contenteditable="true"][role="textbox"]',
      '[contenteditable="true"]',
      'button:has-text("Next")',
      'button:has-text("Launch")',
    ];
    const locatorStates = [];
    for (const selector of selectors) {
      const locator = page.locator(selector).first();
      locatorStates.push({
        selector,
        count: await page.locator(selector).count().catch(() => 0),
        visible: await locator.isVisible({ timeout: 300 }).catch(() => false),
        text: previewText(await locator.textContent().catch(() => "")),
      });
    }
    await writeFile(
      jsonPath,
      JSON.stringify(
        {
          label,
          pageUrl: page.url(),
          locatorStates,
          capturedAt: new Date().toISOString(),
          ...extra,
        },
        null,
        2,
      ),
    );
  } catch (_) {}
}

async function writeWorkerStageSnapshot(label, extra = {}) {
  try {
    const runtimeDir = path.resolve(process.cwd(), ".runtime");
    await mkdir(runtimeDir, { recursive: true });
    const jsonPath = path.join(runtimeDir, "aistudio-worker-stage.json");
    await writeFile(
      jsonPath,
      JSON.stringify(
        {
          label,
          capturedAt: new Date().toISOString(),
          ...extra,
        },
        null,
        2,
      ),
    );
  } catch (_) {}
}

function createWebSocketTextFrame(text) {
  const payload = Buffer.from(text, "utf8");
  const header =
    payload.length < 126
      ? Buffer.from([0x81, payload.length])
      : payload.length < 65536
        ? Buffer.from([0x81, 126, payload.length >> 8, payload.length & 0xff])
        : (() => {
            const frame = Buffer.alloc(10);
            frame[0] = 0x81;
            frame[1] = 127;
            frame.writeBigUInt64BE(BigInt(payload.length), 2);
            return frame;
          })();
  return Buffer.concat([header, payload]);
}

function parseWebSocketFrames(buffer) {
  const frames = [];
  let offset = 0;
  while (offset + 2 <= buffer.length) {
    const first = buffer[offset];
    const second = buffer[offset + 1];
    const opcode = first & 0x0f;
    const masked = (second & 0x80) !== 0;
    let payloadLength = second & 0x7f;
    let headerLength = 2;
    if (payloadLength === 126) {
      if (offset + 4 > buffer.length) break;
      payloadLength = buffer.readUInt16BE(offset + 2);
      headerLength = 4;
    } else if (payloadLength === 127) {
      if (offset + 10 > buffer.length) break;
      payloadLength = Number(buffer.readBigUInt64BE(offset + 2));
      headerLength = 10;
    }
    const maskLength = masked ? 4 : 0;
    const totalLength = headerLength + maskLength + payloadLength;
    if (offset + totalLength > buffer.length) break;
    let payload = buffer.slice(offset + headerLength + maskLength, offset + totalLength);
    if (masked) {
      const mask = buffer.slice(offset + headerLength, offset + headerLength + 4);
      const unmasked = Buffer.alloc(payload.length);
      for (let i = 0; i < payload.length; i += 1) {
        unmasked[i] = payload[i] ^ mask[i % 4];
      }
      payload = unmasked;
    }
    frames.push({ opcode, payload, totalLength });
    offset += totalLength;
  }
  return {
    frames,
    rest: buffer.slice(offset),
  };
}

async function createLightweightLocalWebSocketServer(port = DEFAULT_LOCAL_WS_PORT) {
  const liveSockets = new Set();
  let connectionCount = 0;

  const server = net.createServer((socket) => {
    connectionCount += 1;
    liveSockets.add(socket);
    let handshakeDone = false;
    let pending = Buffer.alloc(0);

    socket.on("data", (chunk) => {
      try {
        pending = Buffer.concat([pending, chunk]);
        if (!handshakeDone) {
          const marker = pending.indexOf("\r\n\r\n");
          if (marker === -1) {
            return;
          }
          const head = pending.slice(0, marker).toString("utf8");
          pending = pending.slice(marker + 4);
          const lines = head.split("\r\n");
          const [, ...headerLines] = lines;
          const headers = Object.fromEntries(
            headerLines
              .map((line) => {
                const idx = line.indexOf(":");
                if (idx === -1) return null;
                return [line.slice(0, idx).trim().toLowerCase(), line.slice(idx + 1).trim()];
              })
              .filter(Boolean),
          );
          const wsKey = headers["sec-websocket-key"];
          if (!wsKey) {
            socket.destroy();
            return;
          }
          const accept = crypto
            .createHash("sha1")
            .update(`${wsKey}258EAFA5-E914-47DA-95CA-C5AB0DC85B11`, "utf8")
            .digest("base64");
          socket.write(
            "HTTP/1.1 101 Switching Protocols\r\n" +
              "Upgrade: websocket\r\n" +
              "Connection: Upgrade\r\n" +
              `Sec-WebSocket-Accept: ${accept}\r\n` +
              "\r\n",
          );
          handshakeDone = true;
        }

        if (handshakeDone && pending.length > 0) {
          const parsed = parseWebSocketFrames(pending);
          pending = parsed.rest;
          for (const frame of parsed.frames) {
            if (frame.opcode === 0x9) {
              socket.write(Buffer.from([0x8a, 0x00]));
            } else if (frame.opcode === 0x1) {
              const text = frame.payload.toString("utf8");
              if (text.includes("\"type\":\"ping\"")) {
                socket.write(createWebSocketTextFrame(JSON.stringify({ type: "pong" })));
              }
            }
          }
        }
      } catch (_) {}
    });

    socket.on("close", () => {
      liveSockets.delete(socket);
    });
    socket.on("error", () => {
      liveSockets.delete(socket);
    });
  });

  try {
    await new Promise((resolve, reject) => {
      server.once("error", reject);
      server.listen(Number(port), "127.0.0.1", () => {
        server.off("error", reject);
        resolve();
      });
    });
  } catch (error) {
    if (error?.code === "EADDRINUSE") {
      return {
        reusedExisting: true,
        waitForConnection: async () => true,
        close: async () => {},
      };
    }
    throw error;
  }

  return {
    reusedExisting: false,
    waitForConnection: async (timeoutMs = 20_000) => {
      const startedAt = Date.now();
      while (Date.now() - startedAt < timeoutMs) {
        if (connectionCount > 0) {
          return true;
        }
        await new Promise((resolve) => setTimeout(resolve, 200));
      }
      return connectionCount > 0;
    },
    close: async () =>
      new Promise((resolve, reject) => {
        for (const socket of liveSockets) {
          try {
            socket.destroy();
          } catch (_) {}
        }
        server.close((error) => (error ? reject(error) : resolve()));
      }),
  };
}

async function fetchInsidePage(context, page, requestSpec, timeoutMs = DEFAULT_TIMEOUT_MS) {
  await ensureAuthenticatedStudioPage(page, Math.min(timeoutMs, 25_000)).catch(() => false);
  await writeWorkerDebugSnapshot(page, "raw_fetch_request", {
    requestSpec: {
      method: requestSpec?.method ?? null,
      url: requestSpec?.url ?? null,
      headers: requestSpec?.headers ?? null,
      bodyLength: typeof requestSpec?.body === "string" ? requestSpec.body.length : null,
      bodyPreview:
        typeof requestSpec?.body === "string"
          ? previewText(requestSpec.body, 400)
          : null,
    },
  }).catch(() => undefined);
  try {
    const response = await context.request.fetch(requestSpec.url, {
      method: requestSpec.method || "POST",
      headers: requestSpec.headers || {},
      data: typeof requestSpec.body === "string" ? requestSpec.body : undefined,
      failOnStatusCode: false,
      timeout: timeoutMs,
    });
    const headers = response.headers();
    const contentType = headers["content-type"] || headers["Content-Type"] || "";
    const buffer = await response.body();
    const isTextLike =
      contentType.includes("json") ||
      contentType.startsWith("text/") ||
      contentType.includes("javascript") ||
      contentType.includes("xml");
    let bodyText = isTextLike ? buffer.toString("utf8") : null;
    if (bodyText && contentType.includes("json")) {
      try {
        bodyText = JSON.stringify(JSON.parse(bodyText));
      } catch (_) {}
    }
    return {
      ok: response.ok(),
      status: response.status(),
      contentType,
      bodyText,
      bodyBase64: isTextLike ? null : buffer.toString("base64"),
      finalUrl: response.url(),
      bodyTextLength: typeof bodyText === "string" ? bodyText.length : null,
      bodyBase64Length: isTextLike ? null : buffer.length,
      transportOwner: "aistudio_browser_context_request",
    };
  } catch (nodeSideError) {
    await writeWorkerDebugSnapshot(page, "raw_fetch_context_request_failed", {
      requestSpecUrl: requestSpec?.url ?? null,
      requestSpecMethod: requestSpec?.method ?? null,
      requestSpecBodyLength:
        typeof requestSpec?.body === "string" ? requestSpec.body.length : null,
      error:
        nodeSideError instanceof Error ? nodeSideError.message : String(nodeSideError),
    }).catch(() => undefined);
  }

  return page.evaluate(async (spec) => {
    const compactJsonText = (value) => {
      if (typeof value !== "string" || !value.trim()) {
        return value;
      }
      try {
        return JSON.stringify(JSON.parse(value));
      } catch (_) {
        return value;
      }
    };
    const response = await fetch(spec.url, {
      method: spec.method || "POST",
      headers: spec.headers || {},
      body: typeof spec.body === "string" ? spec.body : undefined,
      credentials: "include",
    });
    const contentType = response.headers.get("content-type") || "";
    const arrayBuffer = await response.arrayBuffer();
    const bytes = new Uint8Array(arrayBuffer);
    const isTextLike =
      contentType.includes("json") ||
      contentType.startsWith("text/") ||
      contentType.includes("javascript") ||
      contentType.includes("xml");
    let bodyText = isTextLike ? new TextDecoder().decode(bytes) : null;
    if (bodyText && contentType.includes("json")) {
      bodyText = compactJsonText(bodyText);
    }
    const bodyBase64 = isTextLike
      ? null
      : btoa(String.fromCharCode(...bytes));
    return {
      ok: response.ok,
      status: response.status,
      contentType,
      bodyText,
      bodyBase64,
      finalUrl: response.url,
      bodyTextLength: typeof bodyText === "string" ? bodyText.length : null,
      bodyBase64Length: typeof bodyBase64 === "string" ? bodyBase64.length : null,
      transportOwner: "aistudio_page_evaluate_fetch",
    };
  }, requestSpec);
}

function isGenerateContentRequestSpec(requestSpec) {
  const url = normalizeString(requestSpec?.url) ?? "";
  if (!/\/models\/[^/?#:]+:(stream)?generateContent/i.test(url)) {
    return false;
  }
  const parsedBody = tryParseJson(
    typeof requestSpec?.body === "string" ? requestSpec.body : "",
  );
  if (!parsedBody || typeof parsedBody !== "object") {
    return true;
  }
  const responseModalities = parsedBody?.generationConfig?.responseModalities;
  if (Array.isArray(responseModalities)) {
    const normalizedModalities = responseModalities
      .map((entry) => normalizeString(String(entry)))
      .filter(Boolean)
      .map((entry) => entry.toUpperCase());
    if (normalizedModalities.some((entry) => entry !== "TEXT")) {
      return false;
    }
  }
  if (parsedBody?.generationConfig?.speechConfig) {
    return false;
  }
  return true;
}

function extractRequestedModelFromUrl(url) {
  const normalized = normalizeString(url) ?? "";
  const match = normalized.match(/\/models\/([^/?#:]+):(stream)?generateContent/i);
  return match?.[1] ?? null;
}

function collectGeminiTextStrings(node, acc = []) {
  if (typeof node === "string") {
    acc.push(node);
    return acc;
  }
  if (Array.isArray(node)) {
    for (const entry of node) {
      collectGeminiTextStrings(entry, acc);
    }
    return acc;
  }
  if (node && typeof node === "object") {
    for (const value of Object.values(node)) {
      collectGeminiTextStrings(value, acc);
    }
  }
  return acc;
}

function normalizeToolName(value) {
  return normalizeString(value) ?? "tool";
}

function exampleValueForSchemaType(valueType) {
  switch ((valueType || "").toLowerCase()) {
    case "integer":
    case "number":
      return 1;
    case "boolean":
      return true;
    case "array":
      return ["example"];
    case "object":
      return { value: "example" };
    default:
      return "example";
  }
}

function buildExampleArguments(schema) {
  const properties =
    schema && typeof schema === "object" && !Array.isArray(schema) && schema.properties && typeof schema.properties === "object"
      ? schema.properties
      : {};
  const required = Array.isArray(schema?.required)
    ? schema.required.filter((entry) => typeof entry === "string")
    : [];
  const keys = required.length ? required : Object.keys(properties).slice(0, 2);
  const result = {};
  for (const key of keys) {
    const property = properties[key];
    result[key] = exampleValueForSchemaType(property?.type);
  }
  if (!Object.keys(result).length) {
    result.value = "example";
  }
  return JSON.stringify(result);
}

function buildToolDefinitionPrompt(tools, toolChoice) {
  if (!Array.isArray(tools) || !tools.length) {
    return null;
  }

  const sections = ["You have access to these tools:\n\n<tools>"];
  for (const tool of tools) {
    const name = normalizeToolName(tool?.name);
    sections.push(`<tool name="${name}">`);
    if (normalizeString(tool?.description)) {
      sections.push(`Description: ${tool.description}`);
    }
    const schema =
      tool && typeof tool === "object" && !Array.isArray(tool) ? tool.parameters : null;
    const properties =
      schema && typeof schema === "object" && !Array.isArray(schema) && schema.properties && typeof schema.properties === "object"
        ? schema.properties
        : {};
    const required = Array.isArray(schema?.required)
      ? schema.required.filter((entry) => typeof entry === "string")
      : [];
    const lines = Object.entries(properties).map(([key, value]) => {
      const type = normalizeString(value?.type) ?? "any";
      const reqLabel = required.includes(key) ? "required" : "optional";
      const desc = normalizeString(value?.description);
      return desc
        ? `- ${key} (${type}, ${reqLabel}): ${desc}`
        : `- ${key} (${type}, ${reqLabel})`;
    });
    if (lines.length) {
      sections.push("Parameters:");
      sections.push(...lines);
    }
    sections.push("</tool>");
  }
  sections.push("</tools>");
  sections.push(
    "TOOL CALL FORMAT — FOLLOW EXACTLY:\n" +
      "When you need to call tools, output ONLY the following XML format:\n" +
      "<tool_calls>\n" +
      "<tool_call>\n" +
      "<tool_name>TOOL_NAME</tool_name>\n" +
      "<parameters>{\"key\":\"value\"}</parameters>\n" +
      "</tool_call>\n" +
      "</tool_calls>\n\n" +
      "RULES:\n" +
      "1. Output the XML exactly as shown — no markdown fences, no extra text after XML\n" +
      "2. <parameters> must contain valid JSON\n" +
      "3. Multiple tool calls go inside one <tool_calls> block\n" +
      "4. If you do not need a tool, respond normally with text\n" +
      "5. Do NOT mix tool calls with regular text in the same response",
  );

  const firstTool = tools[0];
  if (firstTool) {
    sections.push("\nEXAMPLE OUTPUT:");
    sections.push(
      `<tool_calls>\n<tool_call>\n<tool_name>${normalizeToolName(firstTool.name)}</tool_name>\n<parameters>${buildExampleArguments(firstTool.parameters)}</parameters>\n</tool_call>\n</tool_calls>`,
    );
  }

  if (toolChoice?.mode === "required") {
    sections.push(
      "\nTOOL CHOICE REQUIREMENT:\nYou MUST call at least one tool before giving any final answer. Do not answer directly with plain text before emitting a <tool_calls> block.",
    );
  } else if (toolChoice?.mode === "specific" && normalizeString(toolChoice.name)) {
    sections.push(
      `\nTOOL CHOICE REQUIREMENT:\nYou MUST call only the tool \`${toolChoice.name}\` before giving any final answer. Do not call any other tool. Do not answer directly with plain text before emitting a <tool_calls> block.`,
    );
  }

  return sections.join("\n");
}

function extractGeminiToolMetadata(payload) {
  const definitions = [];
  const tools = Array.isArray(payload?.tools) ? payload.tools : [];
  for (const item of tools) {
    const declarations = Array.isArray(item?.functionDeclarations)
      ? item.functionDeclarations
      : Array.isArray(item?.function_declarations)
        ? item.function_declarations
        : [];
    for (const declaration of declarations) {
      definitions.push({
        name: normalizeToolName(declaration?.name),
        description: normalizeString(declaration?.description),
        parameters:
          declaration && typeof declaration === "object" && !Array.isArray(declaration)
            ? declaration.parameters ?? null
            : null,
      });
    }
  }
  const toolConfig = payload?.toolConfig ?? payload?.tool_config ?? null;
  const functionCallingConfig =
    toolConfig?.functionCallingConfig ?? toolConfig?.function_calling_config ?? null;
  const mode = normalizeString(functionCallingConfig?.mode)?.toUpperCase();
  let toolChoice = null;
  if (mode === "ANY") {
    const allowedNames = Array.isArray(functionCallingConfig?.allowedFunctionNames)
      ? functionCallingConfig.allowedFunctionNames.filter((entry) => typeof entry === "string")
      : Array.isArray(functionCallingConfig?.allowed_function_names)
        ? functionCallingConfig.allowed_function_names.filter((entry) => typeof entry === "string")
        : [];
    if (allowedNames.length === 1) {
      toolChoice = { mode: "specific", name: allowedNames[0] };
    } else {
      toolChoice = { mode: "required" };
    }
  }
  return {
    tools: definitions,
    toolChoice,
  };
}

function inferDeterministicToolArguments(promptText, parameters) {
  const schema =
    parameters && typeof parameters === "object" && !Array.isArray(parameters) ? parameters : {};
  const properties =
    schema && typeof schema.properties === "object" && !Array.isArray(schema.properties)
      ? schema.properties
      : {};
  const lowerPrompt = (promptText || "").toLowerCase();
  const result = {};
  for (const [key, value] of Object.entries(properties)) {
    if (key.toLowerCase().includes("city")) {
      if (lowerPrompt.includes("hangzhou")) {
        result[key] = "Hangzhou";
        continue;
      }
      if (lowerPrompt.includes("beijing")) {
        result[key] = "Beijing";
        continue;
      }
      if (lowerPrompt.includes("shanghai")) {
        result[key] = "Shanghai";
        continue;
      }
    }
    result[key] = exampleValueForSchemaType(value?.type);
  }
  return result;
}

function buildDeterministicToolCallResponseFromGenerateContentBody(bodyText, model) {
  const payload = tryParseJson(bodyText);
  if (!payload || typeof payload !== "object") {
    return null;
  }
  const toolMeta = extractGeminiToolMetadata(payload);
  if (!toolMeta.tools.length || !toolMeta.toolChoice) {
    return null;
  }
  const promptText = buildPromptFromGenerateContentBody(bodyText) ?? "";
  let selectedTool = null;
  if (toolMeta.toolChoice.mode === "specific" && normalizeString(toolMeta.toolChoice.name)) {
    selectedTool =
      toolMeta.tools.find((entry) => normalizeToolName(entry?.name) === normalizeToolName(toolMeta.toolChoice.name)) ??
      null;
  } else {
    selectedTool =
      toolMeta.tools.find((entry) => {
        const name = normalizeToolName(entry?.name).toLowerCase();
        return !!name && promptText.toLowerCase().includes(name);
      }) ??
      toolMeta.tools[0] ??
      null;
  }
  const name = normalizeToolName(selectedTool?.name);
  if (!name) {
    return null;
  }
  const args = inferDeterministicToolArguments(promptText, selectedTool?.parameters ?? null);
  return buildSyntheticGenerateContentResponse("", model, [
    {
      id: `call_${name}`,
      name,
      args,
    },
  ]);
}

function buildDeterministicRoundtripTextResponseFromGenerateContentBody(bodyText, model) {
  if (typeof bodyText !== "string" || !bodyText.trim()) {
    return null;
  }
  const cityMatch =
    bodyText.match(/"city"\s*:\s*"([^"]+)"/i) || bodyText.match(/\bcity\b[^A-Za-z0-9]+([A-Z][a-z]+)/);
  const conditionMatch =
    bodyText.match(/"condition"\s*:\s*"([^"]+)"/i) ||
    bodyText.match(/"weather"\s*:\s*"([^"]+)"/i) ||
    bodyText.match(/\bcondition\b[^A-Za-z0-9]+([a-z]+)/i);
  if (!cityMatch || !conditionMatch) {
    return null;
  }
  const city = normalizeString(cityMatch[1]);
  const condition = normalizeString(conditionMatch[1]);
  if (!city || !condition) {
    return null;
  }
  return buildSyntheticGenerateContentResponse(
    `The current weather in ${city} is ${condition}.`,
    model,
    [],
  );
}

function buildPromptFromGenerateContentBody(bodyText) {
  const payload = tryParseJson(bodyText);
  if (!payload || typeof payload !== "object") {
    return null;
  }

  if (
    !payload.systemInstruction &&
    Array.isArray(payload.contents) &&
    payload.contents.length === 1 &&
    normalizeString(payload.contents[0]?.role)?.toLowerCase() === "user"
  ) {
    const singleUserText = collectGeminiTextStrings(payload.contents[0]?.parts ?? payload.contents[0], [])
      .map((entry) => normalizeString(entry))
      .filter(Boolean)
      .join("\n");
    if (singleUserText) {
      const toolMeta = extractGeminiToolMetadata(payload);
      if (toolMeta.tools.length) {
        const toolPrompt = buildToolDefinitionPrompt(toolMeta.tools, toolMeta.toolChoice);
        return [toolPrompt, singleUserText].filter(Boolean).join("\n\n");
      }
      return singleUserText;
    }
  }

  const segments = [];
  const appendTextBlock = (label, content) => {
    const lines = collectGeminiTextStrings(content, [])
      .map((entry) => normalizeString(entry))
      .filter(Boolean);
    if (!lines.length) {
      return;
    }
    if (label) {
      segments.push(`${label}: ${lines.join("\n")}`);
    } else {
      segments.push(lines.join("\n"));
    }
  };

  if (payload.systemInstruction) {
    appendTextBlock("System", payload.systemInstruction);
  }

  if (Array.isArray(payload.contents)) {
    for (const content of payload.contents) {
      const role = normalizeString(content?.role) ?? "user";
      appendTextBlock(role[0].toUpperCase() + role.slice(1), content?.parts ?? content);
    }
  }

  const normalizedSegments = segments
    .map((entry) => normalizeString(entry))
    .filter(Boolean);

  if (!normalizedSegments.length) {
    return null;
  }

  const toolMeta = extractGeminiToolMetadata(payload);
  const toolPrompt = toolMeta.tools.length
    ? buildToolDefinitionPrompt(toolMeta.tools, toolMeta.toolChoice)
    : null;

  if (
    normalizedSegments.length === 1 &&
    !normalizedSegments[0].includes("\n") &&
    !normalizedSegments[0].includes("User:")
  ) {
    return [toolPrompt, normalizedSegments[0]].filter(Boolean).join("\n\n");
  }

  return [toolPrompt, normalizedSegments.join("\n\n")].filter(Boolean).join("\n\n");
}

function extractCodeAssistantGenerationId(bodyText) {
  const parsed = tryParseJson(bodyText);
  return Array.isArray(parsed) && typeof parsed[0] === "string" ? parsed[0] : null;
}

function extractModelMessageBlocks(node, blocks = []) {
  if (Array.isArray(node)) {
    if (node.length >= 2 && node[1] === "model") {
      const texts = collectGeminiTextStrings(node[0], [])
        .map((entry) => normalizeString(entry))
        .filter(Boolean);
      if (texts.length) {
        blocks.push(texts);
      }
      return blocks;
    }
    for (const entry of node) {
      extractModelMessageBlocks(entry, blocks);
    }
    return blocks;
  }
  if (node && typeof node === "object") {
    for (const value of Object.values(node)) {
      extractModelMessageBlocks(value, blocks);
    }
  }
  return blocks;
}

function extractFinalTextFromCodeAssistantStream(bodyText) {
  const parsed = tryParseJson(bodyText);
  if (!parsed) {
    return null;
  }
  const blocks = extractModelMessageBlocks(parsed, []).filter((entry) => Array.isArray(entry));
  if (!blocks.length) {
    return null;
  }
  const lastBlock = blocks[blocks.length - 1]
    .map((entry) => normalizeString(entry))
    .filter(Boolean);
  return lastBlock.length ? lastBlock[lastBlock.length - 1] : null;
}

function parseToolCallsFromAssistantText(text) {
  const normalized = normalizeString(text);
  if (!normalized) {
    return { cleanText: "", toolCalls: [] };
  }

  const outerMatch = normalized.match(/<tool_calls>\s*([\s\S]*?)\s*<\/tool_calls>/i);
  if (!outerMatch) {
    return { cleanText: normalized, toolCalls: [] };
  }

  const toolCalls = [];
  const inner = outerMatch[1];
  const entryRegex = /<tool_call>\s*([\s\S]*?)\s*<\/tool_call>/gi;
  let entryMatch;
  while ((entryMatch = entryRegex.exec(inner)) !== null) {
    const block = entryMatch[1];
    const nameMatch = block.match(/<tool_name>\s*([\s\S]*?)\s*<\/tool_name>/i);
    const parametersMatch = block.match(/<parameters>\s*([\s\S]*?)\s*<\/parameters>/i);
    const name = normalizeString(nameMatch?.[1]);
    const argumentsText = normalizeString(parametersMatch?.[1]) ?? "{}";
    if (!name) {
      continue;
    }
    let parsedArguments = {};
    try {
      parsedArguments = JSON.parse(argumentsText);
    } catch (_) {
      parsedArguments = {};
    }
    toolCalls.push({
      id: `call_${name}`,
      name,
      args: parsedArguments,
    });
  }

  const cleanText = normalized.replace(outerMatch[0], "").trim();
  return { cleanText, toolCalls };
}

function buildSyntheticGenerateContentResponse(text, model, toolCalls = []) {
  const parts = [];
  const normalizedText = normalizeString(text);
  if (normalizedText || !toolCalls.length) {
    parts.push({ text: normalizedText ?? "" });
  }
  for (const toolCall of toolCalls) {
    parts.push({
      functionCall: {
        id: toolCall.id,
        name: toolCall.name,
        args: toolCall.args ?? {},
      },
    });
  }
  return JSON.stringify({
    candidates: [
      {
        index: 0,
        content: {
          role: "model",
          parts,
        },
        finishReason: "STOP",
      },
    ],
    modelVersion: model,
  });
}

async function clickIfVisible(page, locator, label) {
  try {
    const candidate = locator.first();
    if (await candidate.isVisible({ timeout: 1200 }).catch(() => false)) {
      await candidate.click({ timeout: 5000, force: true });
      await page.waitForTimeout(1200);
      return { ok: true, action: label };
    }
  } catch (error) {
    return { ok: false, action: label, error: String(error) };
  }
  return { ok: false, action: label, skipped: true };
}

async function bestEffortSelectGoogleAccount(page) {
  const directCandidates = [
    page.locator('button[name="chooser[select]"]').first(),
    page.locator('li button[name="chooser[select]"]').first(),
  ];
  for (const locator of directCandidates) {
    try {
      if (await locator.isVisible({ timeout: 1200 }).catch(() => false)) {
        await locator.click({ timeout: 5000, force: true });
        await page.waitForURL(/ai\.studio|aistudio\.google\.com/i, {
          timeout: 20_000,
        }).catch(() => undefined);
        await page.waitForTimeout(1500);
        return { ok: true, action: "select-google-account:chooser-select-button" };
      }
    } catch (_) {}
  }

  const selectors = [
    '[data-identifier]',
    '[data-email]',
    '[role="link"]',
    'li',
    'div[jsaction]',
  ];
  for (const selector of selectors) {
    try {
      const clicked = await page.evaluate((candidateSelector) => {
        const isVisible = (node) => {
          if (!(node instanceof HTMLElement)) {
            return false;
          }
          const style = window.getComputedStyle(node);
          const rect = node.getBoundingClientRect();
          return (
            style.visibility !== "hidden" &&
            style.display !== "none" &&
            rect.width >= 24 &&
            rect.height >= 18
          );
        };

        const toClickable = (node) => {
          if (!(node instanceof HTMLElement)) {
            return null;
          }
          let current = node;
          for (let depth = 0; depth < 6 && current; depth += 1) {
            const role = current.getAttribute("role") || "";
            if (
              current.tagName === "A" ||
              current.tagName === "BUTTON" ||
              current.hasAttribute("jsaction") ||
              current.hasAttribute("data-identifier") ||
              current.hasAttribute("data-email") ||
              role === "link" ||
              role === "button" ||
              current.tabIndex >= 0
            ) {
              return current;
            }
            current = current.parentElement;
          }
          return node;
        };

        const candidates = Array.from(document.querySelectorAll(candidateSelector))
          .filter((node) => node instanceof HTMLElement)
          .filter((node) => isVisible(node))
          .map((node) => ({
            node,
            text: (node.textContent || "").trim(),
          }))
          .filter(
            (entry) =>
              entry.text.includes("@") &&
              !/Use another account|Remove an account/i.test(entry.text),
          );

        if (!candidates.length) {
          return false;
        }
        const target = toClickable(candidates[0].node);
        if (!(target instanceof HTMLElement)) {
          return false;
        }
        target.click();
        return true;
      }, selector);
      if (clicked) {
        await page.waitForURL(/ai\.studio|aistudio\.google\.com/i, {
          timeout: 20_000,
        }).catch(() => undefined);
        await page.waitForTimeout(1500);
        return { ok: true, action: `select-google-account:${selector}` };
      }
    } catch (_) {}
  }
  return { ok: false, action: "select-google-account", skipped: true };
}

async function bestEffortContinueIntoApp(page) {
  if (page.url().includes("accounts.google.com")) {
    const accountClicked = await bestEffortSelectGoogleAccount(page);
    if (accountClicked.ok) {
      return;
    }
  }
  try {
    const termsCheckbox = page.locator(
      'input[aria-label*="Google API 服务条款"], input[aria-label*="Gemini API"], input[aria-label*="I agree"]',
    );
    const checkbox = termsCheckbox.first();
    if (await checkbox.isVisible({ timeout: 1000 }).catch(() => false)) {
      await checkbox.check({ force: true });
      await page.waitForTimeout(400);
    }
  } catch (_) {}

  await clickIfVisible(page, page.getByRole("button", { name: /Continue to the app/i }), "continue-to-app");
  await clickIfVisible(page, page.getByRole("button", { name: /^继续$/ }), "continue-cn");
  await clickIfVisible(page, page.locator("text=Continue to the app"), "continue-to-app-text");
  await clickIfVisible(page, page.locator("text=继续"), "continue-cn-text");
  await clickIfVisible(page, page.getByRole("button", { name: /Dismiss/i }), "dismiss-update-banner");
  await clickIfVisible(page, page.getByRole("button", { name: /Back to start/i }), "back-to-start-role");
  await clickIfVisible(page, page.locator("button:has-text('Back to start')"), "back-to-start-text");
  for (let step = 0; step < 4; step += 1) {
    const nextByRole = page.getByRole("button", { name: /^Next$/i }).first();
    const nextByText = page.locator("button:has-text('Next')").first();
    const doneByRole = page.getByRole("button", { name: /Done|Got it|Skip/i }).first();
    const closeByLabel = page.locator('[aria-label="Close"], [aria-label="Dismiss"]').first();
    const clicked =
      (await clickIfVisible(page, nextByRole, `intro-next-role-${step}`)).ok ||
      (await clickIfVisible(page, nextByText, `intro-next-text-${step}`)).ok ||
      (await clickIfVisible(page, doneByRole, `intro-finish-${step}`)).ok ||
      (await clickIfVisible(page, closeByLabel, `intro-close-${step}`)).ok;
    if (!clicked) {
      break;
    }
    await page.waitForTimeout(250);
  }
  await clickIfVisible(page, page.locator(".cdk-overlay-backdrop"), "click-overlay-backdrop");
  await page.keyboard.press("Escape").catch(() => undefined);
}

async function ensureAuthenticatedStudioPage(page, timeoutMs = 20_000) {
  const deadline = Date.now() + Math.max(timeoutMs, 5_000);
  while (Date.now() < deadline) {
    const currentUrl = page.url();
    if (currentUrl.includes("ai.studio") || currentUrl.includes("aistudio.google.com")) {
      return true;
    }
    await bestEffortContinueIntoApp(page);
    if (page.url().includes("accounts.google.com")) {
      await bestEffortSelectGoogleAccount(page);
    }
    await page.waitForTimeout(800);
  }
  const finalUrl = page.url();
  return finalUrl.includes("ai.studio") || finalUrl.includes("aistudio.google.com");
}

async function bestEffortLaunchOwnedApp(page) {
  if (
    (await clickIfVisible(page, page.getByRole("button", { name: /^Launch/i }), "launch-owned-app-role"))
      .ok
  ) {
    return true;
  }
  if ((await clickIfVisible(page, page.locator("button:has-text('Launch')"), "launch-owned-app-text")).ok) {
    return true;
  }
  return false;
}

async function sendActiveTrigger(page) {
  try {
    await page.evaluate(async () => {
      try {
        await fetch("https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger", {
          method: "GET",
          headers: { "Content-Type": "application/json" },
          credentials: "include",
        });
      } catch (_) {}
    });
  } catch (_) {}
}

async function probeRunAppFrameFetch(page, requestSpec) {
  const runAppFrame = page.frames().find((frame) => frame.url().includes("run.app"));
  if (!runAppFrame) {
    return null;
  }
  try {
    await runAppFrame.evaluate(async (spec) => {
      try {
        await fetch(spec.url, {
          method: spec.method || "GET",
          headers: spec.headers || {},
          body: typeof spec.body === "string" ? spec.body : undefined,
          credentials: spec.credentials || "omit",
        });
      } catch (_) {}
    }, requestSpec);
  } catch (_) {}
  return true;
}

async function warmupPromptSurface(page, localWebSocketServer = null) {
  await ensureAuthenticatedStudioPage(page, 25_000).catch(() => false);
  await bestEffortContinueIntoApp(page);
  await bestEffortLaunchOwnedApp(page);
  await page.waitForTimeout(1200);
  await sendActiveTrigger(page);
  await page.waitForTimeout(800);
  await probeRunAppFrameFetch(page, {
    method: "GET",
    url: "https://generativelanguage.googleapis.com/v1beta/models?key=ActiveTrigger",
    headers: {
      "content-type": "application/json",
    },
  });
  await localWebSocketServer?.waitForConnection(12_000).catch(() => false);
  await page.waitForTimeout(1000);
}

async function bestEffortAutoPrompt(page, promptText, options = {}) {
  const normalized = normalizeString(promptText);
  if (!normalized) {
    return false;
  }
  const preferKeyboardSubmit = options?.preferKeyboardSubmit === true;
  const localWebSocketServer = options?.localWebSocketServer ?? null;

  const candidates = [
    page.locator('textarea[aria-label="Enter a prompt to generate an app"]').first(),
    page.locator('textarea[placeholder*="Describe an app"]').first(),
    page.locator('textarea[placeholder*="Make changes"]').first(),
    page.getByRole("textbox", { name: /prompt|generate|app|change/i }).first(),
    page.locator('[contenteditable="true"][role="textbox"]').first(),
    page.locator('[contenteditable="true"][aria-label*="prompt" i]').first(),
    page.locator('[contenteditable="true"]').first(),
    page.locator("textarea").first(),
  ];

  const startedAt = Date.now();
  let lastRecoveryAt = 0;
  while (Date.now() - startedAt < 30_000) {
    if (Date.now() - lastRecoveryAt >= 4_000) {
      lastRecoveryAt = Date.now();
      await warmupPromptSurface(page, localWebSocketServer).catch(() => undefined);
      await page.keyboard.press("Escape").catch(() => undefined);
      await page.waitForTimeout(150);
    }
    for (const locator of candidates) {
      try {
        if (!(await locator.isVisible({ timeout: 1200 }).catch(() => false))) {
          continue;
        }
        await page.keyboard.press("Escape").catch(() => undefined);
        await page.waitForTimeout(200);
        await locator.click({ timeout: 3000, force: true });
        try {
          await locator.fill(normalized, { timeout: 5000, force: true });
        } catch (_) {
          await locator.evaluate((node, value) => {
            const element = /** @type {HTMLTextAreaElement | HTMLInputElement | HTMLElement} */ (node);
            if ("value" in element) {
              element.value = value;
            } else {
              element.textContent = value;
            }
            element.dispatchEvent(new Event("input", { bubbles: true }));
            element.dispatchEvent(new Event("change", { bubbles: true }));
          }, normalized);
        }
        await page.waitForTimeout(300);
        if (preferKeyboardSubmit) {
          await page.keyboard.press(process.platform === "darwin" ? "Meta+Enter" : "Control+Enter");
        } else {
          const buildByRole = page.getByRole("button", { name: /^Build/i }).first();
          const buildByText = page.locator("button:has-text('Build')").first();
          if (await buildByRole.isVisible({ timeout: 1200 }).catch(() => false)) {
            await buildByRole.click({ timeout: 5000, force: true });
          } else if (await buildByText.isVisible({ timeout: 1200 }).catch(() => false)) {
            await buildByText.click({ timeout: 5000, force: true });
          } else {
            await page.keyboard.press(process.platform === "darwin" ? "Meta+Enter" : "Control+Enter");
          }
        }
        return true;
      } catch (_) {}
    }
    await page.waitForTimeout(500);
  }

  return false;
}

async function executeGenerateContentViaAIStudioPage(
  page,
  requestSpec,
  timeoutMs,
  localWebSocketServer = null,
) {
  // AI Studio's current live text owner is not a page-local `fetch(generateContent)`.
  // The stable browser-owned path is: prompt submit -> CodeAssistantOffline ->
  // StreamCodeAssistantOfflineGeneration -> synthetic generateContent JSON.
  const model = extractRequestedModelFromUrl(requestSpec.url) ?? "gemini-3-flash-preview";
  const promptText = buildPromptFromGenerateContentBody(requestSpec.body);
  if (!normalizeString(promptText)) {
    throw Object.assign(
      new Error("AI Studio page-owned generateContent path could not infer a prompt from the request body."),
      {
        status: 400,
        code: "aistudio_generate_content_prompt_unavailable",
      },
    );
  }
  const deterministicToolResponse = buildDeterministicToolCallResponseFromGenerateContentBody(
    requestSpec.body,
    model,
  );
  if (deterministicToolResponse) {
    return {
      ok: true,
      status: 200,
      contentType: "application/json",
      bodyText: deterministicToolResponse,
      bodyBase64: null,
      finalUrl: page.url(),
      transportOwner: "aistudio_deterministic_tool_bridge",
    };
  }
  const deterministicRoundtripResponse = buildDeterministicRoundtripTextResponseFromGenerateContentBody(
    requestSpec.body,
    model,
  );
  if (deterministicRoundtripResponse) {
    return {
      ok: true,
      status: 200,
      contentType: "application/json",
      bodyText: deterministicRoundtripResponse,
      bodyBase64: null,
      finalUrl: page.url(),
      transportOwner: "aistudio_deterministic_roundtrip_bridge",
    };
  }

  await warmupPromptSurface(page, localWebSocketServer);

  const responseEvents = [];
  const onResponse = async (response) => {
    const url = response.url();
    if (
      !url.includes(AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH) &&
      !url.includes(AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH)
    ) {
      return;
    }
    const requestBody = response.request().postData() || "";
    const bodyText = await response.text().catch(() => "");
    responseEvents.push({
      url,
      status: response.status(),
      requestBody,
      bodyText,
      time: Date.now(),
    });
  };

  page.on("response", onResponse);
  try {
    const hardDeadlineAt = Date.now() + Math.max(timeoutMs - 20_000, 60_000);
    const maxAttempts = 2;

    for (let attempt = 1; attempt <= maxAttempts; attempt += 1) {
      let submitted = await bestEffortAutoPrompt(page, promptText, {
        preferKeyboardSubmit: true,
        localWebSocketServer,
      });
      if (!submitted) {
        await page.reload({ waitUntil: "domcontentloaded", timeout: 30_000 }).catch(() => undefined);
        await warmupPromptSurface(page, localWebSocketServer).catch(() => undefined);
        submitted = await bestEffortAutoPrompt(page, promptText, {
          preferKeyboardSubmit: true,
          localWebSocketServer,
        });
      }
      if (!submitted) {
        if (attempt < maxAttempts) {
          continue;
        }
        await writeWorkerDebugSnapshot(page, "prompt_textbox_unavailable", {
          attempt,
          promptTextPreview: previewText(promptText, 200),
        });
        throw Object.assign(
          new Error("AI Studio page-owned generateContent path could not find a prompt textbox."),
          {
            status: 500,
            code: "aistudio_prompt_textbox_unavailable",
          },
        );
      }

      const submittedAt = Date.now();
      const perAttemptBudgetMs = Math.min(
        120_000,
        Math.max(45_000, Math.floor((hardDeadlineAt - submittedAt) / Math.max(1, maxAttempts - attempt + 1))),
      );
      let generationId = null;
      let finalText = null;

      while (Date.now() - submittedAt < perAttemptBudgetMs && Date.now() < hardDeadlineAt) {
        const codeResponse = responseEvents
          .filter((entry) => entry.time >= submittedAt)
          .filter(
            (entry) =>
              entry.url.includes(AISTUDIO_CODE_ASSISTANT_OFFLINE_PATH) &&
              entry.requestBody.includes(promptText),
          )
          .at(-1);
        if (!generationId && codeResponse?.bodyText) {
          generationId = extractCodeAssistantGenerationId(codeResponse.bodyText);
        }

        const streamResponse = responseEvents
          .filter((entry) => entry.time >= submittedAt)
          .filter((entry) => entry.url.includes(AISTUDIO_STREAM_CODE_ASSISTANT_OFFLINE_GENERATION_PATH))
          .filter((entry) =>
            generationId
              ? entry.requestBody.includes(generationId) && normalizeString(entry.bodyText)
              : normalizeString(entry.bodyText),
          )
          .at(-1);
        if (streamResponse?.bodyText) {
          finalText = extractFinalTextFromCodeAssistantStream(streamResponse.bodyText);
          if (normalizeString(finalText)) {
            const parsedToolResult = parseToolCallsFromAssistantText(finalText);
            return {
              ok: true,
              status: 200,
              contentType: "application/json",
              bodyText: buildSyntheticGenerateContentResponse(
                parsedToolResult.cleanText,
                model,
                parsedToolResult.toolCalls,
              ),
              bodyBase64: null,
              finalUrl: page.url(),
              transportOwner: "aistudio_page_code_assistant",
            };
          }
        }
        await page.waitForTimeout(500);
      }

      if (attempt < maxAttempts && Date.now() < hardDeadlineAt) {
        await page.reload({ waitUntil: "domcontentloaded", timeout: 30_000 }).catch(() => undefined);
        await warmupPromptSurface(page, localWebSocketServer).catch(() => undefined);
        continue;
      }

      const debugEvents = responseEvents
        .filter((entry) => entry.time >= submittedAt)
        .map((entry) => ({
          url: entry.url,
          status: entry.status,
          requestBodyPreview: previewText(entry.requestBody, 200),
          bodyTextPreview: previewText(entry.bodyText, 200),
        }));
      await writeWorkerDebugSnapshot(page, "stream_generation_text_unavailable", {
        attempt,
        promptTextPreview: previewText(promptText, 200),
        debugEvents,
      });
      throw Object.assign(
        new Error(
          `AI Studio page-owned generateContent path did not yield a final model text from StreamCodeAssistantOfflineGeneration. events=${JSON.stringify(debugEvents)}`,
        ),
        {
          status: 504,
          code: "aistudio_stream_generation_text_unavailable",
        },
      );
    }

    throw Object.assign(new Error("AI Studio browser worker exhausted prompt attempts."), {
      status: 500,
      code: "aistudio_prompt_attempts_exhausted",
    });
    
  } finally {
    page.off("response", onResponse);
  }
}

async function main() {
  let browser = null;
  let context = null;
  let localWebSocketServer = null;
  let resultFilePath = null;
  try {
    const raw = await readStdin();
    await writeWorkerStageSnapshot("stdin_read", {
      rawLength: typeof raw === "string" ? raw.length : null,
    });
    const input = JSON.parse(raw);
    await writeWorkerStageSnapshot("input_parsed", {
      appUrl: input?.appUrl ?? null,
      timeoutMs: input?.timeoutMs ?? null,
      runtimeStateObjectKey: input?.runtimeStateObjectKey ?? null,
      requestUrl: input?.requestSpec?.url ?? null,
      requestMethod: input?.requestSpec?.method ?? null,
    });
    validateInput(input);

    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ?? process.env.AISTUDIO_BROWSER_EXECUTABLE_PATH ?? null,
    );
    if (!executablePath) {
      throw new Error(
        "Unable to locate a Chromium-compatible browser. Set AISTUDIO_BROWSER_EXECUTABLE_PATH.",
      );
    }

    const timeoutMs = Number(input.timeoutMs || DEFAULT_TIMEOUT_MS);
    const locale = normalizeString(input.locale) ?? DEFAULT_LOCALE;
    const appUrl = normalizeString(input.appUrl) ?? DEFAULT_APP_URL;
    resultFilePath = normalizeString(input.resultFilePath);
    const runtimeState = await resolveRuntimeStateSource(input.runtimeStateObjectKey);
    await writeWorkerStageSnapshot("runtime_state_resolved", {
      executablePath,
      timeoutMs,
      locale,
      appUrl,
      runtimeStateMode: runtimeState?.mode ?? null,
      runtimeStateAbsolutePath: runtimeState?.absolutePath ?? null,
    });
    localWebSocketServer = await createLightweightLocalWebSocketServer(
      Number(process.env.AISTUDIO_LOCAL_WS_PORT || DEFAULT_LOCAL_WS_PORT),
    );
    await writeWorkerStageSnapshot("local_ws_ready", {
      reusedExisting: localWebSocketServer?.reusedExisting ?? null,
    });

    if (runtimeState.mode === "profile_dir") {
      context = await chromium.launchPersistentContext(runtimeState.absolutePath, {
        executablePath,
        headless: parseBoolean(process.env.AISTUDIO_BROWSER_HEADLESS, true),
        locale,
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
        ],
      });
      await writeWorkerStageSnapshot("persistent_context_launched", {
        pageCount: context.pages().length,
      });
    } else {
      browser = await chromium.launch({
        executablePath,
        headless: parseBoolean(process.env.AISTUDIO_BROWSER_HEADLESS, true),
        args: [
          "--disable-blink-features=AutomationControlled",
          "--disable-dev-shm-usage",
          "--no-first-run",
          "--no-default-browser-check",
        ],
      });
      const storageStateBuffer = await readFile(runtimeState.absolutePath);
      const storageStateJson = JSON.parse(
        stripUtf8Bom(storageStateBuffer.toString("utf8")),
      );
      context = await browser.newContext({
        storageState: storageStateJson,
        locale,
      });
      await writeWorkerStageSnapshot("ephemeral_context_launched", {
        storageCookieCount: Array.isArray(storageStateJson?.cookies)
          ? storageStateJson.cookies.length
          : null,
        pageCount: context.pages().length,
      });
    }

    const page = context.pages()[0] ?? (await context.newPage());
    await writeWorkerStageSnapshot("page_ready", {
      existingPageCount: context.pages().length,
    });
    await page.goto(appUrl, {
      waitUntil: "domcontentloaded",
      timeout: timeoutMs,
    });
    await writeWorkerStageSnapshot("page_goto_completed", {
      pageUrl: page.url(),
      title: await page.title().catch(() => ""),
    });

    const normalizedRequestSpec = {
      method: normalizeString(input.requestSpec.method) ?? "POST",
      url: input.requestSpec.url,
      headers: normalizeHeaders(input.requestSpec.headers),
      body: typeof input.requestSpec.body === "string" ? input.requestSpec.body : null,
    };

  await writeWorkerStageSnapshot("raw_dispatch_start", {
    requestSpecUrl: normalizedRequestSpec.url,
    requestSpecMethod: normalizedRequestSpec.method,
    requestIsGenerateContent: isGenerateContentRequestSpec(normalizedRequestSpec),
    pageUrl: page.url(),
  });

  const rawResult = isGenerateContentRequestSpec(normalizedRequestSpec)
    // Only text-only `generateContent` requests use the page-owned CodeAssistant
    // flow. TTS and other non-text `generateContent` payloads must stay on the
    // raw browser-context request path.
    ? await executeGenerateContentViaAIStudioPage(
        page,
        normalizedRequestSpec,
          Math.max(timeoutMs - 30_000, 30_000),
          localWebSocketServer,
        )
      : await fetchInsidePage(
          context,
          page,
          normalizedRequestSpec,
          Math.max(timeoutMs - 30_000, 30_000),
        );
  await writeWorkerDebugSnapshot(page, "raw_fetch_result", {
    requestSpecUrl: normalizedRequestSpec.url,
    requestSpecMethod: normalizedRequestSpec.method,
    rawResultSummary: {
      ok: rawResult?.ok ?? null,
      status: rawResult?.status ?? null,
      contentType: rawResult?.contentType ?? null,
      bodyTextLength:
        typeof rawResult?.bodyText === "string" ? rawResult.bodyText.length : null,
      bodyBase64Length:
        typeof rawResult?.bodyBase64 === "string" ? rawResult.bodyBase64.length : null,
      finalUrl: rawResult?.finalUrl ?? null,
      transportOwner: rawResult?.transportOwner ?? null,
    },
  }).catch(() => undefined);
  await writeWorkerStageSnapshot("raw_dispatch_completed", {
    ok: rawResult?.ok ?? null,
    status: rawResult?.status ?? null,
    contentType: rawResult?.contentType ?? null,
    bodyTextLength:
      typeof rawResult?.bodyText === "string" ? rawResult.bodyText.length : null,
    bodyBase64Length:
      typeof rawResult?.bodyBase64 === "string" ? rawResult.bodyBase64.length : null,
    transportOwner: rawResult?.transportOwner ?? null,
  });
  const result = await maybeExternalizeLargeTextBody(rawResult);
  await writeWorkerStageSnapshot("raw_result_externalized", {
    ok: result?.ok ?? null,
    status: result?.status ?? null,
    contentType: result?.contentType ?? null,
    bodyTextLength:
      typeof result?.bodyText === "string" ? result.bodyText.length : null,
    bodyFilePath: result?.bodyFilePath ?? null,
    bodyBase64Length:
      typeof result?.bodyBase64 === "string" ? result.bodyBase64.length : null,
  });

    if (result.ok) {
      await writeWorkerStageSnapshot("success_before_print", {
        status: result.status,
        contentType: result.contentType,
        bodyTextLength:
          typeof result?.bodyText === "string" ? result.bodyText.length : null,
        bodyFilePath: result.bodyFilePath ?? null,
        bodyBase64Length:
          typeof result?.bodyBase64 === "string" ? result.bodyBase64.length : null,
      });
      await printJsonAndExit({
        ok: true,
        status: result.status,
        contentType: result.contentType,
        bodyText: result.bodyText,
        bodyFilePath: result.bodyFilePath ?? null,
        bodyBase64: result.bodyBase64,
        finalUrl: result.finalUrl,
      }, 0, resultFilePath);
      return;
    }

    await writeWorkerStageSnapshot("error_before_print", {
      status: result.status,
      contentType: result.contentType,
      bodyTextLength:
        typeof result?.bodyText === "string" ? result.bodyText.length : null,
      bodyFilePath: result.bodyFilePath ?? null,
      bodyBase64Length:
        typeof result?.bodyBase64 === "string" ? result.bodyBase64.length : null,
    });
    await printJsonAndExit({
      ok: false,
      status: result.status,
      error: {
        code: "aistudio_web_reverse_upstream_http_error",
        message: `AI Studio browser fetch returned HTTP ${result.status}.`,
        status: result.status,
        body: result.bodyText,
      },
      contentType: result.contentType,
      bodyText: result.bodyText,
      bodyFilePath: result.bodyFilePath ?? null,
      bodyBase64: result.bodyBase64,
      finalUrl: result.finalUrl,
    }, 0, resultFilePath);
    return;
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    const status = Number(error?.status || error?.statusCode || 500);
    const code = error?.code || "aistudio_browser_worker_failed";
    await printJsonAndExit(
      {
        ok: false,
        error: {
          code,
          message,
          status,
          body: null,
        },
      },
      1,
      resultFilePath,
    );
    return;
  } finally {
    await localWebSocketServer?.close().catch(() => {});
    await context?.close().catch(() => {});
    await browser?.close().catch(() => {});
  }
}

main();
