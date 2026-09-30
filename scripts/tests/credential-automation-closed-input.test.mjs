import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { once } from "node:events";
import { copyFile, mkdtemp, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { test } from "node:test";

test("POSIX closed-input fixture closes the OS pipe while remaining alive", {
  skip: process.platform === "win32" && "Windows uses the separate closed_input.ps1 fixture",
}, async () => {
  const directory = await mkdtemp(path.join(tmpdir(), "gateway-closed-input-"));
  const script = path.join(directory, "closed-input.mjs");
  await copyFile(new URL("../../src/credential_pool_automation/driver/script_contract/fixture.mjs", import.meta.url), script);
  const child = spawn(process.execPath, [script], { stdio: ["pipe", "ignore", "ignore"] });
  const exit = once(child, "exit");
  let timer;
  try {
    const error = await new Promise((resolve, reject) => {
      timer = setTimeout(() => reject(new Error("fixture left stdin open")), 3000);
      child.once("error", reject);
      child.stdin.on("error", resolve);
      child.stdin.write(Buffer.alloc(1024 * 1024), (error) => {
        if (error) resolve(error);
        else reject(new Error("closed-input fixture accepted the complete request"));
      });
    });
    assert.match(error.code, /EPIPE|ERR_STREAM_DESTROYED/);
    assert.equal(child.exitCode, null, "the fixture must close stdin, not exit");
  } finally {
    clearTimeout(timer);
    child.kill();
    await exit;
    await rm(directory, { recursive: true, force: true });
  }
});
