import fs from "node:fs/promises";
import fsSync from "node:fs";
import https from "node:https";
import path from "node:path";
import { WebSocketServer } from "ws";
import {
  resolveGeminiCanvasManualLiveVendorProfileObjectKey,
  resolveGeminiCanvasStorageRoot,
} from "./gemini-canvas-runtime-paths.mjs";

const DEFAULT_BROWSER_POOL_BASE_URL = process.env.GEMINI_CANVAS_BROWSER_POOL_BASE_URL?.trim() || "http://127.0.0.1:42321";
const DEFAULT_PROGRAM_PAGE_URL = process.env.GEMINI_CANVAS_PROGRAM_PAGE_URL?.trim() || "https://gemini.google.com/canvas";
const DEFAULT_PROGRAM_URL = process.env.GEMINI_CANVAS_PROGRAM_URL?.trim() || "https://gemini.google.com/app/4abc4e7577b6149f";
const DEFAULT_APP_PATH = process.env.GEMINI_CANVAS_PROGRAM_APP_PATH?.trim() || "/app/4abc4e7577b6149f";
const DEFAULT_RUNTIME_STATE_OBJECT_KEY =
  process.env.GEMINI_CANVAS_PROGRAM_RUNTIME_STATE_OBJECT_KEY?.trim()
  || resolveGeminiCanvasManualLiveVendorProfileObjectKey(resolveGeminiCanvasStorageRoot());
const DEFAULT_SHARE_ID = process.env.GEMINI_CANVAS_PROGRAM_SHARE_ID?.trim() || "fe24c455a570";
const DEFAULT_MODEL = process.env.GEMINI_CANVAS_PROGRAM_NO_KEY_MODEL?.trim() || "gemini-3-flash-preview";
const DEFAULT_HOST = "127.0.0.1";
const DEFAULT_PORT = 9998;
const DEFAULT_TLS_DIR = path.resolve(
  process.cwd(),
  "gateway",
  ".runtime",
  "gemini-canvas-browser-pool-tls",
);

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function timestampId() {
  return new Date().toISOString().replace(/[:.]/g, "-");
}

function compact(value, limit = 800) {
  const text = typeof value === "string" ? value : JSON.stringify(value);
  return text.length > limit ? `${text.slice(0, limit)}...[truncated]` : text;
}

async function ensureDir(dir) {
  await fs.mkdir(dir, { recursive: true });
}

async function postJson(url, body, timeoutMs = 120000) {
  const controller = new AbortController();
  const timer = setTimeout(() => controller.abort(), timeoutMs);
  try {
    const response = await fetch(url, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(body),
      signal: controller.signal,
    });
    const text = await response.text();
    return {
      status: response.status,
      ok: response.ok,
      bodyText: text,
      json: (() => {
        try {
          return JSON.parse(text);
        } catch {
          return null;
        }
      })(),
    };
  } finally {
    clearTimeout(timer);
  }
}

