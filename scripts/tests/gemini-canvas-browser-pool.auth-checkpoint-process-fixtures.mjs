import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";

const checkpointUrl = new URL("../gemini-canvas-browser-pool-auth-checkpoint.mjs", import.meta.url).href;
const SCRIPT = `
import { loadAuthCheckpoint, saveAuthCheckpoint } from ${JSON.stringify(checkpointUrl)};
const source = JSON.parse(process.env.AUTH_CHECKPOINT_TEST_SOURCE);
try {
  const checkpoint = await loadAuthCheckpoint(source);
  process.send({ stage: "ready", value: checkpoint.storageState.cookies[0].value });
  const command = await new Promise(resolve => process.once("message", resolve));
  if (command === "write") {
    checkpoint.storageState.cookies[0].value = "child-rotated-fixture";
    await saveAuthCheckpoint(checkpoint, checkpoint.storageState);
  }
  process.send({ stage: "done", result: "saved" });
} catch(error) {
  process.send({ stage: "done", result: error.code });
}
process.disconnect();
`;

export function checkpointProcess(t, source, script = SCRIPT, extraEnv = {}) {
  const child = spawn(process.execPath, ["--input-type=module", "-e", script], {
    cwd: process.cwd(), env: { ...process.env, AUTH_CHECKPOINT_TEST_SOURCE: JSON.stringify(source), ...extraEnv },
    stdio: ["ignore", "pipe", "pipe", "ipc"], windowsHide: true,
  });
  let stdout = "", stderr = "";
  child.stdout.on("data", chunk => { stdout += chunk; });
  child.stderr.on("data", chunk => { stderr += chunk; });
  const exit = once(child, "exit");
  const pending = [], received = [];
  child.on("message", value => { const resolve = pending.shift(); if (resolve) resolve(value); else received.push(value); });
  const timer = setTimeout(() => child.kill(), 15000);
  t.after(async () => {
    clearTimeout(timer);
    if (child.exitCode === null && child.signalCode === null) child.kill();
    await exit;
    assert.equal(stdout, "");
    assert.equal(stderr.includes("child-rotated-fixture"), false);
  });
  return { child, exit, next: () => received.length ? Promise.resolve(received.shift()) :
    Promise.race([new Promise(resolve => pending.push(resolve)), exit.then(() => { throw new Error("Checkpoint child exited before receipt"); })]) };
}
