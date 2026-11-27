// Error projection only: never retain upstream messages, metadata, debug or detail bytes.
import { ProbeError } from "./transport.mjs";
import { fields } from "./protobuf_wire.mjs";
import { connectMessageSignals, cursorMessageSignals, noMessageSignals } from "./error_signals.mjs";

const connectCodes = new Set(["canceled", "unknown", "invalid_argument", "deadline_exceeded",
  "not_found", "already_exists", "permission_denied", "resource_exhausted", "failed_precondition",
  "aborted", "out_of_range", "unimplemented", "internal", "unavailable", "data_loss", "unauthenticated"]);
const cursorActions = new Set(["login", "upgrade", "payment", "config"]);

// aiserver.v1.ErrorDetails.Error, statically checked in @cursor/sdk@1.4.2.
// Numeric gaps and unknown future values are deliberately not inferred.
const cursorCodes = new Map([
  [1, "BAD_API_KEY"], [2, "NOT_LOGGED_IN"], [3, "INVALID_AUTH_ID"],
  [4, "NOT_HIGH_ENOUGH_PERMISSIONS"], [5, "BAD_MODEL_NAME"], [6, "USER_NOT_FOUND"],
  [7, "FREE_USER_RATE_LIMIT_EXCEEDED"], [8, "PRO_USER_RATE_LIMIT_EXCEEDED"],
  [9, "FREE_USER_USAGE_LIMIT"], [10, "PRO_USER_USAGE_LIMIT"], [11, "AUTH_TOKEN_NOT_FOUND"],
  [12, "AUTH_TOKEN_EXPIRED"], [13, "OPENAI"], [14, "OPENAI_RATE_LIMIT_EXCEEDED"],
  [18, "AGENT_REQUIRES_LOGIN"], [20, "MAX_TOKENS"], [21, "USER_ABORTED_REQUEST"],
  [22, "GENERIC_RATE_LIMIT_EXCEEDED"], [23, "PRO_USER_ONLY"], [25, "TIMEOUT"],
  [28, "GPT_4_VISION_PREVIEW_RATE_LIMIT"], [29, "CUSTOM_MESSAGE"], [30, "OUTDATED_CLIENT"],
  [31, "CLAUDE_IMAGE_TOO_LARGE"], [33, "FILE_NOT_FOUND"], [34, "API_KEY_RATE_LIMIT"],
  [35, "DEBOUNCED"], [36, "BAD_REQUEST"], [37, "REPOSITORY_SERVICE_REPOSITORY_IS_NOT_INITIALIZED"],
  [38, "UNAUTHORIZED"], [39, "NOT_FOUND"], [40, "DEPRECATED"], [41, "RESOURCE_EXHAUSTED"],
  [42, "BAD_USER_API_KEY"], [43, "CONVERSATION_TOO_LONG"], [44, "USAGE_PRICING_REQUIRED"],
  [45, "USAGE_PRICING_REQUIRED_CHANGEABLE"], [46, "GITHUB_NO_USER_CREDENTIALS"],
  [47, "GITHUB_USER_NO_ACCESS"], [48, "GITHUB_APP_NO_ACCESS"], [49, "GITHUB_MULTIPLE_OWNERS"],
  [50, "RATE_LIMITED"], [51, "RATE_LIMITED_CHANGEABLE"], [52, "CUSTOM"], [53, "HOOKS_BLOCKED"],
  [54, "SUSPICIOUS_USAGE_BLOCKED"], [55, "EXTENSION_HOST_TIMEOUT"], [56, "NETWORK_ERROR"],
  [57, "PROVIDER_ERROR"], [58, "MODEL_BLOCKED"], [59, "INTERNAL"], [60, "MAX_MODE_REQUIRED"],
  [61, "MODEL_NO_LONGER_SUPPORTED"], [62, "PRICING_WARNING"], [63, "SLOW_POOL"],
  [64, "UNSUPPORTED_REGION"], [65, "ACCOUNT_CLOSED"], [66, "TENANT_RESTRICTION_VIOLATION"],
]);

export function safeRemoteErrorLabel(key, value) {
  if (key === "remoteConnectCode") return connectCodes.has(value) ? value : null;
  if (key === "remoteCursorError") return [...cursorCodes.values()].includes(value) ? value : null;
  if (key === "remoteCursorAction") return cursorActions.has(value) ? value : null;
  return null;
}

const noHints = (status = "absent") => ({
  remoteCursorRetryable: null, remoteCursorAction: null, remoteCursorHintsStatus: status,
});

