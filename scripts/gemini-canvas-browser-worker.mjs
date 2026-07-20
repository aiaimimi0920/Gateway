import { chromium } from "playwright-core";
import { existsSync } from "node:fs";
import { readFile } from "node:fs/promises";
import path from "node:path";
import { S3Client, GetObjectCommand } from "@aws-sdk/client-s3";

const DEFAULT_TIMEOUT_MS = 180_000;
const DEFAULT_LOCALE = "en-US";
const DEFAULT_SHARE_ID = "fe24c455a570";

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
  if (!normalized) {
    return fallback;
  }
  if (["1", "true", "yes", "on"].includes(normalized)) {
    return true;
  }
  if (["0", "false", "no", "off"].includes(normalized)) {
    return false;
  }
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
    throw new Error("Gemini Canvas worker object storage is not fully configured.");
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

async function readRuntimeStateObject(objectKey) {
  const config = resolveObjectStorageConfig();
  if (config.driver === "local") {
    const absolutePath = path.join(getStorageRoot(), ...objectKey.split("/"));
    return readFile(absolutePath);
  }

  const client = getS3Client(config);
  const response = await client.send(
    new GetObjectCommand({
      Bucket: config.bucket,
      Key: objectKey,
    }),
  );
  return toBuffer(response.Body);
}

