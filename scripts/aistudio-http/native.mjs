/** Native helper contains SpiderMonkey, not a browser. Parent supervisor owns the whole process tree. */
import { spawn } from "node:child_process";
import { runtimeExecutable } from "./runtime-path.mjs";

export function helperPath() {
  return runtimeExecutable("aistudio-proof.exe", process.env.AISTUDIO_WAA_HELPER_PATH, "aistudio_http_waa_helper_missing");
}

export async function nativeProof(input, executable = helperPath()) {
  const encoded = JSON.stringify(input);
  if (Buffer.byteLength(encoded) > 12 * 1024 * 1024) throw new Error("aistudio_http_waa_input_limit");
  const child = spawn(executable, [], { windowsHide: true, shell: false, stdio: ["pipe", "pipe", "pipe"] });
  const chunks = []; let size = 0, exceeded = false;
  const timer = setTimeout(() => child.kill(), 65000);
  child.stdout.on("data", chunk => {
    size += chunk.length;
    if (size > 128 * 1024) { exceeded = true; child.kill(); } else chunks.push(chunk);
  });
  child.stderr.resume();
  child.stdin.on("error", () => {});
  const closed = new Promise((resolve, reject) => { child.once("error", reject); child.once("close", resolve); });
  child.stdin.end(encoded);
  try {
    const status = await closed;
    if (exceeded || status !== 0) throw new Error("aistudio_http_waa_failed");
    const result = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    if (!result.ok || typeof result.proof !== "string" || !result.proof.startsWith("!") || result.proof.length > 65536) {
      throw new Error("aistudio_http_waa_failed");
    }
    return result.proof;
  } finally { clearTimeout(timer); }
}
