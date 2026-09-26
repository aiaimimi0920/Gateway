import { normalizeString, dedupeStrings } from "./configuration.mjs";
import { decodeMailboxProviderInstanceId } from "./mailbox-context.mjs";
import { parseMailTimestamp, extractOpenAiCodeFromMessage } from "./mailbox-codes.mjs";
import { fetchIm215DirectMailboxCode } from "./mailbox-im215.mjs";
import { fetchMailboxJson } from "./mailbox-http.mjs";

export async function fetchMailboxCodePayload(mailboxContext, options = {}) {
  return fetchMailboxJson(
    `${mailboxContext.serviceBaseUrl.replace(/\/+$/, "")}/mail/mailboxes/${encodeURIComponent(mailboxContext.mailboxSessionId)}/code`,
    {
      signal: options.signal,
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${mailboxContext.apiKey}`,
      },
    },
    "mailbox_code_status",
  );
}

export async function fetchMailboxSnapshot(mailboxContext, options = {}) {
  const root = await fetchMailboxSnapshotRoot(mailboxContext, options);
  const candidateSessionIds = resolveMailboxSnapshotSessionIds(root, mailboxContext);
  const messages = Array.isArray(root?.messages) ? root.messages : [];
  let bestCode = null;
  let bestMarker = 0;
  for (const message of messages) {
    if (!message || typeof message !== "object") {
      continue;
    }
    const messageSessionId = String(message.sessionId ?? "").trim();
    if (!candidateSessionIds.has(messageSessionId)) {
      continue;
    }
    const marker = Math.max(
      parseMailTimestamp(message.observedAt),
      parseMailTimestamp(message.receivedAt),
    );
    if (marker <= 0 || marker < bestMarker) {
      continue;
    }
    if (options.markerOnly === true) {
      bestMarker = marker;
      continue;
    }
    const code = extractOpenAiCodeFromMessage(message);
    if (code) {
      bestCode = code;
      bestMarker = marker;
    }
  }
  if (bestCode) {
    return { code: bestCode, marker: bestMarker };
  }
  if (options.markerOnly === true && bestMarker > 0) {
    return { marker: bestMarker };
  }
  return null;
}

export function resolveMailboxSnapshotSessionIds(root, mailboxContext) {
  const ids = new Set();
  const expectedSessionId = normalizeString(mailboxContext?.mailboxSessionId);
  if (expectedSessionId) {
    ids.add(expectedSessionId);
  }
  const emailCandidates = dedupeStrings([
    mailboxContext?.email,
    mailboxContext?.mailboxAddress,
  ]).map((value) => value.toLowerCase());
  if (emailCandidates.length === 0) {
    return ids;
  }
  const expectedProviderType = normalizeString(mailboxContext?.mailboxProviderKey)?.toLowerCase();
  const expectedInstanceId = normalizeString(mailboxContext?.mailboxProviderInstanceId);
  const sessions = Array.isArray(root?.sessions) ? root.sessions : [];
  for (const session of sessions) {
    if (!session || typeof session !== "object") {
      continue;
    }
    const emailAddress = normalizeString(session.emailAddress)?.toLowerCase();
    if (!emailAddress || !emailCandidates.includes(emailAddress)) {
      continue;
    }
    const providerTypeKey = normalizeString(session.providerTypeKey)?.toLowerCase();
    if (expectedProviderType && providerTypeKey && providerTypeKey !== expectedProviderType) {
      continue;
    }
    const providerInstanceId = normalizeString(session.providerInstanceId);
    if (expectedInstanceId && providerInstanceId && providerInstanceId !== expectedInstanceId) {
      continue;
    }
    const sessionId = normalizeString(session.id);
    if (sessionId) {
      ids.add(sessionId);
    }
  }
  return ids;
}

export async function fetchProviderDirectMailboxCode(mailboxContext, options = {}) {
  const providerKey = normalizeString(mailboxContext?.mailboxProviderKey)?.toLowerCase();
  if (providerKey !== "im215") {
    return null;
  }
  const address = (
    normalizeString(mailboxContext?.email) ?? normalizeString(mailboxContext?.mailboxAddress)
  )?.toLowerCase();
  if (!address || !address.includes("@")) {
    return null;
  }
  const root = await fetchMailboxSnapshotRoot(mailboxContext, options);
  const config = resolveIm215ConfigFromSnapshot(root, mailboxContext);
  if (!config?.apiKey || !config?.baseUrl) {
    return null;
  }
  return fetchIm215DirectMailboxCode(config, address, options);
}

export async function fetchMailboxSnapshotRoot(mailboxContext, options = {}) {
  const payload = await fetchMailboxJson(
    `${mailboxContext.serviceBaseUrl.replace(/\/+$/, "")}/mail/snapshot`,
    {
      signal: options.signal,
      headers: {
        Accept: "application/json",
        Authorization: `Bearer ${mailboxContext.apiKey}`,
      },
    },
    "mailbox_snapshot_status",
  );
  return payload?.snapshot && typeof payload.snapshot === "object"
    ? payload.snapshot
    : payload?.result && typeof payload.result === "object"
      ? payload.result
      : payload;
}

export function resolveIm215ConfigFromSnapshot(root, mailboxContext) {
  const expectedInstanceId =
    normalizeString(mailboxContext?.mailboxProviderInstanceId) ??
    normalizeString(decodeMailboxProviderInstanceId(mailboxContext?.mailboxRef));
  if (!expectedInstanceId) {
    return null;
  }
  const instances = Array.isArray(root?.instances) ? root.instances : [];
  const instance = instances.find(
    (item) => normalizeString(item?.id) === expectedInstanceId,
  );
  if (!instance || typeof instance !== "object") {
    return null;
  }
  const metadata =
    instance.metadata && typeof instance.metadata === "object" ? instance.metadata : {};
  const baseUrl =
    normalizeString(metadata.apiBase) ??
    normalizeString(metadata.baseUrl) ??
    "https://maliapi.215.im/v1";
  const directApiKey = normalizeString(metadata.apiKey);
  if (directApiKey) {
    return { baseUrl, apiKey: directApiKey };
  }
  const credentialSetsJson = normalizeString(metadata.credentialSetsJson);
  if (!credentialSetsJson) {
    return null;
  }
  try {
    const sets = JSON.parse(credentialSetsJson);
    if (!Array.isArray(sets)) {
      return null;
    }
    for (const set of sets) {
      const items = Array.isArray(set?.items) ? set.items : [];
      for (const item of items) {
        const value = normalizeString(item?.value);
        if (value) {
          return { baseUrl, apiKey: value };
        }
      }
    }
  } catch {
    return null;
  }
  return null;
}
