import http from "node:http";
import process from "node:process";
import { chromium } from "playwright-core";

const DEFAULT_BRIDGE_TIMEOUT_MS = 10 * 60 * 1000;
const DEFAULT_POLL_INTERVAL_MS = 500;
const DEFAULT_EXECUTOR_TIMEOUT_MS = 9 * 60 * 1000;

function normalizeString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : null;
}

function normalizeTimeoutMs(value, fallback, min, max) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) {
    return fallback;
  }
  return Math.min(Math.max(Math.trunc(parsed), min), max);
}

async function readStdin() {
  const chunks = [];
  for await (const chunk of process.stdin) {
    chunks.push(Buffer.from(chunk));
  }
  return Buffer.concat(chunks).toString("utf8");
}

function printJson(value) {
  process.stdout.write(`${JSON.stringify(value)}\n`);
}

async function waitForCaptchaToken(page, timeoutMs, pollIntervalMs) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const token = await page
      .evaluate(() => {
        const values = [];
        for (const el of Array.from(document.querySelectorAll("textarea, input"))) {
          const name = (el.getAttribute("name") || "").trim();
          if (name === "g-recaptcha-response" || name === "h-captcha-response") {
            values.push(typeof el.value === "string" ? el.value.trim() : "");
          }
        }
        return values.find((value) => value.length > 0) || null;
      })
      .catch(() => null);
    if (token) {
      return token;
    }
    await page.waitForTimeout(pollIntervalMs).catch(() => undefined);
  }
  throw new Error("Timed out waiting for a non-empty hCaptcha response on the bridged Udio page.");
}

async function postExecutor(baseUrl, bearerToken, payload, timeoutMs) {
  const url = new URL("/v1/internal/browser-executor/execute", baseUrl);
  return await new Promise((resolve, reject) => {
    const body = JSON.stringify(payload);
    const req = http.request(
      {
        protocol: url.protocol,
        hostname: url.hostname,
        port: url.port,
        path: `${url.pathname}${url.search}`,
        method: "POST",
        headers: {
          "content-type": "application/json",
          "content-length": Buffer.byteLength(body),
          ...(normalizeString(bearerToken)
            ? { authorization: `Bearer ${normalizeString(bearerToken)}` }
            : {}),
        },
        timeout: timeoutMs,
      },
      (res) => {
        let text = "";
        res.setEncoding("utf8");
        res.on("data", (chunk) => {
          text += chunk;
        });
        res.on("end", () => {
          resolve({
            status: res.statusCode ?? 0,
            text,
          });
        });
      },
    );
    req.on("timeout", () => {
      req.destroy(new Error("browser_executor_request_timeout"));
    });
    req.on("error", reject);
    req.write(body);
    req.end();
  });
}

async function main() {
  const raw = (await readStdin()).replace(/^\uFEFF/, "");
  const input = JSON.parse(raw);
  const browserCdpUrl = normalizeString(input.browserCdpUrl);
  if (!browserCdpUrl) {
    throw new Error("browserCdpUrl is required.");
  }
  const browserCdpTargetUrl =
    normalizeString(input.browserCdpTargetUrl) ?? "https://www.udio.com/create";
  const bridgeTimeoutMs = normalizeTimeoutMs(
    input.bridgeTimeoutMs,
    DEFAULT_BRIDGE_TIMEOUT_MS,
    5_000,
    20 * 60 * 1000,
  );
  const pollIntervalMs = normalizeTimeoutMs(
    input.pollIntervalMs,
    DEFAULT_POLL_INTERVAL_MS,
    100,
    5_000,
  );
  const cdpConnectTimeoutMs = normalizeTimeoutMs(
    input.browserCdpConnectTimeoutMs,
    300_000,
    5_000,
    10 * 60 * 1000,
  );
  const executorBaseUrl =
    normalizeString(input.executorBaseUrl) ?? "http://127.0.0.1:42341";
  const executorBearerToken = normalizeString(input.executorBearerToken);
  const executorPayload = input.executorPayload;
  if (!executorPayload || typeof executorPayload !== "object" || Array.isArray(executorPayload)) {
    throw new Error("executorPayload is required.");
  }

  const browser = await chromium.connectOverCDP(browserCdpUrl, {
    timeout: cdpConnectTimeoutMs,
  });
  try {
    const context = browser.contexts()[0];
    const page =
      context.pages().find((entry) => entry.url().startsWith(browserCdpTargetUrl)) ??
      context.pages()[0];
    if (!page) {
      throw new Error("No Udio page found in the bridged CDP browser.");
    }
    await page.bringToFront().catch(() => undefined);
    const token = await waitForCaptchaToken(page, bridgeTimeoutMs, pollIntervalMs);
    const bridgedPayload = JSON.parse(JSON.stringify(executorPayload));
    if (
      !bridgedPayload?.input?.requestBody ||
      typeof bridgedPayload.input.requestBody !== "object" ||
      Array.isArray(bridgedPayload.input.requestBody)
    ) {
      throw new Error("executorPayload.input.requestBody must be a JSON object.");
    }
    bridgedPayload.input.requestBody.captchaToken = token;
    const response = await postExecutor(
      executorBaseUrl,
      executorBearerToken,
      bridgedPayload,
      DEFAULT_EXECUTOR_TIMEOUT_MS,
    );
    printJson({
      ok: true,
      tokenLength: token.length,
      executorStatus: response.status,
      executorBody: response.text,
    });
  } finally {
    await browser.close().catch(() => undefined);
  }
}

main().catch((error) => {
  printJson({
    ok: false,
    message: error?.message ?? String(error),
  });
  process.exit(1);
});
