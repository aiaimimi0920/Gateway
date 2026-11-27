// Operator-only fixed Codex acceptance; project operations belong to the client sandbox.
import fs from "node:fs";
import path from "node:path";
import { spawn } from "node:child_process";
import { cursorErrorCodes } from "./gateway_acceptance.mjs";

export function codingArguments(base, workspace, prompt) {
  const settings = {
    model_provider: "cursor_acceptance", model: "cursor-personal", approval_policy: "never",
    project_doc_max_bytes: 0, check_for_update_on_startup: false, web_search: "disabled",
    model_reasoning_effort: "none", model_reasoning_summary: "none", "windows.sandbox": "unelevated",
    "shell_environment_policy.exclude": ["CURSOR_ACCEPTANCE_LOCAL_KEY", "*TOKEN*", "*SECRET*", "*API_KEY*"],
    "features.apps": false, "features.plugins": false, "features.multi_agent": false,
    "features.default_mode_request_user_input": false, "features.collaboration_modes": false,
    "features.remote_models": false, "features.api_key_model_discovery": false,
    "features.view_image": false, "features.responses_websockets": false,
    "features.responses_websockets_v2": false,
    "model_providers.cursor_acceptance.name": "Cursor coding acceptance",
    "model_providers.cursor_acceptance.base_url": `${base.replace(/\/$/, "")}/v1`,
    "model_providers.cursor_acceptance.wire_api": "responses",
    "model_providers.cursor_acceptance.env_key": "CURSOR_ACCEPTANCE_LOCAL_KEY",
    "model_providers.cursor_acceptance.requires_openai_auth": false,
    "model_providers.cursor_acceptance.supports_websockets": false,
    "model_providers.cursor_acceptance.request_max_retries": 0,
    "model_providers.cursor_acceptance.stream_max_retries": 0,
    "model_providers.cursor_acceptance.stream_idle_timeout_ms": 130000,
  };
  const args = ["exec", "--ignore-user-config", "--ignore-rules", "--ephemeral", "--skip-git-repo-check",
    "--sandbox", "workspace-write", "--color", "never", "--json", "--cd", workspace];
  for (const [key, value] of Object.entries(settings)) args.push("-c", `${key}=${JSON.stringify(value)}`);
  args.push("-c", `projects.${JSON.stringify(workspace)}.trust_level="trusted"`, prompt);
  return args;
}

export async function codingClient({ codex, base, workspace, token, prompt, timeoutMs = 280000 }) {
  if (!path.isAbsolute(codex)) throw new Error("absolute_codex_required");
  fs.mkdirSync(path.join(workspace, "home"), { recursive: true });
  const env = {};
  for (const name of ["SystemRoot", "WINDIR", "PATH", "PATHEXT", "USERPROFILE", "APPDATA", "LOCALAPPDATA", "ComSpec"]) {
    const key = Object.keys(process.env).find((k) => k.toLowerCase() === name.toLowerCase());
    if (key) env[key] = process.env[key];
  }
  Object.assign(env, { CODEX_HOME: path.join(workspace, "home"), TEMP: workspace, TMP: workspace,
    CURSOR_ACCEPTANCE_LOCAL_KEY: token, NO_PROXY: "127.0.0.1,localhost" });
  const child = spawn(codex, codingArguments(base, workspace, prompt), {
    cwd: workspace, env, windowsHide: true, stdio: ["ignore", "pipe", "pipe"],
  });
  delete env.CURSOR_ACCEPTANCE_LOCAL_KEY;
  let stdout = "", stderr = "", bytes = 0, bounded = false;
  const stop = () => {
    bounded = true;
    if (process.platform === "win32" && child.pid) {
      const killer = spawn(path.join(process.env.SystemRoot, "System32/taskkill.exe"),
        ["/PID", String(child.pid), "/T", "/F"], { windowsHide: true, stdio: "ignore" });
      killer.on("error", () => child.kill());
    } else child.kill();
  };
  const timer = setTimeout(stop, timeoutMs);
  child.stdout.on("data", (chunk) => {
    if ((bytes += chunk.length) > 1024 * 1024) stop(); else stdout += chunk.toString("utf8");
  });
  child.stderr.on("data", (chunk) => {
    if ((bytes += chunk.length) > 1024 * 1024) stop(); else stderr += chunk.toString("utf8");
  });
  const exitCode = await new Promise((resolve, reject) => {
    child.once("error", () => reject(new Error("codex_spawn_failed")));
    child.once("close", resolve);
  }).finally(() => clearTimeout(timer));
  const events = stdout.split(/\r?\n/).flatMap((line) => {
    try { const e = JSON.parse(line); return typeof e.type === "string" ? [e] : []; } catch { return []; }
  });
  return { exitCode, bounded, outputBytes: bytes, events,
    sawReconnect: /Reconnecting|retrying/.test(stdout + stderr),
    errorCodes: cursorErrorCodes(stdout + stderr),
    // Kept only in the operator process for diagnosis; callers must not persist arbitrary stderr.
    diagnostic: stderr.slice(-4096) };
}
