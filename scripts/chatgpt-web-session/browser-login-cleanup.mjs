import { spawn } from "node:child_process";
import { rm } from "node:fs/promises";
import path from "node:path";

async function bounded(promise, ms) {
  let timer;
  try { return await Promise.race([promise, new Promise((_, reject) => {
    timer = setTimeout(() => reject(new Error("browser_cleanup_timeout")), ms);
  })]); } finally { clearTimeout(timer); }
}

const alive = (child) => Number.isSafeInteger(child.pid) && child.pid > 0 && child.exitCode === null && child.signalCode === null;
function processExists(pid) {
  try { process.kill(pid, 0); return true; }
  catch (error) { return error.code !== "ESRCH"; }
}
export function ownedProcessRunning(child, exists = processExists) {
  if (!alive(child)) return false;
  try { return exists(child.pid) !== false; } catch { return true; }
}
async function waitForExit(child, ms, running) {
  const deadline = Date.now() + ms;
  while (running(child)) {
    if (Date.now() >= deadline) throw new Error("browser_cleanup_timeout");
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
}
export async function terminateOwned(child) {
  if (!alive(child) || !processExists(child.pid)) return;
  if (process.platform !== "win32") { child.kill("SIGKILL"); return; }
  const helper = spawn(path.join(process.env.SystemRoot || "C:\\Windows", "System32", "taskkill.exe"),
    ["/PID", String(child.pid), "/T", "/F"], { stdio: "ignore", windowsHide: true });
  const done = new Promise((resolve, reject) => {
    helper.once("exit", resolve); helper.once("error", reject);
  });
  try { await bounded(done, 8000); }
  finally {
    // Never leave our taskkill helper running after its own deadline.
    if (alive(helper)) helper.kill();
    await bounded(done, 500).catch(() => {});
  }
}

async function closeDebugger(endpoint) {
  // The factory passes only its verified, ephemeral loopback browser endpoint.
  const socket = new WebSocket(endpoint);
  try {
    await bounded(new Promise((resolve, reject) => {
      socket.addEventListener("open", () => {
        try { socket.send(JSON.stringify({ id: 1, method: "Browser.close" })); }
        catch { reject(new Error("browser_cleanup_failed")); }
      }, { once: true });
      socket.addEventListener("close", resolve, { once: true });
      socket.addEventListener("error", reject, { once: true });
    }), 2000);
  } finally { socket.close(); }
}

/** Graceful close first; failures leave profile deletion to the parent-owned supervisor. */
export async function closeNativeSession({ children, profile, browserEndpoint,
  terminateProcess = terminateOwned, closeBrowser = closeDebugger, removeProfile = rm, retainProfile = false,
  processExists: exists = processExists }) {
  // Windows can report ESRCH before Node delivers the owned ChildProcess's exit callback.
  // Only explicit OS absence counts; probe errors remain conservatively live.
  const running = (child) => ownedProcessRunning(child, exists);
  const cleanup = async () => {
    const capture = children.at(-1);
    if (browserEndpoint && capture && running(capture.child)) {
      try { await closeBrowser(browserEndpoint); await waitForExit(capture.child, 2000, running); }
      catch { /* Fall back to terminating only this session's owned process tree. */ }
    }
    let pending = false;
    for (const entry of children.slice().reverse()) {
      if (running(entry.child)) {
        try { await terminateProcess(entry.child); }
        catch { /* A helper failure is not proof that its owned target is still running. */ }
      }
      try {
        await waitForExit(entry.child, 1000, running);
      } catch { pending = true; }
    }
    // Never recursively remove a profile that still has a known live owner.
    if (pending || children.some((entry) => running(entry.child))) throw new Error("browser_cleanup_timeout");
    if (retainProfile) return;
    try { await removeProfile(profile, { recursive: true, force: true, maxRetries: 3, retryDelay: 100 }); }
    catch { throw new Error("browser_cleanup_failed"); }
  };
  // The worker leaves 15 seconds for cleanup; Rust's Job Object remains the final owner.
  await bounded(cleanup(), 14000);
}
