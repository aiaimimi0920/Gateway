import { chromium } from "playwright-core";
import { existsSync, statSync } from "node:fs";
import net from "node:net";
import { mkdir, readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import process from "node:process";

const PROVIDERS = {
  udio: {
    targetUrl: "https://www.udio.com/create",
    statePath: ".runtime/udio-manual-browser-helper.storage-state.json",
    outputStatePath: ".runtime/udio-cdp-browser-helper.storage-state.json",
    objectStatePath:
      ".runtime/ai-gateway-objects/credential-runtime/udio/manual-browser-helper/storage-state.json",
    statusPath: ".runtime/udio-cdp-browser-helper.status.json",
    port: 9225,
  },
  suno: {
    targetUrl: "https://suno.com/create",
    statePath: ".runtime/suno-manual-browser-helper.storage-state.json",
    outputStatePath: ".runtime/suno-cdp-browser-helper.storage-state.json",
    objectStatePath:
      ".runtime/ai-gateway-objects/credential-runtime/suno/manual-browser-helper/storage-state.json",
    statusPath: ".runtime/suno-cdp-browser-helper.status.json",
    port: 9226,
  },
};

const BROWSER_PATHS = [
  "C:\\Program Files\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Google\\Chrome\\Application\\chrome.exe",
  "C:\\Program Files (x86)\\Microsoft\\Edge\\Application\\msedge.exe",
  "C:\\Program Files\\Microsoft\\Edge\\Application\\msedge.exe",
];

function parseArgs(argv) {
  const providerIndex = argv.indexOf("--provider");
  const provider = providerIndex >= 0 ? argv[providerIndex + 1] : null;
  if (!provider || !PROVIDERS[provider]) {
    throw new Error("--provider must be udio or suno");
  }
  return { provider, ...PROVIDERS[provider] };
}

async function writeJson(filePath, value) {
  const absolute = path.resolve(process.cwd(), filePath);
  await mkdir(path.dirname(absolute), { recursive: true });
  await writeFile(absolute, JSON.stringify(value, null, 2), "utf8");
}

async function main() {
  const options = parseArgs(process.argv.slice(2));
  if (await isPortListening(options.port)) {
    throw new Error(`CDP port ${options.port} is already in use; refusing to open a duplicate window.`);
  }
  const executablePath = BROWSER_PATHS.find((candidate) => existsSync(candidate));
  if (!executablePath) {
    throw new Error("Chrome or Edge was not found");
  }
  const statePath = [options.outputStatePath, options.objectStatePath, options.statePath]
    .map((candidate) => path.resolve(candidate))
    .filter((candidate) => existsSync(candidate))
    .sort((left, right) => {
      const leftMtime = statSync(left).mtimeMs;
      const rightMtime = statSync(right).mtimeMs;
      return rightMtime - leftMtime;
    })[0];
  if (!statePath) {
    throw new Error(`No cached ${options.provider} browser state is available.`);
  }
  const state = JSON.parse((await readFile(statePath, "utf8")).replace(/^\uFEFF/, ""));
  const browser = await chromium.launch({
    executablePath,
    headless: false,
    args: [
      `--remote-debugging-port=${options.port}`,
      "--remote-debugging-address=127.0.0.1",
      "--remote-allow-origins=*",
      "--disable-blink-features=AutomationControlled",
      "--no-first-run",
      "--no-default-browser-check",
      "--start-maximized",
    ],
  });
  const context = await browser.newContext({ storageState: state, locale: "en-US", viewport: null });
  const page = await context.newPage();
  await page.goto(options.targetUrl, { waitUntil: "domcontentloaded", timeout: 60_000 });

  let stopping = false;
  const stop = () => {
    stopping = true;
  };
  process.on("SIGINT", stop);
  process.on("SIGTERM", stop);

  try {
    while (!stopping && !page.isClosed()) {
      const nextState = await context.storageState();
      if (hasPersistableProviderState(nextState, options.provider)) {
        await writeJson(options.outputStatePath, nextState);
        await writeJson(options.objectStatePath, nextState);
      }
      await writeJson(options.statusPath, {
        ok: true,
        provider: options.provider,
        pid: process.pid,
        cdpUrl: `http://127.0.0.1:${options.port}`,
        currentUrl: page.url(),
        updatedAt: new Date().toISOString(),
      });
      await new Promise((resolve) => setTimeout(resolve, 5_000));
    }
  } finally {
    await context.storageState({ path: path.resolve(options.outputStatePath) }).catch(() => undefined);
    await browser.close().catch(() => undefined);
  }
}

function hasPersistableProviderState(state, provider) {
  const cookies = Array.isArray(state?.cookies) ? state.cookies : [];
  if (provider === "udio") {
    return cookies.some((cookie) =>
      String(cookie?.name ?? "").startsWith("sb-ssr-production-auth-token"),
    );
  }
  return cookies.length > 0;
}

async function isPortListening(port) {
  return await new Promise((resolve) => {
    const socket = net.createConnection({ host: "127.0.0.1", port });
    const finish = (listening) => {
      socket.removeAllListeners();
      socket.destroy();
      resolve(listening);
    };
    socket.setTimeout(500);
    socket.once("connect", () => finish(true));
    socket.once("timeout", () => finish(false));
    socket.once("error", () => finish(false));
  });
}

main().catch(async (error) => {
  const provider = process.argv[process.argv.indexOf("--provider") + 1] ?? "unknown";
  const statusPath = PROVIDERS[provider]?.statusPath ?? ".runtime/media-cdp-browser-helper.status.json";
  await writeJson(statusPath, {
    ok: false,
    provider,
    updatedAt: new Date().toISOString(),
    error: error instanceof Error ? error.message : String(error),
  }).catch(() => undefined);
  process.exitCode = 1;
});
