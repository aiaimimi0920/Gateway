/** Two phases: ordinary manual login first; cookie-only CDP attachment after that browser exits. */
import { spawn } from "node:child_process";
import { createServer } from "node:net";
import { lstat, mkdir, mkdtemp, realpath, rm } from "node:fs/promises";
import path from "node:path";

const sleep = ms => new Promise(resolve => setTimeout(resolve, ms));

export function manualLoginArguments(profileDir, targetUrl, disableBackgroundMode = false) {
  return [`--user-data-dir=${profileDir}`, "--no-first-run", "--no-default-browser-check",
    ...(disableBackgroundMode ? ["--disable-background-mode"] : []), targetUrl];
}

export function cookieImportArguments(profileDir, targetUrl, port, disableBackgroundMode = false) {
  if (!Number.isInteger(port) || port < 1 || port > 65535) throw new Error("Invalid import port.");
  return [...manualLoginArguments(profileDir, targetUrl, disableBackgroundMode).slice(0, -1),
    "--remote-debugging-address=127.0.0.1", `--remote-debugging-port=${port}`, targetUrl];
}

export function ownedProfilePath(root, profileDir) {
  if (!path.isAbsolute(root) || !path.isAbsolute(profileDir)) throw new Error("Absolute temporary paths required.");
  const profile = path.resolve(profileDir);
  if (path.dirname(profile) !== path.resolve(root) || !/^aistudio-login-[\w-]+$/.test(path.basename(profile))) {
    throw new Error("Refusing cleanup outside the owned temporary profile.");
  }
  return profile;
}

function startBrowser(executablePath, args) {
  // No shell, Playwright launch defaults, inherited profile, or automated login interactions.
  const child = spawn(executablePath, args, { stdio: "ignore", windowsHide: false, shell: false });
  const owner = { child, exited: false };
  owner.closed = new Promise(resolve => {
    child.once("error", () => { owner.exited = true; resolve(false); });
    child.once("exit", code => { owner.exited = true; resolve(code === 0); });
  });
  return owner;
}

async function bounded(promise, timeoutMs) {
  let timer;
  try {
    return await Promise.race([promise, new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error("Manual browser phase timed out.")), timeoutMs);
    })]);
  } finally { clearTimeout(timer); }
}

async function availablePort() {
  const server = createServer();
  await new Promise((resolve, reject) => {
    server.once("error", reject);
    server.listen(0, "127.0.0.1", resolve);
  });
  const port = server.address().port;
  await new Promise((resolve, reject) => server.close(error => error ? reject(error) : resolve()));
  return port;
}

export async function openManualLoginBrowser({ chromium, executablePath, targetUrl, tempRoot, timeoutMs = 1_200_000, disableBackgroundMode = false,
  progress = value => process.stdout.write(`${JSON.stringify(value)}\n`) }) {
  if (!tempRoot || !path.isAbsolute(tempRoot)) throw new Error("An explicit absolute private temporary root is required.");
  await mkdir(tempRoot, { recursive: true, mode: 0o700 });
  const root = await realpath(tempRoot);
  const profileDir = await mkdtemp(path.join(root, "aistudio-login-"));
  let owner, browser, control, stopPromise, interrupted = false;
  const onInterrupt = () => {
    interrupted = true;
    process.exitCode = 130;
    void close().catch(() => { process.exitCode = 1; });
  };
  // Cleanup is confined to this invocation's mkdtemp child, never a real user profile or credential object.
  async function close() {
    if (stopPromise) return stopPromise;
    stopPromise = (async () => {
      process.removeListener("SIGINT", onInterrupt);
      process.removeListener("SIGTERM", onInterrupt);
      if (control) await bounded(control.send("Browser.close"), 5000).catch(() => {});
      if (browser) await bounded(browser.close(), 5000).catch(() => {});
      if (control && owner) await bounded(owner.closed, 3000).catch(() => {});
      if (owner && !owner.exited) owner.child.kill();
      if (owner) await bounded(owner.closed, 5000).catch(() => {});
      if (owner && !owner.exited) throw new Error("Temporary browser is still running; profile retained.");
      const profile = ownedProfilePath(root, profileDir);
      if ((await lstat(profile)).isSymbolicLink() || await realpath(profile) !== profile) {
        throw new Error("Temporary profile changed identity; cleanup refused.");
      }
      await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
    })();
    return stopPromise;
  }
  process.once("SIGINT", onInterrupt);
  process.once("SIGTERM", onInterrupt);
  try {
    owner = startBrowser(executablePath, manualLoginArguments(profileDir, targetUrl, disableBackgroundMode));
    progress({ status: "waiting_manual_login", automationConnected: false,
      closeDedicatedWindowAfterLogin: true, browserPid: owner.child.pid });
    if (!await bounded(owner.closed, timeoutMs) || interrupted) throw new Error("Manual browser did not exit normally.");
    const port = await availablePort();
    if (interrupted) throw new Error("Manual login was cancelled.");
    owner = startBrowser(executablePath, cookieImportArguments(profileDir, targetUrl, port, disableBackgroundMode));
    const deadline = Date.now() + 15_000;
    while (Date.now() < deadline && !owner.exited) {
      try {
        browser = await chromium.connectOverCDP(`http://127.0.0.1:${port}`, { timeout: 1000 });
        break;
      } catch { await sleep(200); }
    }
    const context = browser?.contexts()[0];
    if (!context) throw new Error("Unable to attach to the dedicated cookie import browser.");
    const channel = await browser.newBrowserCDPSession();
    const info = await bounded(channel.send("SystemInfo.getProcessInfo"), 2000);
    if (!info.processInfo.some(item => item.type === "browser" && item.id === owner.child.pid)) {
      await channel.detach();
      throw new Error("Import browser ownership verification failed.");
    }
    control = channel;
    return { browser, context, close };
  } catch (error) {
    await close();
    throw error;
  }
}
