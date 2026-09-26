import { normalizeString } from "./configuration.mjs";

export function createWorkerError(status, code, message) {
  const error = new Error(message);
  error.status = status;
  error.code = code;
  return error;
}

export function serializeError(error) {
  return {
    status: Number.isFinite(error?.status) ? error.status : 500,
    code: normalizeString(error?.code) ?? "chatgpt_web_session_worker_failed",
    message:
      normalizeString(error?.message) ??
      "ChatGPT Web session worker failed without a structured error message.",
  };
}