function validateInput(input) {
  if (!normalizeString(input?.runtimeStateObjectKey)) {
    throw new Error("runtimeStateObjectKey is required.");
  }
  if (!normalizeString(input?.requestUrl)) {
    throw new Error("requestUrl is required.");
  }
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

function printJsonAndExit(payload, exitCode = 0) {
  process.stdout.write(`${JSON.stringify(payload)}\n`);
  process.exit(exitCode);
}

async function main() {
  try {
    const raw = await readStdin();
    const input = JSON.parse(raw);
    validateInput(input);

    const executablePath = resolveExecutablePath(
      input.browserExecutablePath ?? process.env.GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH ?? null,
    );
    if (!executablePath) {
      throw new Error(
        "Unable to locate a Chromium-compatible browser. Set GEMINI_CANVAS_BROWSER_EXECUTABLE_PATH.",
      );
    }

    const timeoutMs = Number(input.timeoutMs || DEFAULT_TIMEOUT_MS);
    const baseUrl = normalizeString(input.baseUrl) ?? "https://gemini.google.com";
    const shareId = normalizeString(input.shareId) ?? DEFAULT_SHARE_ID;
    const shareUrl = `${baseUrl.replace(/\/+$/, "")}/share/${shareId}`;
    const locale = normalizeString(input.locale) ?? DEFAULT_LOCALE;
    const storageStateBuffer = await readRuntimeStateObject(input.runtimeStateObjectKey);
    const storageStateJson = JSON.parse(storageStateBuffer.toString("utf8"));

    const browser = await chromium.launch({
      executablePath,
      headless: parseBoolean(process.env.GEMINI_CANVAS_BROWSER_HEADLESS, true),
      args: [
        "--disable-blink-features=AutomationControlled",
        "--disable-dev-shm-usage",
        "--no-first-run",
        "--no-default-browser-check",
      ],
    });

    try {
      const context = await browser.newContext({
        storageState: storageStateJson,
        locale,
        userAgent: normalizeString(input.userAgent) ?? undefined,
      });

      try {
        const page = await context.newPage();
        await page.goto(shareUrl, {
          waitUntil: "domcontentloaded",
          timeout: timeoutMs,
        });
        await page.waitForLoadState("networkidle", {
          timeout: Math.min(timeoutMs, 20_000),
        }).catch(() => undefined);
        const swState = await page
          .evaluate(async (waitTimeoutMs) => {
            if (!("serviceWorker" in navigator)) {
              return { supported: false, ready: false, controlled: false };
            }
            const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
            const timeout = Math.min(Number(waitTimeoutMs) || 20_000, 20_000);
            try {
              await Promise.race([
                navigator.serviceWorker.ready,
                new Promise((_, reject) =>
                  setTimeout(() => reject(new Error("service_worker_ready_timeout")), timeout),
                ),
              ]);
            } catch {
              // Ignore timeout and continue with best-effort state reporting.
            }
            await sleep(500);
            return {
              supported: true,
              ready: true,
              controlled: Boolean(navigator.serviceWorker.controller),
            };
          }, timeoutMs)
          .catch(() => ({ supported: false, ready: false, controlled: false }));
        if (swState.supported && !swState.controlled) {
          await page.reload({
            waitUntil: "domcontentloaded",
            timeout: Math.min(timeoutMs, 30_000),
          });
          await page.waitForLoadState("networkidle", {
            timeout: Math.min(timeoutMs, 20_000),
          }).catch(() => undefined);
        }

        const finalUrl = page.url();
        const lowered = finalUrl.toLowerCase();
        if (
          lowered.includes("signin") ||
          lowered.includes("servicelogin") ||
          lowered.includes("accounts.google.com")
        ) {
          printJsonAndExit({
            ok: false,
            status: 401,
            error: {
              code: "gemini_canvas_auth_redirect",
              message: `Gemini Canvas navigation redirected to ${finalUrl}.`,
              status: 401,
            },
          });
        }

        const result = await page.evaluate(
          async ({ requestUrl, method, headers, requestBodyText, timeoutMs }) => {
            const getCookieValue = (name) => {
              const pattern = `${name}=`;
              return document.cookie
                .split(";")
                .map((entry) => entry.trim())
                .find((entry) => entry.startsWith(pattern))
                ?.slice(pattern.length) ?? null;
            };
            const buildSapisidHash = async (authOrigin) => {
              const sapisid =
                getCookieValue("SAPISID") ??
                getCookieValue("__Secure-3PAPISID") ??
                getCookieValue("APISID");
              if (!sapisid) {
                return null;
              }
              const timestamp = Math.floor(Date.now() / 1000);
              const input = `${timestamp} ${sapisid} ${authOrigin}`;
              const digest = await crypto.subtle.digest(
                "SHA-1",
                new TextEncoder().encode(input),
              );
              const hash = Array.from(new Uint8Array(digest))
                .map((byte) => byte.toString(16).padStart(2, "0"))
                .join("");
              return `SAPISIDHASH ${timestamp}_${hash}`;
            };
            const controller = new AbortController();
            const timeoutId = setTimeout(() => controller.abort("timeout"), timeoutMs);
            try {
              const targetOrigin = new URL(requestUrl, location.origin).origin;
              const authHeader = await buildSapisidHash(targetOrigin);
              const finalHeaders = {
                ...headers,
                ...(authHeader
                  ? {
                      authorization: authHeader,
                      "x-origin": targetOrigin,
                      "x-goog-authuser": "0",
                    }
                  : {}),
              };
              const response = await fetch(requestUrl, {
                method,
                headers: finalHeaders,
                body: requestBodyText ?? undefined,
                credentials: "include",
                signal: controller.signal,
              });
              const responseBodyText = await response.text();
              return {
                ok: response.ok,
                status: response.status,
                contentType: response.headers.get("content-type"),
                bodyText: responseBodyText,
              };
            } catch (error) {
              const message = error instanceof Error ? error.message : String(error);
              return {
                ok: false,
                status: message.includes("timeout") ? 504 : 500,
                error: {
                  code: message.includes("timeout")
                    ? "gemini_canvas_browser_timeout"
                    : "gemini_canvas_browser_fetch_failed",
                  message,
                  status: message.includes("timeout") ? 504 : 500,
                },
              };
            } finally {
              clearTimeout(timeoutId);
            }
          },
          {
            requestUrl: input.requestUrl,
            method: normalizeString(input.method) ?? "POST",
            headers: {
              "content-type": "application/json",
              accept: "application/json",
              ...(input.headers && typeof input.headers === "object" ? input.headers : {}),
            },
            requestBodyText: input.requestBody ? JSON.stringify(input.requestBody) : null,
            timeoutMs,
          },
        );

        printJsonAndExit(result, result.ok ? 0 : 1);
      } finally {
        await context.close().catch(() => undefined);
      }
    } finally {
      await browser.close().catch(() => undefined);
    }
  } catch (error) {
    printJsonAndExit(
      {
        ok: false,
        error: {
          code: "gemini_canvas_browser_worker_failed",
          message: error instanceof Error ? error.message : String(error),
          status: Number(error?.status ?? 500),
        },
      },
      1,
    );
  }
}

main();
