import { resolveExecutablePath } from "../producer-browser/profile.mjs";
import { importBrowserSession, SESSION_FAILURE_CODES } from "./browser-import.mjs";
import { createNativeBrowserSession } from "./browser-login-process.mjs";

// Human sign-in is native; only the closed, owned profile enters the capture phase.
export async function loginBrowserSession({ createSession = createNativeBrowserSession,
  readSession = importBrowserSession, timeoutMs = 180000, captureMs = 60000, pollMs = 1500,
  workTimeoutMs = 205000, interaction } = {}) {
  const executablePath = createSession === createNativeBrowserSession
    ? resolveExecutablePath(process.env.CHATGPT_WEB_BROWSER_EXECUTABLE_PATH) : undefined;
  if (createSession === createNativeBrowserSession && !executablePath) throw new Error("browser_not_found");
  const session = await createSession({ executablePath });
  let timer;
  let result;
  let failure;
  let stopped = false;
  let cleanupPending = false;
  let verificationUsed = false;
  const abort = new AbortController();
  const workDeadline = Date.now() + workTimeoutMs;
  const catalogDeadline = Date.now() + workTimeoutMs - 1000;
  try {
    const work = async () => {
      await interaction?.setPhase("login_window");
      await session.waitForWindowClosed(timeoutMs);
      if (stopped) throw new Error("browser_work_timeout");
      await interaction?.setPhase("session_read");
      let port = await session.startCapture(10000);
      let deadline = Date.now() + captureMs;
      let lastError;
      do {
        if (stopped || !session.isAlive()) throw new Error("browser_closed");
        try {
          const { tabs } = await readSession({ operation: "list", port });
          if (stopped || !session.isAlive()) throw new Error("browser_closed");
          if (tabs.length !== 1) throw new Error("tab_unavailable");
          const result = await readSession({ operation: "prepare", port, targetId: tabs[0].id }, { catalogDeadline });
          if (stopped || !session.isAlive()) throw new Error("browser_closed");
          return result;
        } catch (error) {
          // An explicit denial is not document-readiness jitter; never poll a challenge until timeout.
          if (error?.message === "session_verification_required" && interaction && !verificationUsed) {
            verificationUsed = true;
            try { await session.prepareVerification(); }
            catch { throw new Error("verification_prepare_failed"); }
            if (stopped) throw new Error("browser_work_timeout");
            await interaction.setPhase("verification_required");
            await interaction.waitForVerification(Math.min(workDeadline, Date.now() + 120000), abort.signal);
            if (stopped) throw new Error("browser_work_timeout");
            await interaction.setPhase("verification_window");
            if (stopped) throw new Error("browser_work_timeout");
            try { await session.openVerification(Math.max(1, workDeadline - Date.now())); }
            catch (cause) { if (cause?.message === "browser_timeout") throw new Error("verification_wait_timeout"); throw cause; }
            if (stopped) throw new Error("browser_work_timeout");
            await interaction.setPhase("session_read");
            port = await session.startCapture(10000);
            deadline = Date.now() + captureMs;
            continue;
          }
          if (["session_verification_required", "session_access_forbidden"].includes(error?.message)) throw error;
          if (!["tab_unavailable", "incomplete_session", "browser_unavailable", "browser_command_failed",
            ...SESSION_FAILURE_CODES].includes(error?.message)) throw error;
          lastError = error;
        }
        const delay = lastError?.message === "session_rate_limited" ? Math.max(5000, pollMs) : pollMs;
        await new Promise((resolve) => setTimeout(resolve, delay));
      } while (Date.now() < deadline);
      throw lastError || new Error("login_required");
    };
    // Leave 15 seconds for owned-process/profile cleanup before the worker's hard stop.
    result = await Promise.race([work(), new Promise((_, reject) => {
      timer = setTimeout(() => reject(new Error("browser_work_timeout")), workTimeoutMs);
    })]);
  } catch (error) { failure = error; }
  finally {
    stopped = true; abort.abort(); clearTimeout(timer);
    try { await session.close(); } catch { cleanupPending = true; }
  }
  // Cleanup never changes authentication/read outcome; the managed worker's parent still reaps its tree.
  if (failure) { if (cleanupPending) failure.cleanupPending = true; throw failure; }
  return cleanupPending ? { ...result, cleanupPending: true } : result;
}
