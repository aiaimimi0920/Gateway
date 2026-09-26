import { normalizeString } from "./configuration.mjs";
import { createWorkerError } from "./errors.mjs";
import { setTimeout as delay } from "node:timers/promises";
import { fetchMailboxJson } from "./mailbox-http.mjs";
import { extractMailboxCodeMarker, selectOpenAiVerificationCode } from "./mailbox-codes.mjs";
import { fetchMailboxCodePayload, fetchMailboxSnapshot, fetchProviderDirectMailboxCode } from "./mailbox-snapshot.mjs";

export async function ensureRecoveredMailboxContext(mailboxContext) {
  if (!mailboxContext?.email || !mailboxContext?.mailboxProviderKey) {
    return mailboxContext;
  }
  try {
    const recovered = await recoverMailboxByEmail(mailboxContext, {
      emailAddress: mailboxContext.email,
      providerTypeKey: mailboxContext.mailboxProviderKey,
      hostId: mailboxContext.hostId,
    });
    if (recovered?.mailboxSessionId) {
      return {
        ...mailboxContext,
        mailboxRef: recovered.mailboxRef ?? mailboxContext.mailboxRef,
        mailboxSessionId: recovered.mailboxSessionId,
      };
    }
  } catch {
    // best-effort only; keep the original mailbox session if recovery fails
  }
  return mailboxContext;
}

export async function fetchMailboxLatestMarker(mailboxContext) {
  const payload = await fetchMailboxCodePayload(mailboxContext);
  const codeObject = payload?.code;
  return extractMailboxCodeMarker(codeObject);
}

export async function fetchMailboxSnapshotLatestMarker(mailboxContext) {
  const snapshot = await fetchMailboxSnapshot(mailboxContext, { markerOnly: true });
  return Number(snapshot?.marker || 0);
}

export async function fetchImmediateMailboxCode(mailboxContext, options = {}) {
  const minMarker = Number(options?.minMarker || 0);
  const payload = await fetchMailboxCodePayload(mailboxContext);
  const directCode = selectOpenAiVerificationCode(payload?.code);
  const directMarker = extractMailboxCodeMarker(payload?.code);
  if (directCode && (!minMarker || directMarker >= minMarker)) {
    return directCode;
  }
  const snapshot = await fetchMailboxSnapshot(mailboxContext).catch(() => null);
  if (snapshot?.code && (!minMarker || Number(snapshot.marker || 0) >= minMarker)) {
    return snapshot.code;
  }
  const providerCode = await fetchProviderDirectMailboxCode(mailboxContext).catch(() => null);
  if (providerCode?.code && (!minMarker || Number(providerCode.marker || 0) >= minMarker)) {
    return providerCode.code;
  }
  return "";
}

export async function recoverMailboxByEmail(mailboxContext, payload) {
  const raw = await fetchMailboxJson(
    `${mailboxContext.serviceBaseUrl.replace(/\/+$/, "")}/mail/mailboxes/recover-by-email`,
    {
      method: "POST",
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${mailboxContext.apiKey}`,
        "Content-Type": "application/json",
      },
      body: JSON.stringify(payload),
    },
    "mailbox_recover_status",
  );
  const result = raw?.result && typeof raw.result === "object" ? raw.result : raw;
  const session = result?.session && typeof result.session === "object" ? result.session : null;
  return session
    ? {
        mailboxRef: normalizeString(session.mailboxRef),
        mailboxSessionId: normalizeString(session.id),
      }
    : null;
}

export async function waitForMailboxOpenAiCode(mailboxContext, { timeoutMs, minMarker = 0, signal: parentSignal }) {
  const duration = Math.min(600_000, Math.max(5_000, Number(timeoutMs) || 5_000));
  const controller = new AbortController();
  const onParentAbort = () => controller.abort(parentSignal.reason);
  const timer = setTimeout(() => controller.abort(), duration);
  try {
    parentSignal?.throwIfAborted();
    parentSignal?.addEventListener("abort", onParentAbort, { once: true });
    return await pollMailboxCode(mailboxContext, Date.now() + duration, minMarker, controller.signal);
  } catch (error) {
    parentSignal?.throwIfAborted();
    throw error;
  } finally {
    parentSignal?.removeEventListener("abort", onParentAbort);
    clearTimeout(timer);
    controller.abort();
  }
}

async function pollMailboxCode(mailboxContext, deadline, minMarker, signal) {
  let attemptFailed = false;
  let lastSeenMarker = Number(minMarker || 0);
  while (!signal.aborted && Date.now() < deadline) {
    let snapshot = null;
    try {
      const payload = await fetchMailboxCodePayload(mailboxContext, { signal });
      signal.throwIfAborted();
      if (Date.now() >= deadline) break;
      const codeObject = payload?.code;
      const code = selectOpenAiVerificationCode(codeObject);
      const marker = extractMailboxCodeMarker(codeObject);
      if (code && (!lastSeenMarker || marker > lastSeenMarker)) {
        return code;
      }
    } catch {
      attemptFailed = true;
    }
    if (signal.aborted || Date.now() >= deadline) break;
    try {
      snapshot = await fetchMailboxSnapshot(mailboxContext, { signal });
      signal.throwIfAborted();
      if (Date.now() >= deadline) break;
      const snapshotCode = snapshot?.code;
      if (snapshotCode && (!lastSeenMarker || Number(snapshot.marker || 0) > lastSeenMarker)) {
        return snapshotCode;
      }
    } catch {
      attemptFailed = true;
    }
    if (signal.aborted || Date.now() >= deadline) break;
    try {
      const providerCode = await fetchProviderDirectMailboxCode(mailboxContext, { signal });
      signal.throwIfAborted();
      if (Date.now() >= deadline) break;
      if (
        providerCode?.code &&
        (!lastSeenMarker || Number(providerCode.marker || 0) > lastSeenMarker)
      ) {
        return providerCode.code;
      }
    } catch {
      attemptFailed = true;
    }
    if (signal.aborted || Date.now() >= deadline) break;
    await delay(Math.max(1, Math.min(4_000, deadline - Date.now())), undefined, { signal }).catch((error) => {
      if (!signal.aborted) throw error;
    });
  }
  throw createWorkerError(
    504,
    "chatgpt_web_mailbox_code_timeout",
    attemptFailed
      ? "Timed out waiting for ChatGPT/OpenAI email verification code. A mailbox attempt failed."
      : "Timed out waiting for ChatGPT/OpenAI email verification code.",
  );
}
