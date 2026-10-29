// Fixed metadata only, inside the Rust supervisor's owned scratch directory.
import { lstat, open, realpath, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath, pathToFileURL } from "node:url";

const phases = ["login_window", "session_read", "verification_required", "verification_window", "session_read"];
export async function createLoginInteraction(root = process.env.GATEWAY_CHATGPT_LOGIN_CHANNEL) {
  if (!root) return undefined;
  if (!path.isAbsolute(root)) throw new Error("invalid_input");
  const owned = await realpath(root);
  if (!(await lstat(owned)).isDirectory()) throw new Error("invalid_input");
  const samePath = (value) => {
    const normalized = fileURLToPath(pathToFileURL(value));
    return process.platform === "win32" ? normalized.toLowerCase() : normalized;
  };
  if (samePath(owned) !== samePath(root)) throw new Error("invalid_input");
  const command = path.join(owned, "login-verify");
  let step = 0;
  const checkRoot = async () => {
    if (samePath(await realpath(owned)) !== samePath(owned)) throw new Error("invalid_input");
  };
  return {
    async setPhase(phase) {
      if (phase !== phases[step]) throw new Error("invalid_input");
      await checkRoot();
      // Immutable empty markers avoid Windows/SMB replace-vs-reader sharing conflicts.
      await writeFile(path.join(owned, `login-step-${step + 1}`), "", { encoding: "utf8", mode: 0o600, flag: "wx" });
      step++;
    },
    async waitForVerification(deadline, signal) {
      while (!signal.aborted && Date.now() < deadline) {
        await checkRoot();
        try {
          const metadata = await lstat(command);
          if (!metadata.isFile() || metadata.isSymbolicLink() || metadata.size !== 0) throw new Error("invalid_input");
          const file = await open(command, "r");
          await file.close();
          if (!signal.aborted) return;
        } catch (error) { if (error.code !== "ENOENT") throw error; }
        await new Promise((resolve) => {
          const finish = () => { clearTimeout(timer); signal.removeEventListener("abort", finish); resolve(); };
          const timer = setTimeout(finish, 200);
          signal.addEventListener("abort", finish, { once: true });
          if (signal.aborted) finish();
        });
      }
      throw new Error(signal.aborted ? "verification_cancelled" : "verification_wait_timeout");
    },
  };
}
