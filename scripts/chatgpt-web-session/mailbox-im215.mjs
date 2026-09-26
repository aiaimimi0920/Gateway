import { normalizeBaseUrl } from "./configuration.mjs";
import { fetchMailboxResponse } from "./mailbox-http.mjs";
import { parseMailTimestamp, extractOpenAiCodeFromMessage } from "./mailbox-codes.mjs";

export async function fetchIm215DirectMailboxCode(config, address, options = {}) {
  const listResult = await im215Request(config, "GET", "/messages", {
    signal: options.signal,
    query: { address },
  });
  if (listResult.status === 404) {
    return null;
  }
  if (listResult.status !== 200) {
    throw new Error(`im215_list_status_${listResult.status}`);
  }
  const rows = extractIm215MessageList(listResult.data);
  let bestCode = null;
  let bestMarker = 0;
  for (const row of rows) {
    options.signal?.throwIfAborted();
    const record = readValueRecord(row);
    const sender = readIm215MessageSender(record) || "";
    const subject = readIm215MessageSubject(record) || "";
    const summaryText = readIm215MessageText(record) || "";
    const summaryHtml = readIm215MessageHtml(record) || "";
    const summaryCode = extractOpenAiCodeFromMessage({
      sender,
      subject,
      textBody: summaryText,
      htmlBody: summaryHtml,
    });
    const observedAt = readIm215ObservedAt(record) || new Date().toISOString();
    const marker = parseMailTimestamp(observedAt);
    if (summaryCode && marker >= bestMarker) {
      bestCode = summaryCode;
      bestMarker = marker;
      continue;
    }
    const messageId = readIm215MessageId(record);
    if (!messageId) {
      continue;
    }
    const detailResult = await im215Request(
      config,
      "GET",
      `/messages/${encodeURIComponent(messageId)}`,
      { query: { address }, signal: options.signal },
    );
    if (detailResult.status !== 200 && detailResult.status !== 404) {
      throw new Error(`im215_detail_status_${detailResult.status}`);
    }
    const detail = detailResult.status === 200 ? unwrapIm215MessageRecord(detailResult.data) : record;
    const code = extractOpenAiCodeFromMessage({
      sender: readIm215MessageSender(detail) || sender,
      subject: readIm215MessageSubject(detail) || subject,
      textBody: readIm215MessageText(detail) || summaryText,
      htmlBody: readIm215MessageHtml(detail) || summaryHtml,
    });
    const detailMarker = parseMailTimestamp(readIm215ObservedAt(detail) || observedAt);
    if (code && detailMarker >= bestMarker) {
      bestCode = code;
      bestMarker = detailMarker;
    }
  }
  return bestCode ? { code: bestCode, marker: bestMarker } : null;
}

export async function im215Request(config, method, requestPath, options = {}) {
  const baseUrl = normalizeBaseUrl(config.baseUrl);
  const url = new URL(
    requestPath.replace(/^\//, ""),
    baseUrl.endsWith("/") ? baseUrl : `${baseUrl}/`,
  );
  for (const [key, value] of Object.entries(options.query || {})) {
    if (value !== undefined && value !== null && String(value) !== "") {
      url.searchParams.set(key, String(value));
    }
  }
  const headers = {
    Accept: "application/json",
    ...(options.body ? { "Content-Type": "application/json" } : {}),
  };
  const apiKey = String(config.apiKey || "").trim();
  if (apiKey) {
    if (/^AC-/i.test(apiKey)) {
      headers["X-API-Key"] = apiKey;
    } else {
      headers.Authorization = `Bearer ${apiKey}`;
    }
  }
  const response = await fetchMailboxResponse(url.toString(), {
    signal: options.signal,
    method,
    headers,
    body: options.body ? JSON.stringify(options.body) : undefined,
  }, "im215_response_too_large");
  let data = response.text;
  if (response.text) {
    try {
      data = JSON.parse(response.text);
    } catch {
      data = response.text;
    }
  }
  return { status: response.status, data };
}

export function extractIm215MessageList(body) {
  const record = readValueRecord(body);
  const sources = [
    body,
    record.data,
    record.items,
    record.messages,
    record.list,
    readValueRecord(record.data).items,
    readValueRecord(record.data).messages,
    readValueRecord(record.result).items,
    readValueRecord(record.result).messages,
  ];
  for (const source of sources) {
    const items = readValueRecordList(source);
    if (items.length > 0) {
      return items;
    }
  }
  return [];
}

export function unwrapIm215MessageRecord(body) {
  const root = readValueRecord(body);
  const nested = [
    readValueRecord(root.data),
    readValueRecord(root.result),
    readValueRecord(root.message),
  ].find((item) => Object.keys(item).length);
  return Object.keys(nested || {}).length ? nested : root;
}

export function readIm215MessageId(record) {
  return (
    readValueString(record.id) ||
    readValueString(record.messageId) ||
    readValueString(record.mailId) ||
    readValueString(record.uuid) ||
    readValueString(record._id) ||
    ""
  );
}

export function readIm215ObservedAt(record) {
  return (
    readValueString(record.receivedAt) ||
    readValueString(record.createdAt) ||
    readValueString(record.updatedAt) ||
    readValueString(record.timestamp) ||
    ""
  );
}

export function readIm215MessageSubject(record) {
  return readValueString(record.subject) || readValueString(record.title) || "";
}

export function readIm215MessageSender(record) {
  return (
    readSender(record.from) ||
    readSender(record.sender) ||
    readValueString(record.from_address) ||
    readValueString(record.mailFrom) ||
    ""
  );
}

export function readIm215MessageText(record) {
  return (
    readValueString(record.text) ||
    readValueString(record.textBody) ||
    readValueString(record.body) ||
    readValueString(record.content) ||
    readValueString(record.snippet) ||
    readValueString(record.preview) ||
    readValueString(record.intro) ||
    ""
  );
}

export function readIm215MessageHtml(record) {
  return (
    readValueString(record.html) ||
    readValueString(record.htmlBody) ||
    readValueString(record.html_content) ||
    readValueString(record.raw_content) ||
    ""
  );
}

export function readValueRecord(value) {
  return value && typeof value === "object" && !Array.isArray(value) ? value : {};
}

export function readValueRecordList(value) {
  return Array.isArray(value)
    ? value.filter((item) => item && typeof item === "object" && !Array.isArray(item))
    : [];
}

export function readValueString(value) {
  return typeof value === "string" && value.trim() ? value.trim() : "";
}

export function readSender(value) {
  if (typeof value === "string" && value.trim()) {
    return value.trim();
  }
  if (!value || typeof value !== "object") {
    return "";
  }
  return (
    readValueString(value.address) ||
    readValueString(value.email) ||
    readValueString(value.name) ||
    ""
  );
}
