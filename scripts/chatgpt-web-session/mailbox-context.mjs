import { normalizeString } from "./configuration.mjs";

export function resolveMailboxContext(input, authSeed) {
  const mailboxRef = normalizeString(input?.mailboxRef);
  const mailboxSessionId =
    normalizeString(input?.mailboxSessionId) ?? decodeMailboxSessionId(mailboxRef);
  const mailboxProviderKey =
    normalizeString(input?.mailboxProviderKey) ?? decodeMailboxProviderKey(mailboxRef);
  const mailboxProviderInstanceId = decodeMailboxProviderInstanceId(mailboxRef);
  const decodedMailboxAddress = decodeMailboxAddress(mailboxRef);
  const configuredBaseUrl =
    normalizeString(process.env.CHATGPT_WEB_MAILBOX_SERVICE_BASE_URL) ??
    normalizeString(process.env.MAILBOX_SERVICE_BASE_URL) ??
    (process.platform === "win32" ? "http://127.0.0.1:18081" : "http://easy-email:8080");
  const apiKey =
    normalizeString(process.env.CHATGPT_WEB_MAILBOX_SERVICE_API_KEY) ??
    normalizeString(process.env.MAILBOX_SERVICE_API_KEY);
  return {
    mailboxRef,
    mailboxSessionId,
    mailboxProviderKey,
    mailboxProviderInstanceId,
    mailboxAddress: decodedMailboxAddress,
    email: normalizeString(authSeed?.email),
    hostId:
      normalizeString(input?.mailboxHostId) ??
      normalizeString(process.env.CHATGPT_WEB_MAILBOX_HOST_ID) ??
      "python-register-orchestration",
    serviceBaseUrl: configuredBaseUrl,
    apiKey,
  };
}

export function decodeMailboxSessionId(mailboxRef) {
  const text = normalizeString(mailboxRef);
  if (!text) {
    return null;
  }
  const index = text.indexOf(":");
  if (index < 0) {
    return text;
  }
  return text.slice(index + 1).trim() || null;
}

export function decodeMailboxProviderKey(mailboxRef) {
  const text = normalizeString(mailboxRef);
  if (!text) {
    return null;
  }
  const index = text.indexOf(":");
  if (index < 0) {
    return null;
  }
  return text.slice(0, index).trim() || null;
}

export function decodeMailboxProviderInstanceId(mailboxRef) {
  const text = normalizeString(mailboxRef);
  if (!text) {
    return null;
  }
  const first = text.indexOf(":");
  if (first < 0) {
    return null;
  }
  const second = text.indexOf(":", first + 1);
  if (second < 0) {
    return null;
  }
  return text.slice(first + 1, second).trim() || null;
}

export function decodeMailboxAddress(mailboxRef) {
  const payload = decodeMailboxRefPayload(mailboxRef);
  const address =
    normalizeString(payload?.address) ??
    normalizeString(payload?.email) ??
    normalizeString(payload?.mailbox);
  return address?.toLowerCase() ?? null;
}

export function decodeMailboxRefPayload(mailboxRef) {
  const text = normalizeString(mailboxRef);
  if (!text) {
    return null;
  }
  const first = text.indexOf(":");
  if (first < 0) {
    return null;
  }
  const second = text.indexOf(":", first + 1);
  if (second < 0) {
    return null;
  }
  const encoded = text.slice(second + 1).trim();
  if (!encoded) {
    return null;
  }
  try {
    const payload = JSON.parse(decodeURIComponent(encoded));
    return payload && typeof payload === "object" ? payload : null;
  } catch {
    return null;
  }
}
