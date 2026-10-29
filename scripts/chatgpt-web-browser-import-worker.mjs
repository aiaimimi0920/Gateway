import { importBrowserSession, SESSION_FAILURE_CODES } from "./chatgpt-web-session/browser-import.mjs";
import { loginBrowserSession } from "./chatgpt-web-session/browser-login.mjs";
import { createLoginInteraction } from "./chatgpt-web-session/browser-login-interaction.mjs";

// Fixed, bounded stdin/stdout protocol; errors never echo CDP responses or session secrets.
const timer = setTimeout(() => { process.stdout.write('{"ok":false,"code":"browser_work_timeout"}\n'); process.exit(1); }, 220000);
try {
  const chunks = [];
  let size = 0;
  for await (const chunk of process.stdin) {
    size += chunk.length;
    if (size > 4096) throw new Error("invalid_input");
    chunks.push(chunk);
  }
  const input = JSON.parse(Buffer.concat(chunks).toString("utf8"));
  if (input.operation === "login" && Object.keys(input).some((key) => key !== "operation")) throw new Error("invalid_input");
  const result = input.operation === "login" ? await loginBrowserSession({ interaction: await createLoginInteraction() }) : await importBrowserSession(input);
  const output = JSON.stringify({ ok: true, ...result });
  if (Buffer.byteLength(output) > 128 * 1024) throw new Error("incomplete_session");
  process.stdout.write(output + "\n", () => process.exit(0));
} catch (error) {
  const allowed = ["invalid_input", "tab_unavailable", "incomplete_session", "invalid_debugger", "browser_timeout",
    "browser_closed", "browser_not_found", "browser_capture_start_timeout", "browser_work_timeout", "verification_wait_timeout", "verification_prepare_failed", ...SESSION_FAILURE_CODES];
  const code = allowed.includes(error?.message) ? error.message : "browser_unavailable";
  process.stdout.write(JSON.stringify({ ok: false, code, ...(error?.cleanupPending === true ? { cleanupPending: true } : {}) }) + "\n", () => process.exit(0));
} finally { clearTimeout(timer); }