function oneField(items, tag, wire) {
  const matches = items.filter((item) => item.tag === tag);
  if (matches.length > 1 || matches.some((item) => item.wire !== wire)) throw new Error();
  return matches[0]?.value;
}

function cursorHints(bytes) {
  if (bytes === undefined) return noHints();
  try {
    // Pinned CLI CustomErrorDetails: #4 optional bool, #10 analytics message.
    // Neither a retry hint nor a suggested action authorizes an automatic operation.
    const items = fields(bytes);
    const retryable = oneField(items, 4, 0);
    if (retryable !== undefined && retryable > 1n) throw new Error();
    const analytics = oneField(items, 10, 2);
    const actionBytes = analytics === undefined ? undefined : oneField(fields(analytics), 1, 2);
    let action;
    if (actionBytes !== undefined) {
      action = actionBytes.length > 32 ? ""
        : new TextDecoder("utf-8", { fatal: true, ignoreBOM: true }).decode(actionBytes);
    }
    return {
      remoteCursorRetryable: retryable === undefined ? null : retryable === 1n,
      remoteCursorAction: cursorActions.has(action) ? action : null,
      remoteCursorHintsStatus: action !== undefined && !cursorActions.has(action) ? "unsupported_action"
        : retryable !== undefined || action !== undefined ? "recognized" : "absent",
    };
  } catch { return noHints("invalid"); }
}

function cursorDetail(details = []) {
  const absent = (status) => ({
    remoteCursorError: null, remoteCursorDetailStatus: status, ...noHints(), ...noMessageSignals(),
  });
  if (details.length > 32) return absent("too_large");
  const matching = details.filter((item) => item.type === "aiserver.v1.ErrorDetails");
  if (!matching.length) return absent("absent");
  if (matching.length !== 1) return absent("ambiguous");
  const value = matching[0].value;
  if (value.length > 16384) return absent("too_large");
  // Connect specifies unpadded standard Base64. Buffer alone accepts malformed input.
  if (!/^[A-Za-z0-9+/]*$/.test(value)) return absent("invalid");
  const bytes = Buffer.from(value, "base64");
  if (bytes.toString("base64").replace(/=+$/, "") !== value) return absent("invalid");
  let parsed;
  try { parsed = fields(bytes); } catch { return absent("invalid"); }
  const seen = new Set();
  for (const item of parsed) {
    if (item.tag > 3) continue; // Future fields cannot supply an error classification.
    if (seen.has(item.tag) || item.wire !== (item.tag === 2 ? 2 : 0) ||
        (item.tag === 3 && item.value > 1n)) return absent("invalid");
    seen.add(item.tag);
  }
  const code = parsed.find((item) => item.tag === 1)?.value;
  const label = code !== undefined && code <= 66n ? cursorCodes.get(Number(code)) : undefined;
  if (!label) return absent("unrecognized");
  return { remoteCursorError: label, remoteCursorDetailStatus: "recognized",
    ...cursorHints(parsed.find((item) => item.tag === 2)?.value),
    ...cursorMessageSignals(parsed.find((item) => item.tag === 2)?.value) };
}

export function validateEndStream(ending) {
  if (!ending || typeof ending !== "object" || Array.isArray(ending) ||
      Object.keys(ending).some((key) => key !== "metadata" && key !== "error")) {
    throw new ProbeError("invalid_end_stream");
  }
  if (Object.hasOwn(ending, "metadata")) {
    const metadata = ending.metadata;
    if (!metadata || typeof metadata !== "object" || Array.isArray(metadata) ||
        Object.values(metadata).some((values) =>
          !Array.isArray(values) || values.some((value) => typeof value !== "string"))) {
      throw new ProbeError("invalid_end_stream");
    }
  }
  if (!Object.hasOwn(ending, "error")) {
    return { remoteConnectCode: null, remoteCursorError: null, remoteCursorDetailStatus: "absent",
      ...noHints(), ...noMessageSignals(), ...connectMessageSignals() };
  }
  const error = ending.error;
  if (!error || typeof error !== "object" || Array.isArray(error) ||
      !connectCodes.has(error.code) ||
      (Object.hasOwn(error, "message") && typeof error.message !== "string") ||
      (Object.hasOwn(error, "details") && (!Array.isArray(error.details) ||
        error.details.some((detail) => !detail || typeof detail !== "object" ||
          typeof detail.type !== "string" || typeof detail.value !== "string")))) {
    throw new ProbeError("invalid_end_stream");
  }
  // Only payload decoding is optional. A malformed Connect detail envelope above
  // remains an invalid terminal; unsupported typed bytes never permit success/replay.
  return { remoteConnectCode: error.code, ...cursorDetail(error.details), ...connectMessageSignals(error.message) };
}
