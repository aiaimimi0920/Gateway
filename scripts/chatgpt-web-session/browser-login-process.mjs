import { spawn } from "node:child_process";
import { mkdtemp, open, unlink } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";
import { closeNativeSession, ownedProcessRunning } from "./browser-login-cleanup.mjs";

const pause = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
async function bounded(promise, ms) {
  let timer;
  try { return await Promise.race([promise, new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error("browser_timeout")), ms);
  })]); } finally { clearTimeout(timer); }
}

async function readEndpoint(profile) {
  const file = await open(path.join(profile, "DevToolsActivePort"), "r");
  try {
    const buffer = Buffer.alloc(256);
    const { bytesRead } = await file.read(buffer, 0, buffer.length, 0);
    const [rawPort, browserPath] = buffer.subarray(0, bytesRead).toString("utf8").trim().split(/\r?\n/);
    const port = Number(rawPort);
    if (bytesRead === buffer.length || !/^\d{4,5}$/.test(rawPort) || port < 1024 || port > 65535 ||
        [4200, 4300].includes(port) || !/^\/devtools\/browser\/[a-zA-Z0-9-]+$/.test(browserPath)) throw new Error("invalid_debugger");
    return { port, browserPath };
  } finally { await file.close(); }
}

async function browserMetadata(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/version`, {
    redirect: "error", signal: AbortSignal.timeout(3000),
  });
  if (!response.ok || !response.body) throw new Error("browser_unavailable");
  const chunks = [];
  let size = 0;
  for await (const chunk of response.body) {
    size += chunk.byteLength;
    if (size > 16384) throw new Error("invalid_debugger");
    chunks.push(chunk);
  }
  return JSON.parse(Buffer.concat(chunks).toString("utf8"));
}

/** Only a freshly-created profile and children of this worker are owned here. */
export async function createNativeBrowserSession({ executablePath, spawnProcess = spawn,
  terminateProcess, readMetadata = browserMetadata, closeBrowser, processExists } = {}) {
  const profile = await mkdtemp(path.join(tmpdir(), "gateway-chatgpt-login-"));
  const children = [];
  // Chromium opens the window with a Windows verbatim path but cannot persist its Cookie DB.
  // Normalize only its argv spelling; file I/O and cleanup retain the owned mkdtemp path.
  const browserProfile = process.platform === "win32" ? fileURLToPath(pathToFileURL(profile)) : profile;
  const args = [`--user-data-dir=${browserProfile}`, "--no-first-run", "--no-default-browser-check"];
  const launch = (extra, url = "https://chatgpt.com/") => {
    const child = spawnProcess(executablePath, [...args, ...extra, "--new-window", url],
      { stdio: "ignore", windowsHide: false, detached: false });
    const done = new Promise((resolve) => {
      child.once("exit", (code) => resolve({ code }));
      child.once("error", () => resolve({ error: true }));
    });
    const entry = { child, done };
    children.push(entry);
    return entry;
  };
  let native;
  let capture;
  let browserEndpoint;
  let cleanup;
  let transition;
  let verificationStarted = false;
  let loginClosed = false;
  const alive = (entry) => entry && ownedProcessRunning(entry.child, processExists);
  const close = () => cleanup ??= (async () => {
    await transition?.catch(() => {});
    await closeNativeSession({ children, profile, browserEndpoint, terminateProcess, closeBrowser, processExists });
  })();
  const waitForWindowClosed = async (timeoutMs) => {
    const outcome = await bounded(native.done, timeoutMs);
    if (outcome.error || outcome.code !== 0) throw new Error("browser_closed");
    loginClosed = true;
  };
  try { native = launch([]); }
  catch { await close().catch(() => {}); throw new Error("browser_unavailable"); }
  return {
    waitForWindowClosed,
    async prepareVerification() {
      if (!capture || verificationStarted || cleanup || transition) throw new Error("invalid_input");
      verificationStarted = true;
      transition = closeNativeSession({ children, profile, browserEndpoint, terminateProcess, closeBrowser, processExists, retainProfile: true });
      await transition;
      capture = undefined; browserEndpoint = undefined; loginClosed = false;
    },
    async openVerification(timeoutMs) {
      if (!verificationStarted || capture || cleanup || loginClosed || alive(native)) throw new Error("invalid_input");
      // Only explicit user consent opens this normal first-party verification page; no debugger is enabled.
      native = launch([], "https://chatgpt.com/api/auth/session");
      await waitForWindowClosed(timeoutMs);
    },
    async startCapture(timeoutMs) {
      if (!loginClosed || capture || cleanup) throw new Error("invalid_input");
      // Human/Google sign-in is finished before any debugging endpoint is enabled.
      await unlink(path.join(profile, "DevToolsActivePort")).catch((error) => { if (error.code !== "ENOENT") throw error; });
      if (cleanup) throw new Error("browser_closed");
      capture = launch(["--remote-debugging-address=127.0.0.1", "--remote-debugging-port=0"]);
      const deadline = Date.now() + timeoutMs;
      do {
        if (!alive(capture)) throw new Error("browser_closed");
        try {
          const { port, browserPath } = await readEndpoint(profile);
          const metadata = await readMetadata(port);
          if (metadata.webSocketDebuggerUrl !== `ws://127.0.0.1:${port}${browserPath}`) throw new Error("invalid_debugger");
          browserEndpoint = metadata.webSocketDebuggerUrl;
          return port;
        } catch (error) {
          if (error?.message === "invalid_debugger") throw error;
        }
        await pause(100);
      } while (Date.now() < deadline);
      throw new Error("browser_capture_start_timeout");
    },
    isAlive: () => Boolean(alive(capture)),
    close,
  };
}