async function run() {
  const outDir = path.resolve(
    process.cwd(),
    "output",
    `gemini_canvas_program_app_ws_no_key_${timestampId()}`,
  );
  await ensureDir(outDir);

  const summary = {
    ok: false,
    browserPoolBaseUrl: DEFAULT_BROWSER_POOL_BASE_URL,
    runtimeStateObjectKey: DEFAULT_RUNTIME_STATE_OBJECT_KEY,
    shareId: DEFAULT_SHARE_ID,
    pageUrl: DEFAULT_PROGRAM_PAGE_URL,
    canvasProgramUrl: DEFAULT_PROGRAM_URL,
    appPath: DEFAULT_APP_PATH,
    model: DEFAULT_MODEL,
    wsEndpoint: `wss://${DEFAULT_HOST}:${DEFAULT_PORT}`,
    bootstrap: null,
    connection: null,
    requestSpec: null,
    responseHeaders: null,
    bodyText: null,
    error: null,
    wsEvents: [],
    wsClose: null,
  };

  let requestResolve;
  let requestReject;
  const requestPromise = new Promise((resolve, reject) => {
    requestResolve = resolve;
    requestReject = reject;
  });

  const certPath = path.join(DEFAULT_TLS_DIR, "127.0.0.1.cert.pem");
  const keyPath = path.join(DEFAULT_TLS_DIR, "127.0.0.1.key.pem");
  const httpsServer = https.createServer({
    cert: fsSync.readFileSync(certPath),
    key: fsSync.readFileSync(keyPath),
  });
  const wss = new WebSocketServer({ server: httpsServer });

  await new Promise((resolve, reject) => {
    httpsServer.once("error", reject);
    httpsServer.listen(DEFAULT_PORT, DEFAULT_HOST, resolve);
  });

  wss.on("connection", (ws, req) => {
    summary.connection = {
      remoteAddress: req.socket.remoteAddress,
      url: req.url ?? null,
      connectedAt: new Date().toISOString(),
    };

    const requestId = `canvas_no_key_${Date.now()}`;
    const requestSpec = {
      event_type: "proxy_request",
      request_id: requestId,
      request_attempt_id: `${requestId}_attempt_1`,
      request_attempt_number: 1,
      streaming_mode: "fake",
      method: "POST",
      path: `v1beta/models/${DEFAULT_MODEL}:generateContent`,
      url: `https://generativelanguage.googleapis.com/v1beta/models/${DEFAULT_MODEL}:generateContent`,
      headers: {
        "content-type": "application/json",
      },
      body: JSON.stringify({
        contents: [
          {
            role: "user",
            parts: [{ text: "Reply with exactly: ok" }],
          },
        ],
      }),
    };
    summary.requestSpec = requestSpec;
    ws.send(JSON.stringify(requestSpec));

    const chunks = [];
    let headers = null;
    const pushWsEvent = (event) => {
      summary.wsEvents.push({
        t: new Date().toISOString(),
        ...event,
      });
      if (summary.wsEvents.length > 80) {
        summary.wsEvents.shift();
      }
    };

    ws.on("message", (raw) => {
      let payload;
      try {
        payload = JSON.parse(String(raw));
      } catch (error) {
        requestReject(new Error(`invalid_ws_payload: ${error.message}`));
        return;
      }
      if (payload.request_id !== requestId) {
        return;
      }
      pushWsEvent({
        event_type: payload.event_type ?? "unknown",
        status: payload.status ?? null,
        message: payload.message ?? null,
        chunkLength: typeof payload.data === "string" ? payload.data.length : null,
      });
      switch (payload.event_type) {
        case "response_headers":
          headers = payload;
          summary.responseHeaders = payload;
          break;
        case "chunk":
          if (typeof payload.data === "string") {
            chunks.push(payload.data);
          }
          break;
        case "stream_close":
          summary.bodyText = chunks.join("");
          requestResolve({
            status: headers?.status ?? 200,
            headers,
            bodyText: summary.bodyText,
          });
          break;
        case "error":
          summary.error = {
            status: payload.status ?? 500,
            message: payload.message ?? null,
          };
          requestResolve({
            status: payload.status ?? 500,
            headers,
            bodyText: payload.message ?? "",
            error: summary.error,
          });
          break;
        default:
          break;
      }
    });

    ws.on("error", (error) => {
      pushWsEvent({
        event_type: "ws_error",
        message: error instanceof Error ? error.message : String(error),
      });
      requestReject(error);
    });
    ws.on("close", (code, reasonBuffer) => {
      const reason = Buffer.isBuffer(reasonBuffer)
        ? reasonBuffer.toString("utf8")
        : String(reasonBuffer || "");
      summary.wsClose = {
        code,
        reason: reason || null,
      };
      pushWsEvent({
        event_type: "ws_close",
        status: code,
        message: reason || null,
      });
    });
  });

  try {
    const bootstrapBody = {
      operation: "bootstrap_program",
      runtimeStateObjectKey: DEFAULT_RUNTIME_STATE_OBJECT_KEY,
      shareId: DEFAULT_SHARE_ID,
      baseUrl: "https://gemini.google.com",
      canvasProgramUrl: DEFAULT_PROGRAM_URL,
      appPath: DEFAULT_APP_PATH,
      pageUrl: DEFAULT_PROGRAM_PAGE_URL,
      discoveryOnly: true,
      launchCanvasProxyPreview: true,
      bootstrapOperation: "text",
    };
    summary.bootstrap = await postJson(`${DEFAULT_BROWSER_POOL_BASE_URL}/invoke`, bootstrapBody, 180000);
    await fs.writeFile(
      path.join(outDir, "bootstrap.json"),
      JSON.stringify(summary.bootstrap, null, 2),
      "utf8",
    );

    const result = await Promise.race([
      requestPromise,
      new Promise((_, reject) =>
        setTimeout(
          () => reject(new Error("canvas_app_ws_no_key_timeout")),
          120000,
        ),
      ),
    ]);

    summary.ok = !result.error && result.status >= 200 && result.status < 300;
    summary.finalStatus = result.status;
    summary.bodyText = result.bodyText;
    if (!summary.error && result.error) {
      summary.error = result.error;
    }
  } catch (error) {
    summary.ok = false;
    summary.error = {
      message: error instanceof Error ? error.message : String(error),
    };
  } finally {
    await new Promise((resolve) => wss.close(resolve));
    await new Promise((resolve) => httpsServer.close(resolve));
  }

  if (summary.bodyText) {
    await fs.writeFile(
      path.join(outDir, "body.txt"),
      String(summary.bodyText),
      "utf8",
    );
  }
  await fs.writeFile(
    path.join(outDir, "summary.json"),
    JSON.stringify(summary, null, 2),
    "utf8",
  );
  console.log(JSON.stringify({
    ok: summary.ok,
    outDir,
    finalStatus: summary.finalStatus ?? null,
    error: summary.error ?? null,
    bodyPreview: summary.bodyText ? compact(summary.bodyText, 240) : null,
  }, null, 2));
}

run().catch((error) => {
  console.error(error);
  process.exit(1);
});
